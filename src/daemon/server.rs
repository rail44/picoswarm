//! Unix socket accept loop and per-connection handler.
//!
//! Most subcommands are handled in a one-shot request/response style; the
//! connection closes after the response. `Attach` is special — once the
//! daemon has acknowledged the attach, the same connection switches into
//! a bidirectional streaming session until either side disconnects or
//! sends `Detach`.

use anyhow::{Context, Result};
use pty_process::Size;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::io::AsyncWriteExt;
use tokio::net::{
    UnixListener, UnixStream,
    unix::{ReadHalf, WriteHalf},
};
use tokio::sync::Notify;
use tracing::{debug, info, warn};

use crate::daemon::output_session::{SessionEvent, SubscribeReply};
use crate::daemon::registry::{AgentEntry, Registry};
use crate::daemon::spawner::Spawner;
use crate::protocol::{
    self, ClientToDaemon, DaemonToClient, ErrorCode, Event, PROTOCOL_VERSION, RunRequest, TermSize,
};

pub async fn run(socket_path: PathBuf, spawner: Spawner) -> Result<()> {
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
    let shutdown = Arc::new(Notify::new());
    let started_at = std::time::Instant::now();

    loop {
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                info!("shutdown requested");
                break;
            }
            accept_result = listener.accept() => {
                let (stream, _) = accept_result?;
                let reg = registry.clone();
                let sd = Arc::clone(&shutdown);
                let sp = spawner.clone();
                tokio::spawn(handle_connection(stream, reg, sd, started_at, sp));
            }
        }
    }

    // Stop accepting new connections, kill every live agent so its child
    // process doesn't survive as an init orphan, and remove the socket
    // file. In-flight connection tasks may still be writing their final
    // responses; we let them race to completion against the daemon's
    // exit.
    drop(listener);
    registry.shutdown_all();
    if let Err(e) = std::fs::remove_file(&socket_path) {
        warn!(
            "failed to remove socket file at {}: {}",
            socket_path.display(),
            e
        );
    }

    Ok(())
}

async fn handle_connection(
    mut stream: UnixStream,
    registry: Registry,
    shutdown: Arc<Notify>,
    started_at: std::time::Instant,
    spawner: Spawner,
) {
    if let Err(e) =
        handle_connection_inner(&mut stream, registry, shutdown, started_at, &spawner).await
    {
        warn!("connection ended: {e:#}");
    }
}

async fn handle_connection_inner(
    stream: &mut UnixStream,
    registry: Registry,
    shutdown: Arc<Notify>,
    started_at: std::time::Instant,
    spawner: &Spawner,
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

    let msg: ClientToDaemon = protocol::read_msg(&mut reader).await?;
    match msg {
        ClientToDaemon::Attach { name, initial_size } => {
            handle_attach(name, initial_size, &registry, &mut reader, &mut writer).await?;
        }
        ClientToDaemon::Shutdown { force } => {
            let attached = registry.attached_names();
            if !force && !attached.is_empty() {
                protocol::write_msg(
                    &mut writer,
                    &DaemonToClient::Error {
                        code: ErrorCode::ActiveAttachments,
                        message: format!(
                            "{} agent(s) still attached: {}. Detach them or rerun with --force.",
                            attached.len(),
                            attached.join(", ")
                        ),
                    },
                )
                .await?;
            } else {
                // Acknowledge BEFORE notifying the accept loop so the
                // client sees the Ok even if the listener closes
                // immediately after.
                protocol::write_msg(&mut writer, &DaemonToClient::Ok).await?;
                if let Err(e) = writer.flush().await {
                    debug!("flush after Shutdown ack failed: {e}");
                }
                shutdown.notify_one();
            }
        }
        other => {
            let response = handle_oneshot(other, &registry, started_at, spawner).await;
            protocol::write_msg(&mut writer, &response).await?;
        }
    }
    Ok(())
}

