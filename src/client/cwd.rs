//! `pswarm cwd <name>`: print the running agent's current working directory.
//!
//! Reads `/proc/<pid>/cwd` on the daemon side. The output is plain text
//! so users can compose it: `cd (pswarm cwd feat-x)` (fish) or
//! `cd "$(pswarm cwd feat-x)"` (bash).

use anyhow::{Result, bail};

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run(name: String) -> Result<()> {
    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::GetCwd { name: name.clone() }).await?;

    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::AgentCwd { path: Some(p) } => {
            println!("{}", p.display());
            Ok(())
        }
        DaemonToClient::AgentCwd { path: None } => {
            bail!("no cwd available for {name} (process gone or unreadable)")
        }
        DaemonToClient::Error { code, message } => bail!("{code:?}: {message}"),
        other => bail!("unexpected response: {other:?}"),
    }
}
