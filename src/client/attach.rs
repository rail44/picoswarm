//! `pswarm attach`: re-attach to a running agent in the current terminal.
//!
//! Puts the local terminal into raw mode, forwards every keystroke to
//! the daemon as `Stdin`, prints every `Stdout` chunk back to the
//! terminal, and watches `SIGWINCH` to keep the agent's PTY size in sync.
//! `Ctrl-\` sends a `Detach` and ends the session cleanly.

use anyhow::{anyhow, bail, Result};
use crossterm::terminal;
use std::io::{Read, Write};
use tokio::net::unix::{ReadHalf, WriteHalf};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient, TermSize};

const DETACH_BYTE: u8 = 0x1c; // Ctrl-\

enum AttachExit {
    Detached,
    SessionEnded(Option<i32>),
    Closed,
}

pub async fn run(name: String) -> Result<()> {
    let initial_size = current_terminal_size().unwrap_or(TermSize { rows: 24, cols: 80 });

    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();

    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Attach {
            name: name.clone(),
            initial_size,
        },
    )
    .await?;

    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Ok => {}
        DaemonToClient::Error { code, message } => {
            bail!("attach failed: {code:?} {message}")
        }
        other => bail!("unexpected response: {other:?}"),
    }

    terminal::enable_raw_mode().map_err(|e| anyhow!("enable_raw_mode: {e}"))?;
    let result = stream_loop(&mut reader, &mut writer).await;
    let _ = terminal::disable_raw_mode();

    match result? {
        AttachExit::Detached => eprintln!("[detached: {name}]"),
        AttachExit::SessionEnded(Some(code)) => eprintln!("[session ended: code={code}]"),
        AttachExit::SessionEnded(None) => eprintln!("[session ended]"),
        AttachExit::Closed => eprintln!("[connection closed]"),
    }
    Ok(())
}

async fn stream_loop(
    reader: &mut ReadHalf<'_>,
    writer: &mut WriteHalf<'_>,
) -> Result<AttachExit> {
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<ClientToDaemon>();

    // Stdin -> daemon. A blocking OS thread reads keystrokes from the
    // local terminal and forwards them through `msg_tx`.
    let stdin_tx = msg_tx.clone();
    let stdin_thread = std::thread::Builder::new()
        .name("pswarm-stdin".into())
        .spawn(move || stdin_loop(stdin_tx))
        .map_err(|e| anyhow!("failed to start stdin thread: {e}"))?;

    // SIGWINCH -> daemon.
    let mut sigwinch = signal(SignalKind::window_change())?;
    let sigwinch_tx = msg_tx.clone();
    let sigwinch_task = tokio::spawn(async move {
        loop {
            if sigwinch.recv().await.is_none() {
                return;
            }
            if let Some(size) = current_terminal_size() {
                if sigwinch_tx.send(ClientToDaemon::Resize(size)).is_err() {
                    return;
                }
            }
        }
    });

    drop(msg_tx);

    let exit = loop {
        tokio::select! {
            biased;

            Some(outgoing) = msg_rx.recv() => {
                if protocol::write_msg(writer, &outgoing).await.is_err() {
                    break AttachExit::Closed;
                }
            },

            incoming = protocol::read_msg::<DaemonToClient, _>(reader) => {
                match incoming {
                    Ok(DaemonToClient::Stdout(bytes)) => {
                        let mut out = std::io::stdout();
                        if out.write_all(&bytes).is_err() {
                            break AttachExit::Closed;
                        }
                        let _ = out.flush();
                    }
                    Ok(DaemonToClient::SessionEnded { exit_code }) => {
                        break AttachExit::SessionEnded(exit_code);
                    }
                    Ok(DaemonToClient::Ok) => break AttachExit::Detached,
                    Ok(other) => {
                        eprintln!("\r\n[unexpected daemon message: {other:?}]");
                        break AttachExit::Closed;
                    }
                    Err(_) => break AttachExit::Closed,
                }
            },
        }
    };

    sigwinch_task.abort();
    // The stdin thread is blocked on a `read` from the terminal and cannot
    // be cancelled cleanly. It will be reaped when the process exits.
    drop(stdin_thread);

    Ok(exit)
}

fn stdin_loop(tx: mpsc::UnboundedSender<ClientToDaemon>) {
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut buf = [0u8; 4096];
    loop {
        let n = match handle.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => n,
            Err(_) => return,
        };
        let chunk = &buf[..n];
        if let Some(pos) = chunk.iter().position(|&b| b == DETACH_BYTE) {
            if pos > 0 && tx
                .send(ClientToDaemon::Stdin(chunk[..pos].to_vec()))
                .is_err()
            {
                return;
            }
            let _ = tx.send(ClientToDaemon::Detach);
            return;
        }
        if tx.send(ClientToDaemon::Stdin(chunk.to_vec())).is_err() {
            return;
        }
    }
}

fn current_terminal_size() -> Option<TermSize> {
    crossterm::terminal::size()
        .ok()
        .map(|(cols, rows)| TermSize { rows, cols })
}
