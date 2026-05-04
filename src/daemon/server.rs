//! Unix socket accept loop and per-connection handler.

use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tokio::net::{UnixListener, UnixStream};
use tracing::{info, warn};

use crate::protocol::{
    self, ClientToDaemon, DaemonToClient, ErrorCode, PROTOCOL_VERSION,
};

pub async fn run(socket_path: PathBuf) -> Result<()> {
    // Single-instance check: if the socket file is already there, see
    // whether a daemon is actually answering on it.
    if socket_path.exists() {
        match UnixStream::connect(&socket_path).await {
            Ok(_) => {
                info!(
                    "another daemon is already listening on {}, exiting",
                    socket_path.display()
                );
                return Ok(());
            }
            Err(_) => {
                warn!(
                    "removing stale socket at {}",
                    socket_path.display()
                );
                std::fs::remove_file(&socket_path).with_context(|| {
                    format!("failed to remove stale socket {}", socket_path.display())
                })?;
            }
        }
    }

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind {}", socket_path.display()))?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to chmod 0600 on {}", socket_path.display()))?;
    info!("listening on {}", socket_path.display());

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(handle_connection(stream));
    }
}

async fn handle_connection(mut stream: UnixStream) {
    if let Err(e) = handle_connection_inner(&mut stream).await {
        warn!("connection ended: {e:#}");
    }
}

async fn handle_connection_inner(stream: &mut UnixStream) -> Result<()> {
    let (mut reader, mut writer) = stream.split();

    // Hello handshake. The first message must be a Hello with a matching
    // protocol version, otherwise the daemon refuses and closes.
    let hello: ClientToDaemon = protocol::read_msg(&mut reader).await?;
    match hello {
        ClientToDaemon::Hello { protocol_version } if protocol_version == PROTOCOL_VERSION => {
            protocol::write_msg(
                &mut writer,
                &DaemonToClient::Hello {
                    protocol_version: PROTOCOL_VERSION,
                },
            )
            .await?;
        }
        ClientToDaemon::Hello { protocol_version } => {
            protocol::write_msg(
                &mut writer,
                &DaemonToClient::Error {
                    code: ErrorCode::ProtocolMismatch,
                    message: format!(
                        "client speaks protocol {protocol_version}, daemon speaks {PROTOCOL_VERSION}"
                    ),
                },
            )
            .await?;
            return Ok(());
        }
        _ => {
            protocol::write_msg(
                &mut writer,
                &DaemonToClient::Error {
                    code: ErrorCode::Internal,
                    message: "first message must be Hello".into(),
                },
            )
            .await?;
            return Ok(());
        }
    }

    // Process one request and respond. Streaming attach lives in a future
    // commit; for now the daemon closes the connection after one round-trip.
    let msg: ClientToDaemon = protocol::read_msg(&mut reader).await?;
    let response = handle_message(msg);
    protocol::write_msg(&mut writer, &response).await?;
    Ok(())
}

fn handle_message(msg: ClientToDaemon) -> DaemonToClient {
    match msg {
        ClientToDaemon::Ping => DaemonToClient::Pong,
        ClientToDaemon::Hello { .. } => DaemonToClient::Error {
            code: ErrorCode::Internal,
            message: "Hello already exchanged".into(),
        },
        ClientToDaemon::Run(_)
        | ClientToDaemon::Ls
        | ClientToDaemon::Attach { .. }
        | ClientToDaemon::Detach
        | ClientToDaemon::Resize(_)
        | ClientToDaemon::Stdin(_)
        | ClientToDaemon::Rm { .. } => DaemonToClient::Error {
            code: ErrorCode::Internal,
            message: "not implemented yet".into(),
        },
    }
}
