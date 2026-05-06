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
//! \e[201~`) before the trailing CR. Modern TUIs (Claude Code,
//! current bash / zsh / fish, neovim, …) use bracketed-paste mode by
//! default, and without explicit markers they detect "paste" via
//! timing heuristics that consume the trailing CR as part of the
//! paste payload — which means the text lands in the input box but is
//! never submitted. Wrapping makes the boundary explicit so the CR
//! after the close marker reads as Enter.

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
    let payload = build_payload(raw);

    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Send { name, payload }).await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Ok => Ok(()),
        DaemonToClient::Error { code, message } => bail!("send failed: {code:?} {message}"),
        other => bail!("unexpected response: {other:?}"),
    }
}

/// Strip any trailing CR/LF the caller (or `echo`) may have included,
/// wrap the remainder in bracketed-paste markers when it contains an
/// embedded LF, then append a single CR so the agent reads Enter.
fn build_payload(mut raw: Vec<u8>) -> Vec<u8> {
    while matches!(raw.last(), Some(b'\n' | b'\r')) {
        raw.pop();
    }
    if raw.contains(&b'\n') {
        let mut wrapped = Vec::with_capacity(raw.len() + PASTE_START.len() + PASTE_END.len() + 1);
        wrapped.extend_from_slice(PASTE_START);
        wrapped.extend_from_slice(&raw);
        wrapped.extend_from_slice(PASTE_END);
        wrapped.push(b'\r');
        wrapped
    } else {
        raw.push(b'\r');
        raw
    }
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
    use super::build_payload;

    #[test]
    fn single_line_just_appends_cr() {
        assert_eq!(build_payload(b"hi".to_vec()), b"hi\r");
    }

    #[test]
    fn trailing_newlines_are_trimmed_before_cr() {
        assert_eq!(build_payload(b"hi\n".to_vec()), b"hi\r");
        assert_eq!(build_payload(b"hi\r\n".to_vec()), b"hi\r");
        assert_eq!(build_payload(b"hi\n\n".to_vec()), b"hi\r");
    }

    #[test]
    fn empty_input_becomes_bare_cr() {
        assert_eq!(build_payload(Vec::new()), b"\r");
        assert_eq!(build_payload(b"\n".to_vec()), b"\r");
    }

    #[test]
    fn multi_line_is_wrapped_in_paste_markers_with_cr_outside() {
        assert_eq!(build_payload(b"a\nb".to_vec()), b"\x1b[200~a\nb\x1b[201~\r");
    }

    #[test]
    fn multi_line_trailing_newline_is_trimmed_before_wrap() {
        // The trailing LF that came in as part of the input is stripped;
        // only the embedded LF survives, and the close marker plus CR
        // sit outside the paste.
        assert_eq!(
            build_payload(b"a\nb\n".to_vec()),
            b"\x1b[200~a\nb\x1b[201~\r"
        );
    }
}
