//! `pswarm doctor`: report local environment + round-trip a Status
//! query against the daemon (auto-starting it if needed).

use anyhow::{Result, bail};

use crate::client::connection;
use crate::paths;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run() -> Result<()> {
    let socket_path = paths::socket_path()?;
    let log_path = paths::log_path()?;
    let kitty_listen = std::env::var("KITTY_LISTEN_ON").ok();

    println!("client version: {}", env!("CARGO_PKG_VERSION"));
    println!("socket:         {}", socket_path.display());
    println!("daemon log:     {}", log_path.display());
    match &kitty_listen {
        Some(value) => println!("kitty:          KITTY_LISTEN_ON={value}"),
        None => println!("kitty:          KITTY_LISTEN_ON not set (kitty integration unavailable)"),
    }

    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Status).await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Status {
            uptime_seconds,
            agent_count,
            version,
        } => {
            println!("daemon:         ok");
            println!("  version:      {version}");
            println!("  uptime:       {}", format_uptime(uptime_seconds));
            println!("  agents:       {agent_count}");
        }
        other => bail!("unexpected response from daemon: {other:?}"),
    }

    Ok(())
}

fn format_uptime(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{h}h{m:02}m{s:02}s")
    } else if m > 0 {
        format!("{m}m{s:02}s")
    } else {
        format!("{s}s")
    }
}