async fn handle_oneshot(
    msg: ClientToDaemon,
    registry: &Registry,
    started_at: std::time::Instant,
    spawner: &Spawner,
) -> DaemonToClient {
    match msg {
        ClientToDaemon::Ping => DaemonToClient::Pong,

        ClientToDaemon::Run(req) => handle_run(req, registry, spawner).await,

        ClientToDaemon::Ls => DaemonToClient::AgentList(registry.list()),

        ClientToDaemon::Rm { name, force } => {
            // remove() can block for up to GRACEFUL_TERMINATE_GRACE
            // (~1s) waiting for SIGTERM to take effect; run it on the
            // blocking pool so the async runtime stays responsive.
            let reg = registry.clone();
            let n = name.clone();
            match tokio::task::spawn_blocking(move || reg.remove(&n, force)).await {
                Ok(Some(_)) => DaemonToClient::Ok,
                Ok(None) => DaemonToClient::Error {
                    code: ErrorCode::NotFound,
                    message: format!("no agent named {name}"),
                },
                Err(e) => DaemonToClient::Error {
                    code: ErrorCode::Internal,
                    message: format!("rm task panicked: {e}"),
                },
            }
        }

        ClientToDaemon::Status => DaemonToClient::Status {
            uptime_seconds: started_at.elapsed().as_secs(),
            agent_count: registry.list().len() as u32,
            version: env!("CARGO_PKG_VERSION").to_string(),
        },

        ClientToDaemon::Clean => DaemonToClient::Cleaned {
            removed: registry.prune_dead(),
        },

        ClientToDaemon::GetCwd { name } => registry.pid_of(&name).map_or_else(
            || DaemonToClient::Error {
                code: ErrorCode::NotFound,
                message: format!("no agent named {name} (or it has no live PID)"),
            },
            |pid| {
                let path = std::fs::read_link(format!("/proc/{pid}/cwd")).ok();
                DaemonToClient::AgentCwd { path }
            },
        ),

        ClientToDaemon::View { name } => match registry.lookup(&name) {
            Some(entry) => entry.inbox.snapshot().await.map_or_else(
                || DaemonToClient::Error {
                    code: ErrorCode::Internal,
                    message: format!("session task for {name} is gone"),
                },
                DaemonToClient::Stdout,
            ),
            None => DaemonToClient::Error {
                code: ErrorCode::NotFound,
                message: format!("no agent named {name}"),
            },
        },

        ClientToDaemon::Event { name, event } => handle_event(registry, &name, event),

        ClientToDaemon::Send { name, payload } => match registry.lookup(&name) {
            Some(entry) => {
                // Write on the blocking pool; the PTY writer is std::io,
                // and we don't want to stall the async runtime.
                let result = tokio::task::spawn_blocking(move || {
                    let _guard = entry
                        .write_lock
                        .lock()
                        .map_err(|_| anyhow::anyhow!("write lock poisoned"))?;
                    let mut w = &*entry.pty;
                    w.write_all(&payload)?;
                    w.flush()?;
                    Ok::<(), anyhow::Error>(())
                })
                .await;
                match result {
                    Ok(Ok(())) => DaemonToClient::Ok,
                    Ok(Err(e)) => DaemonToClient::Error {
                        code: ErrorCode::Internal,
                        message: format!("write to {name} failed: {e}"),
                    },
                    Err(e) => DaemonToClient::Error {
                        code: ErrorCode::Internal,
                        message: format!("send task panicked: {e}"),
                    },
                }
            }
            None => DaemonToClient::Error {
                code: ErrorCode::NotFound,
                message: format!("no agent named {name}"),
            },
        },

        ClientToDaemon::Hello { .. } => DaemonToClient::Error {
            code: ErrorCode::Internal,
            message: "Hello already exchanged".into(),
        },

        ClientToDaemon::Attach { .. } | ClientToDaemon::Shutdown { .. } => DaemonToClient::Error {
            code: ErrorCode::Internal,
            message: "internal routing bug: this message should not reach handle_oneshot".into(),
        },

        ClientToDaemon::Detach | ClientToDaemon::Resize(_) | ClientToDaemon::Stdin(_) => {
            DaemonToClient::Error {
                code: ErrorCode::Internal,
                message: "this message is only valid during an active attach".into(),
            }
        }
    }
}

fn handle_event(registry: &Registry, name: &str, event: Event) -> DaemonToClient {
    // Tracing here is the diagnostic surface for plugin / hook
    // wiring: silent failures in the hook script (intentionally
    // swallowed by `pswarm event self`) are otherwise invisible.
    // `debug!` keeps the success path quiet at the default log
    // level; `warn!` makes the rejection visible without tweaking
    // RUST_LOG.
    registry.lookup(name).map_or_else(
        || {
            warn!(agent = %name, ?event, "event for unknown agent");
            DaemonToClient::Error {
                code: ErrorCode::NotFound,
                message: format!("no agent named {name}"),
            }
        },
        |entry| {
            entry.record_event(event);
            debug!(agent = %name, ?event, "event recorded");
            DaemonToClient::Ok
        },
    )
}

