//! `pswarm attach`: re-attach to a running agent in the current terminal.
//!
//! Puts the local terminal into raw mode, forwards every keystroke to
//! the daemon as `Stdin`, prints every `Stdout` chunk back to the
//! terminal, and watches `SIGWINCH` to keep the agent's PTY size in sync.
//!
//! Detach trigger: **`Ctrl-\`** (single key). Recognised in two encoding
//! forms — the raw C0 byte `0x1c` (no keyboard protocol), and the CSI-u
//! sequence `\e[92;5u` (kitty keyboard protocol level 1, "disambiguate
//! escape codes"). Higher protocol levels (event types, associated text)
//! are not currently supported; in real use Claude Code only enables
//! level 1, but if that changes the matcher will silently miss the
//! trigger and we'll need to extend it.
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
    if let Err(e) = terminal::disable_raw_mode() {
        eprintln!("[warning: failed to restore terminal mode: {e}]");
    }

    match result? {
        AttachExit::Detached => eprintln!("[detached: {name}]"),
        AttachExit::SessionEnded(Some(code)) => eprintln!("[session ended: code={code}]"),
        AttachExit::SessionEnded(None) => eprintln!("[session ended]"),
        AttachExit::Closed => eprintln!(
            "[connection to daemon lost. Run `pswarm doctor` to check daemon state, then `pswarm run` to spawn a fresh agent if needed.]"
        ),
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
        .spawn(move || stdin_loop(&stdin_tx))
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
                        if out.write_all(&bytes).is_err() || out.flush().is_err() {
                            break AttachExit::Closed;
                        }
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

fn stdin_loop(tx: &mpsc::UnboundedSender<ClientToDaemon>) {
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut buf = [0u8; 4096];
    let mut leftover: Vec<u8> = Vec::new();
    let mut debug_log = open_debug_log();

    loop {
        let n = match handle.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };

        if let Some(file) = debug_log.as_mut()
            && let Err(e) = log_chunk(file, &buf[..n])
        {
            eprintln!("[warning: PSWARM_DEBUG_STDIN write failed: {e}]");
        }

        // Combine any leftover bytes from a previous read (held because
        // they could be the start of a partial CSI-u trigger encoding)
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
            // Receiver task is shutting down too; we don't care if this fails.
            #[allow(clippy::let_underscore_must_use)]
            let _ = tx.send(ClientToDaemon::Detach);
            return;
        }

        // No full trigger yet. Hold back any tail bytes that could be the
        // start of a CSI-u trigger encoding still arriving.
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

/// Look for `Ctrl-\` (raw `0x1c` or CSI-u `\e[92;5u`) anywhere in
/// `bytes`. Returns `(start, encoding_len)` of the matched trigger.
fn find_detach_trigger(bytes: &[u8]) -> Option<(usize, usize)> {
    let csi_u = b"\x1b[92;5u";
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1c {
            return Some((i, 1));
        }
        if bytes[i..].starts_with(csi_u) {
            return Some((i, csi_u.len()));
        }
        i += 1;
    }
    None
}

/// Returns the index at which to split `bytes`. The caller should emit
/// `bytes[..idx]` as `Stdin` and hold `bytes[idx..]` for the next read,
/// because that suffix could be the beginning of a CSI-u trigger
/// encoding (`\e[92;5u`) that has not finished arriving yet. The raw
/// `0x1c` trigger is a single byte and never partial.
fn partial_prefix_at_end(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let csi_u = b"\x1b[92;5u";
    let max = csi_u.len().min(bytes.len());
    for k in (1..=max).rev() {
        if bytes[bytes.len() - k..] == csi_u[..k] {
            return bytes.len() - k;
        }
    }
    bytes.len()
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
        // write! into a String can't fail; the helper bound is satisfied.
        #[allow(clippy::let_underscore_must_use)]
        let _ = write!(line, "{b:02x}");
    }
    line.push('\n');
    file.write_all(line.as_bytes())
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
    fn raw_ctrl_backslash_triggers() {
        assert_eq!(find_detach_trigger(b"\x1c"), Some((0, 1)));
    }

    #[test]
    fn raw_ctrl_backslash_in_text_triggers() {
        assert_eq!(find_detach_trigger(b"hello\x1cworld"), Some((5, 1)));
    }

    #[test]
    fn csi_u_ctrl_backslash_triggers() {
        // CSI-u for Ctrl-\: codepoint 92 ('\\') with modifier 5 (Ctrl).
        assert_eq!(find_detach_trigger(b"\x1b[92;5u"), Some((0, 7)));
    }

    #[test]
    fn csi_u_in_text_triggers() {
        assert_eq!(find_detach_trigger(b"abc\x1b[92;5uend"), Some((3, 7)));
    }

    #[test]
    fn unrelated_text_does_not_trigger() {
        assert_eq!(find_detach_trigger(b"hello world"), None);
    }

    #[test]
    fn plain_backslash_does_not_trigger() {
        // Just '\\' (0x5c) without Ctrl is not Ctrl-\.
        assert_eq!(find_detach_trigger(b"\\"), None);
    }

    #[test]
    fn arrow_keys_do_not_trigger() {
        // \x1b[A = arrow up — not a prefix of \x1b[92;5u.
        assert_eq!(find_detach_trigger(b"\x1b[A"), None);
    }

    #[test]
    fn partial_csi_u_is_held() {
        assert_eq!(partial_prefix_at_end(b"hello\x1b"), 5);
        assert_eq!(partial_prefix_at_end(b"hello\x1b["), 5);
        assert_eq!(partial_prefix_at_end(b"hello\x1b[9"), 5);
        assert_eq!(partial_prefix_at_end(b"hello\x1b[92;"), 5);
        assert_eq!(partial_prefix_at_end(b"hello\x1b[92;5"), 5);
    }

    #[test]
    fn raw_0x1c_is_never_partial() {
        // A bare 0x1c at the end of a chunk is the full trigger and is
        // caught by find_detach_trigger before partial_prefix_at_end ever
        // runs; if we reach this function it's because there's no match,
        // so 0x1c at the end should NOT be held back.
        assert_eq!(partial_prefix_at_end(b"hello\x1c"), 6);
    }

    #[test]
    fn unrelated_escape_at_end_is_not_held() {
        // Arrow-up escape doesn't match a prefix of \x1b[92;5u.
        assert_eq!(partial_prefix_at_end(b"hello\x1b[A"), 8);
    }

    #[test]
    fn empty_input() {
        assert_eq!(partial_prefix_at_end(b""), 0);
    }
}
