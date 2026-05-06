//! `pswarm send`: write text to a running agent's PTY without attaching.
//!
//! Reads `text` from the positional argument or stdin, strips trailing
//! line endings, and appends a single CR (`\r`) so TUI agents in
//! raw-mode (Claude Code, vim, etc.) see Enter — those agents treat LF
//! as "insert newline" and CR as "submit", which is the same
//! convention `tmux send-keys Enter` follows.
//!
//! For payloads that contain an embedded LF (i.e. multi-line input),
//! the text is wrapped in bracketed-paste markers (`\e[200~ ...
//! \e[201~`) and the trailing CR is sent in a **separate** request
//! after the paste-end marker. Modern TUIs (Claude Code in
//! particular) collapse a paste of more than ~6 lines into a
//! `[Pasted text +N lines]` placeholder; if the CR arrives in the
//! same write as the paste-end marker, the placeholder swallows it
//! and the message is never submitted. Splitting the writes lets
//! the agent's input loop transition out of paste-handling state
//! before the Enter arrives. Single-line payloads still go through
//! in one write — the boundary issue does not apply there.

use anyhow::{Result, bail};
use std::io::{IsTerminal, Read};

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

pub async fn run(name: String, text: Option<String>) -> Result<()> {
    let raw = match text {
        Some(t) => t.into_bytes(),
        None => read_stdin_or_bail()?,
    };
    let trimmed = trim_trailing_line_endings(raw);

    if trimmed.contains(&b'\n') {
        // Multi-line: send paste-wrap + trailing CR, pause briefly,
        // then send a *second* bare CR as a separate request.
        // Modern TUIs (Claude Code in particular) collapse a multi-
        // line paste into a `[Pasted text +N lines]` placeholder.
        // The first CR ends the paste and lands the placeholder in
        // the input box; the second CR is what actually submits it.
        // ~80 ms between writes is enough for the placeholder
        // transition to settle; below human noticeability.
        let mut wrapped =
            Vec::with_capacity(trimmed.len() + PASTE_START.len() + PASTE_END.len() + 1);
        wrapped.extend_from_slice(PASTE_START);
        wrapped.extend_from_slice(&trimmed);
        wrapped.extend_from_slice(PASTE_END);
        wrapped.push(b'\r');
        send_payload(&name, wrapped).await?;
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        send_payload(&name, b"\r".to_vec()).await?;
    } else {
        let mut payload = trimmed;
        payload.push(b'\r');
        send_payload(&name, payload).await?;
    }
    Ok(())
}

async fn send_payload(name: &str, payload: Vec<u8>) -> Result<()> {
    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Send {
            name: name.to_string(),
            payload,
        },
    )
    .await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Ok => Ok(()),
        DaemonToClient::Error { code, message } => bail!("send failed: {code:?} {message}"),
        other => bail!("unexpected response: {other:?}"),
    }
}

fn trim_trailing_line_endings(mut raw: Vec<u8>) -> Vec<u8> {
    while matches!(raw.last(), Some(b'\n' | b'\r')) {
        raw.pop();
    }
    raw
}

fn read_stdin_or_bail() -> Result<Vec<u8>> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        bail!(
            "no text given and stdin is a terminal; pass text as an argument or pipe it in (e.g. `echo foo | pswarm send <name>`)"
        );
    }
    let mut buf = Vec::new();
    stdin.lock().read_to_end(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::trim_trailing_line_endings;

    #[test]
    fn trim_strips_cr_and_lf_runs() {
        assert_eq!(trim_trailing_line_endings(b"hi".to_vec()), b"hi");
        assert_eq!(trim_trailing_line_endings(b"hi\n".to_vec()), b"hi");
        assert_eq!(trim_trailing_line_endings(b"hi\r\n".to_vec()), b"hi");
        assert_eq!(trim_trailing_line_endings(b"hi\n\n".to_vec()), b"hi");
        assert_eq!(trim_trailing_line_endings(b"a\nb".to_vec()), b"a\nb");
        assert_eq!(trim_trailing_line_endings(b"a\nb\n".to_vec()), b"a\nb");
        assert_eq!(trim_trailing_line_endings(Vec::new()), b"");
        assert_eq!(trim_trailing_line_endings(b"\n".to_vec()), b"");
    }
}
