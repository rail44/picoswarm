//! `pswarm attach`: re-attach to a running agent in the current terminal.
//!
//! Puts the local terminal into raw mode, forwards every keystroke to
//! the daemon as `Stdin`, prints every `Stdout` chunk back to the
//! terminal, and watches `SIGWINCH` to keep the agent's PTY size in sync.
//!
//! Detach trigger: press **`Ctrl-Q`** then **`q`** (or `Q`). Both keys
//! are recognised in either raw byte form or the CSI-u keyboard-protocol
//! form, so detach works whether or not the agent has enabled an
//! extended keyboard protocol. The matcher carries state across stdin
//! reads, so the two keys do not have to land in the same `read()`.
//!
//! Diagnostic: setting `PSWARM_DEBUG_STDIN=/path/to/file` makes the
//! attach client append every raw stdin chunk it sees (in `{:02x}` form)
//! to that file. Useful for figuring out what bytes the user's terminal
//! is actually sending for a given keypress.

use anyhow::{Result, anyhow, bail};
use crossterm::terminal;
use std::io::{Read, Write};
use tokio::net::unix::{ReadHalf, WriteHalf};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc;

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient, TermSize};

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

async fn stream_loop(reader: &mut ReadHalf<'_>, writer: &mut WriteHalf<'_>) -> Result<AttachExit> {
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
            if let Some(size) = current_terminal_size()
                && sigwinch_tx.send(ClientToDaemon::Resize(size)).is_err()
            {
                return;
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
    let mut leftover: Vec<u8> = Vec::new();
    let mut debug_log = open_debug_log();

    loop {
        let n = match handle.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => n,
            Err(_) => return,
        };

        if let Some(file) = debug_log.as_mut() {
            let _ = log_chunk(file, &buf[..n]);
        }

        // Combine any leftover bytes from a previous read (held because
        // they could be the start of a partial detach-trigger encoding)
        // with the new chunk.
        let mut combined: Vec<u8> = std::mem::take(&mut leftover);
        combined.extend_from_slice(&buf[..n]);

        if let Some((start, _len)) = find_detach_trigger(&combined) {
            if start > 0
                && tx
                    .send(ClientToDaemon::Stdin(combined[..start].to_vec()))
                    .is_err()
            {
                return;
            }
            let _ = tx.send(ClientToDaemon::Detach);
            return;
        }

        // No full trigger yet. If `combined` ends with the prefix of one
        // of the encodings we recognise, hold those tail bytes back so
        // the next read can complete the match.
        let split = partial_prefix_at_end(&combined);
        if split > 0
            && tx
                .send(ClientToDaemon::Stdin(combined[..split].to_vec()))
                .is_err()
        {
            return;
        }
        if split < combined.len() {
            leftover.extend_from_slice(&combined[split..]);
        }
    }
}

fn open_debug_log() -> Option<std::fs::File> {
    let path = std::env::var_os("PSWARM_DEBUG_STDIN")?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
}

fn log_chunk(file: &mut std::fs::File, chunk: &[u8]) -> std::io::Result<()> {
    use std::fmt::Write as _;
    let mut line = String::with_capacity(chunk.len() * 3 + 1);
    for (i, b) in chunk.iter().enumerate() {
        if i > 0 {
            line.push(' ');
        }
        let _ = write!(line, "{:02x}", b);
    }
    line.push('\n');
    file.write_all(line.as_bytes())
}

/// Look for a `Ctrl-Q` followed by `q`/`Q` anywhere in `bytes`. Each key
/// is matched in either raw byte form or its CSI-u keyboard-protocol
/// form. Returns `(start, total_len)` of the matched sequence.
fn find_detach_trigger(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < bytes.len() {
        if let Some(prefix_len) = ctrl_q_len(&bytes[i..]) {
            let after = i + prefix_len;
            if after < bytes.len()
                && let Some(cmd_len) = q_len(&bytes[after..])
            {
                return Some((i, prefix_len + cmd_len));
            }
        }
        i += 1;
    }
    None
}

/// If `bytes` begins with a `Ctrl-Q` (any encoding we recognise), return
/// the length of that encoding.
fn ctrl_q_len(bytes: &[u8]) -> Option<usize> {
    // Raw C0 byte (DC1 / XON, sent in raw mode without keyboard protocol).
    if bytes.first() == Some(&0x11) {
        return Some(1);
    }
    // CSI-u with the lowercase 'q' codepoint (113) plus the Ctrl modifier (5).
    if bytes.starts_with(b"\x1b[113;5u") {
        return Some(b"\x1b[113;5u".len());
    }
    None
}

