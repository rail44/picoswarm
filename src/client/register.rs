//! `pswarm register`: claim a PTY-less identity in the registry.
//!
//! Used by drivers — Claude sessions that orchestrate other agents via
//! `pswarm` — to take a unique name, get an inbox, and record lifecycle
//! events without the daemon spawning a child process. The entry shows
//! in `pswarm ls` with status `registered`; PTY-bound subcommands
//! (`send`, `view`, `attach`, `cwd`) reject it with a clear message.

use anyhow::{Result, bail};

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run(name: String) -> Result<()> {
    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Register { name: name.clone() },
    )
    .await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Ok => {
            println!("registered {name}");
            Ok(())
        }
        DaemonToClient::Error { code, message } => bail!("{code:?}: {message}"),
        other => bail!("unexpected response: {other:?}"),
    }
}
