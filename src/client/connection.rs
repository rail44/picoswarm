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

/// Connect to the daemon WITHOUT auto-starting it. Returns Err if no
/// daemon is reachable; useful for `daemon stop` where auto-spawning a
/// new daemon to immediately tell it to shut down would be silly.
pub(super) async fn connect_no_spawn() -> Result<UnixStream> {
    let socket_path = paths::socket_path()?;
    let mut stream = UnixStream::connect(&socket_path).await.with_context(|| {
        format!(
            "no daemon is listening at {} (use `pswarm daemon start`)",
            socket_path.display()
        )
    })?;
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

pub(super) fn spawn_daemon() -> Result<()> {
    let exe = std::env::current_exe().context("could not find own executable path")?;
    std::process::Command::new(&exe)
        .args(["daemon", "start"])
        .spawn()
        .with_context(|| format!("failed to spawn `{} daemon start`", exe.display()))?;
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

/// Block until the daemon socket disappears (or fails to accept new
/// connections). Used after sending Shutdown to confirm the daemon has
/// actually exited before the caller proceeds.
pub(super) async fn wait_for_socket_gone(path: &Path) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut delay = Duration::from_millis(20);
    loop {
        if !path.exists() {
            return Ok(());
        }
        // Socket file may linger briefly after the daemon dies; if a
        // connect attempt fails, the daemon is effectively gone.
        if UnixStream::connect(path).await.is_err() {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err(anyhow!(
                "daemon socket still reachable after 2s (socket: {})",
                path.display()
            ));
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_millis(200));
    }
}
