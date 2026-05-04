//! `pswarm doctor`: report local environment details and round-trip a
//! Ping to the daemon (auto-starting it if needed).

use anyhow::{bail, Result};

use crate::client::connection;
use crate::paths;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run() -> Result<()> {
    let socket_path = paths::socket_path()?;
    let kitty_listen = std::env::var("KITTY_LISTEN_ON").ok();

    println!("socket: {}", socket_path.display());
    match &kitty_listen {
        Some(value) => println!("kitty:  KITTY_LISTEN_ON={value}"),
        None => println!("kitty:  KITTY_LISTEN_ON not set (kitty integration unavailable)"),
    }

    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Ping).await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Pong => println!("daemon: ok"),
        other => bail!("unexpected response from daemon: {other:?}"),
    }

    Ok(())
}
