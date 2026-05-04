//! `pswarm clean`: drop registered agents whose process has already exited.

use anyhow::{Result, bail};

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run() -> Result<()> {
    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Clean).await?;

    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Cleaned { removed } => {
            if removed.is_empty() {
                println!("no dead agents");
            } else {
                for name in &removed {
                    println!("removed {name}");
                }
            }
            Ok(())
        }
        DaemonToClient::Error { code, message } => bail!("{code:?}: {message}"),
        other => bail!("unexpected response: {other:?}"),
    }
}