async fn handle_run(req: RunRequest, registry: &Registry, spawner: &Spawner) -> DaemonToClient {
    let name = req.name.clone();
    // Routing fork through the dedicated spawner thread is load-bearing:
    // see `daemon::spawner` for why `spawn_blocking` would silently kill
    // every agent ~10s after spawn.
    let entry = match spawner.spawn(req).await {
        Ok(entry) => entry,
        Err(e) => {
            return DaemonToClient::Error {
                code: ErrorCode::SpawnFailed,
                message: format!("{e:#}"),
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

/// RAII guard that clears an `attached` flag on drop, so the flag is
/// released regardless of how the attach handler exits.
struct AttachGuard {
    flag: Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for AttachGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

// The bidirectional streaming loop is naturally large (subscribe,
// backlog drain, then a select! over events / client messages). Splitting
// it across helpers would obscure the message flow more than it would
// help readability.
#[allow(clippy::too_many_lines)]
async fn handle_attach(
    name: String,
    initial_size: TermSize,
    registry: &Registry,
    reader: &mut ReadHalf<'_>,
    writer: &mut WriteHalf<'_>,
) -> Result<()> {
    let Some(entry) = registry.lookup(&name) else {
        protocol::write_msg(
            writer,
            &DaemonToClient::Error {
                code: ErrorCode::NotFound,
                message: format!("no agent named {name}"),
            },
        )
        .await?;
        return Ok(());
    };

    if entry
        .attached
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        protocol::write_msg(
            writer,
            &DaemonToClient::Error {
                code: ErrorCode::AlreadyAttached,
                message: format!("{name} is already attached"),
            },
        )
        .await?;
        return Ok(());
    }
    let _attach_guard = AttachGuard {
        flag: Arc::clone(&entry.attached),
    };

    // Resize the PTY to the client's terminal before any output is sent.
    apply_resize(&entry, initial_size);

    protocol::write_msg(writer, &DaemonToClient::Ok).await?;

    let Some(SubscribeReply {
        backlog,
        mut events,
    }) = entry.inbox.subscribe().await
    else {
        protocol::write_msg(
            writer,
            &DaemonToClient::Error {
                code: ErrorCode::Internal,
                message: "session task is gone".into(),
            },
        )
        .await?;
        return Ok(());
    };

    if !backlog.is_empty() {
        protocol::write_msg(writer, &DaemonToClient::Stdout(backlog)).await?;
    }

    debug!("attach streaming for {} started", name);

    loop {
        tokio::select! {
            biased;

            // Daemon -> client: live PTY output and end-of-session.
            evt = events.recv() => match evt {
                Some(SessionEvent::Output(bytes)) => {
                    protocol::write_msg(writer, &DaemonToClient::Stdout(bytes)).await?;
                }
                Some(SessionEvent::Ended { exit_code }) => {
                    protocol::write_msg(
                        writer,
                        &DaemonToClient::SessionEnded { exit_code },
                    )
                    .await?;
                    break;
                }
                None => {
                    debug!("session events channel closed for {}", name);
                    break;
                }
            },

            // Client -> daemon.
            incoming = protocol::read_msg::<ClientToDaemon, _>(reader) => {
                let msg = match incoming {
                    Ok(m) => m,
                    Err(e) => {
                        debug!("client {} disconnected mid-attach: {}", name, e);
                        break;
                    }
                };
                match msg {
                    ClientToDaemon::Stdin(bytes) => {
                        let entry_for_write = entry.clone();
                        let agent_for_log = name.clone();
                        // Result discarded: panics from the closure are
                        // already logged inside it via tracing; the outer
                        // task can't meaningfully recover.
                        #[allow(clippy::let_underscore_must_use)]
                        let _ = tokio::task::spawn_blocking(move || {
                            let Ok(_guard) = entry_for_write.write_lock.lock() else {
                                warn!("write lock poisoned for {agent_for_log}, dropping stdin chunk");
                                return;
                            };
                            let mut w = &*entry_for_write.pty;
                            if let Err(e) = w.write_all(&bytes).and_then(|()| w.flush()) {
                                debug!("stdin write to {agent_for_log} failed (PTY likely gone): {e}");
                            }
                        }).await;
                    }
                    ClientToDaemon::Resize(sz) => {
                        apply_resize(&entry, sz);
                    }
                    ClientToDaemon::Detach => {
                        protocol::write_msg(writer, &DaemonToClient::Ok).await?;
                        if let Err(e) = writer.flush().await {
                            debug!("flush after Detach ack failed: {e}");
                        }
                        break;
                    }
                    other => {
                        warn!("unexpected message during attach for {}: {:?}", name, other);
                    }
                }
            },
        }
    }

    debug!("attach streaming for {} ended", name);
    Ok(())
}

fn apply_resize(entry: &AgentEntry, size: TermSize) {
    if let Err(e) = entry.pty.resize(Size::new(size.rows, size.cols)) {
        debug!(
            "resize to {}x{} failed for {} (PTY likely gone): {e}",
            size.rows, size.cols, entry.name
        );
    }
}