/// If `bytes` begins with a plain `q` or `Q` (any encoding we recognise),
/// return the length.
fn q_len(bytes: &[u8]) -> Option<usize> {
    match bytes.first() {
        Some(&b'q') | Some(&b'Q') => Some(1),
        _ => {
            if bytes.starts_with(b"\x1b[113u") {
                Some(b"\x1b[113u".len())
            } else if bytes.starts_with(b"\x1b[81u") {
                Some(b"\x1b[81u".len())
            } else {
                None
            }
        }
    }
}

/// Returns the index at which to split `bytes`. The caller should emit
/// `bytes[..idx]` as `Stdin` and hold `bytes[idx..]` for the next read,
/// because that suffix could be the beginning of a longer detach-trigger
/// encoding that has not finished arriving yet.
fn partial_prefix_at_end(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }

    // Raw Ctrl-Q is a single byte; if it is the last byte we have to wait
    // to see whether `q` follows.
    if bytes.last() == Some(&0x11) {
        return bytes.len() - 1;
    }

    // CSI-u Ctrl-Q is "\x1b[113;5u". If `bytes` ends with any non-empty
    // proper prefix of that sequence (or the sequence in full, awaiting
    // the trailing `q`), hold it back.
    let csi_u = b"\x1b[113;5u";
    let max = csi_u.len().min(bytes.len());
    for k in (1..=max).rev() {
        if bytes[bytes.len() - k..] == csi_u[..k] {
            return bytes.len() - k;
        }
    }

    bytes.len()
}

fn current_terminal_size() -> Option<TermSize> {
    crossterm::terminal::size()
        .ok()
        .map(|(cols, rows)| TermSize { rows, cols })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_prefix_raw_command_lower() {
        // hello\x11q
        assert_eq!(find_detach_trigger(b"hello\x11q"), Some((5, 2)));
    }

    #[test]
    fn raw_prefix_raw_command_upper() {
        assert_eq!(find_detach_trigger(b"\x11Q"), Some((0, 2)));
    }

    #[test]
    fn csi_u_prefix_raw_command() {
        assert_eq!(find_detach_trigger(b"\x1b[113;5uq"), Some((0, 9)));
    }

    #[test]
    fn raw_prefix_csi_u_command() {
        assert_eq!(find_detach_trigger(b"\x11\x1b[113u"), Some((0, 7)));
    }

    #[test]
    fn csi_u_both() {
        assert_eq!(find_detach_trigger(b"\x1b[113;5u\x1b[113u"), Some((0, 14)));
    }

    #[test]
    fn embedded_in_text() {
        assert_eq!(find_detach_trigger(b"abc\x11qrest"), Some((3, 2)));
    }

    #[test]
    fn no_match() {
        assert_eq!(find_detach_trigger(b"hello world"), None);
    }

    #[test]
    fn prefix_without_command() {
        // Ctrl-Q alone should not trigger.
        assert_eq!(find_detach_trigger(b"\x11"), None);
        assert_eq!(find_detach_trigger(b"hello\x11"), None);
    }

    #[test]
    fn ctrl_q_followed_by_other_key() {
        // Ctrl-Q then 'a' should not trigger.
        assert_eq!(find_detach_trigger(b"\x11a"), None);
    }

    #[test]
    fn partial_prefix_holds_raw_ctrl_q() {
        assert_eq!(partial_prefix_at_end(b"hello\x11"), 5);
    }

    #[test]
    fn partial_prefix_holds_partial_csi_u() {
        // \x1b[113;5u is 8 bytes; every non-empty prefix should be held.
        assert_eq!(partial_prefix_at_end(b"hello\x1b"), 5);
        assert_eq!(partial_prefix_at_end(b"hello\x1b["), 5);
        assert_eq!(partial_prefix_at_end(b"hello\x1b[1"), 5);
        assert_eq!(partial_prefix_at_end(b"hello\x1b[113;5"), 5);
        assert_eq!(partial_prefix_at_end(b"hello\x1b[113;5u"), 5);
    }

    #[test]
    fn partial_prefix_does_not_hold_unrelated_escape() {
        // Arrow up (\x1b[A) is not a prefix of Ctrl-Q's CSI-u encoding.
        assert_eq!(partial_prefix_at_end(b"hello\x1b[A"), 8);
    }

    #[test]
    fn partial_prefix_does_not_hold_resolved_ctrl_q() {
        // \x11 followed by 'a' is fully resolved (not detach, but also
        // not a partial prefix).
        assert_eq!(partial_prefix_at_end(b"hello\x11a"), 7);
    }

    #[test]
    fn partial_prefix_empty() {
        assert_eq!(partial_prefix_at_end(b""), 0);
    }
}
