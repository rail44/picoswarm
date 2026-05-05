//! `pswarm send`: write text to a running agent's PTY without attaching.
//!
//! Reads `text` from the positional argument or stdin (whichever is
//! provided), strips trailing line endings from the input, and appends
//! a single CR (`\r`) so the byte stream looks like the user pressed
//! Enter at the end. TUI agents in raw-mode (Claude Code, vim, etc.)
//! treat LF as "insert newline" and CR as "submit"; using CR is the
//! same convention `tmux send-keys Enter` follows.

use anyhow::{Result, bail};
use std::io::{IsTerminal, Read};

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run(name: String, text: Option<String>) -> Result<()> {
    let mut payload = match text {
        Some(t) => t.into_bytes(),
        None => read_stdin_or_bail()?,
    };
    // Trim any trailing CR/LF the user (or `echo`) may have included,
    // then append exactly one CR.
    while matches!(payload.last(), Some(b'\n' | b'\r')) {
        payload.pop();
    }
    payload.push(b'\r');

    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Send { name, payload }).await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Ok => Ok(()),
        DaemonToClient::Error { code, message } => bail!("send failed: {code:?} {message}"),
        other => bail!("unexpected response: {other:?}"),
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
