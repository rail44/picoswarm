//! `pswarm view`: dump the agent's recent PTY output (ring buffer
//! contents) without attaching. Read-only, no resize side effect.

use anyhow::{Result, bail};
use std::io::Write;

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run(name: String) -> Result<()> {
    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::View { name }).await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Stdout(bytes) => {
            let mut out = std::io::stdout().lock();
            out.write_all(&bytes)?;
            out.flush()?;
            Ok(())
        }
        DaemonToClient::Error { code, message } => bail!("view failed: {code:?} {message}"),
        other => bail!("unexpected response: {other:?}"),
    }
}
