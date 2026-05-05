//! `pswarm daemon stop` and `pswarm daemon restart`. Both run as client
//! commands: they talk to a running daemon via the Unix socket and use
//! the `Shutdown` protocol message to ask it to exit gracefully.

use anyhow::{Result, bail};

use crate::client::connection;
use crate::paths;
use crate::protocol::{self, ClientToDaemon, DaemonToClient, ErrorCode};

pub async fn stop(force: bool) -> Result<()> {
    let mut stream = connection::connect_no_spawn().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Shutdown { force }).await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Ok => {}
        DaemonToClient::Error {
            code: ErrorCode::ActiveAttachments,
            message,
        } => bail!("{message}"),
        DaemonToClient::Error { code, message } => bail!("{code:?}: {message}"),
        other => bail!("unexpected response: {other:?}"),
    }
    drop(stream);

    let socket_path = paths::socket_path()?;
    connection::wait_for_socket_gone(&socket_path).await?;

    println!("daemon stopped");
    Ok(())
}

pub async fn restart(force: bool) -> Result<()> {
    // Best-effort stop. Tolerate "no daemon running" — a restart with
    // nothing currently running is just a start.
    match connection::connect_no_spawn().await {
        Ok(mut stream) => {
            let (mut reader, mut writer) = stream.split();
            protocol::write_msg(&mut writer, &ClientToDaemon::Shutdown { force }).await?;
            match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
                DaemonToClient::Ok => {}
                DaemonToClient::Error {
                    code: ErrorCode::ActiveAttachments,
                    message,
                } => bail!("{message}"),
                DaemonToClient::Error { code, message } => bail!("{code:?}: {message}"),
                other => bail!("unexpected response: {other:?}"),
            }
            drop(stream);

            let socket_path = paths::socket_path()?;
            connection::wait_for_socket_gone(&socket_path).await?;
        }
        Err(_) => {
            // No daemon was running; fall through to start.
        }
    }

    // connect_with_handshake auto-spawns when no daemon is reachable.
    let stream = connection::connect_with_handshake().await?;
    drop(stream);

    println!("daemon restarted");
    Ok(())
}
