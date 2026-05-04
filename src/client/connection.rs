//! Client-side connection helper: open the Unix socket (auto-starting
//! the daemon if needed) and complete the Hello handshake.

use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::net::UnixStream;
use tracing::info;

use crate::paths;
use crate::protocol::{self, ClientToDaemon, DaemonToClient, PROTOCOL_VERSION};

/// Connect to the daemon (auto-starting it if necessary) and complete the
/// Hello handshake. Returns a stream ready for subsequent requests.
pub async fn connect_with_handshake() -> Result<UnixStream> {
    let socket_path = paths::socket_path()?;
    let mut stream = match UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(_) => {
            info!("daemon not reachable, starting it");
            spawn_daemon()?;
            wait_for_socket(&socket_path).await?
        }
    };

    handshake(&mut stream).await?;
    Ok(stream)
}

async fn handshake(stream: &mut UnixStream) -> Result<()> {
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(
        &mut writer,
        &ClientToDaemon::Hello {
            protocol_version: PROTOCOL_VERSION,
        },
    )
    .await?;
    let resp: DaemonToClient = protocol::read_msg(&mut reader).await?;
    match resp {
        DaemonToClient::Hello { protocol_version } if protocol_version == PROTOCOL_VERSION => Ok(()),
        DaemonToClient::Hello { protocol_version } => bail!(
            "daemon speaks protocol {protocol_version}; this client speaks {PROTOCOL_VERSION}. \
             Restart the daemon to match the upgraded binary."
        ),
        DaemonToClient::Error { code, message } => {
            bail!("daemon rejected handshake: {code:?} {message}")
        }
        other => bail!("unexpected daemon response during handshake: {other:?}"),
    }
}

fn spawn_daemon() -> Result<()> {
    let exe = std::env::current_exe().context("could not find own executable path")?;
    std::process::Command::new(&exe)
        .arg("daemon")
        .spawn()
        .with_context(|| format!("failed to spawn `{} daemon`", exe.display()))?;
    Ok(())
}

async fn wait_for_socket(path: &Path) -> Result<UnixStream> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut delay = Duration::from_millis(20);
    loop {
        if let Ok(stream) = UnixStream::connect(path).await {
            return Ok(stream);
        }
        if Instant::now() > deadline {
            return Err(anyhow!(
                "daemon did not become reachable within 2s (socket: {})",
                path.display()
            ));
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_millis(200));
    }
}
