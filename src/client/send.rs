//! `pswarm send`: write text to a running agent's PTY without attaching.
//!
//! Reads `text` from the positional argument or stdin (whichever is
//! provided), ensures the payload ends with a newline so Claude Code
//! submits it, and ships it to the daemon as a single `Send` request.

use anyhow::{Result, bail};
use std::io::{IsTerminal, Read};

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run(name: String, text: Option<String>) -> Result<()> {
    let mut payload = match text {
        Some(t) => t.into_bytes(),
        None => read_stdin_or_bail()?,
    };
    if !payload.ends_with(b"\n") {
        payload.push(b'\n');
    }

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
