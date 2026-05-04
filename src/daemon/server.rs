//! Unix socket accept loop and per-connection handler.

use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tokio::net::{UnixListener, UnixStream};
use tracing::{info, warn};

use crate::daemon::registry::Registry;
use crate::daemon::session;
use crate::protocol::{
    self, ClientToDaemon, DaemonToClient, ErrorCode, RunRequest, PROTOCOL_VERSION,
};

pub async fn run(socket_path: PathBuf) -> Result<()> {
    // Single-instance check.
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
                warn!("removing stale socket at {}", socket_path.display());
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

    let registry = Registry::new();

    loop {
        let (stream, _) = listener.accept().await?;
        let reg = registry.clone();
        tokio::spawn(handle_connection(stream, reg));
    }
}

async fn handle_connection(mut stream: UnixStream, registry: Registry) {
    if let Err(e) = handle_connection_inner(&mut stream, registry).await {
        warn!("connection ended: {e:#}");
    }
}

async fn handle_connection_inner(
    stream: &mut UnixStream,
    registry: Registry,
) -> Result<()> {
    let (mut reader, mut writer) = stream.split();

    // Hello handshake.
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

    // Process one request and respond, then close. Streaming attach lives
    // in a future commit.
    let msg: ClientToDaemon = protocol::read_msg(&mut reader).await?;
    let response = handle_message(msg, &registry).await;
    protocol::write_msg(&mut writer, &response).await?;
    Ok(())
}

async fn handle_message(msg: ClientToDaemon, registry: &Registry) -> DaemonToClient {
    match msg {
        ClientToDaemon::Ping => DaemonToClient::Pong,

        ClientToDaemon::Run(req) => handle_run(req, registry).await,

        ClientToDaemon::Ls => DaemonToClient::AgentList(registry.refresh_and_list()),

        ClientToDaemon::Rm { name, force } => match registry.remove(&name, force) {
            Some(_) => DaemonToClient::Ok,
            None => DaemonToClient::Error {
                code: ErrorCode::NotFound,
                message: format!("no agent named {name}"),
            },
        },

        ClientToDaemon::Hello { .. } => DaemonToClient::Error {
            code: ErrorCode::Internal,
            message: "Hello already exchanged".into(),
        },

        ClientToDaemon::Attach { .. }
        | ClientToDaemon::Detach
        | ClientToDaemon::Resize(_)
        | ClientToDaemon::Stdin(_) => DaemonToClient::Error {
            code: ErrorCode::Internal,
            message: "not implemented yet".into(),
        },
    }
}

async fn handle_run(req: RunRequest, registry: &Registry) -> DaemonToClient {
    let name = req.name.clone();

    // Spawning is blocking work (PTY syscalls, fork). Run it on the
    // blocking pool so we do not stall the async runtime.
    let entry_result = tokio::task::spawn_blocking(move || session::spawn_session(req)).await;

    let entry = match entry_result {
        Ok(Ok(entry)) => entry,
        Ok(Err(e)) => {
            return DaemonToClient::Error {
                code: ErrorCode::SpawnFailed,
                message: format!("{e:#}"),
            };
        }
        Err(e) => {
            return DaemonToClient::Error {
                code: ErrorCode::Internal,
                message: format!("spawn task panicked: {e}"),
            };
        }
    };

    let id = entry.id;
    if let Err(e) = registry.insert(entry) {
        return DaemonToClient::Error {
            code: ErrorCode::NameTaken,
            message: format!("{e:#}"),
        };
    }

    DaemonToClient::RunResult { id, name }
}
