//! `pswarm event`: record a lifecycle event for an agent.
//!
//! The intended caller is a hook running inside an agent's environment
//! — Claude Code's `Stop` / `Notification` / `SessionEnd` hooks (via
//! the bundled plugin), or the analogous hooks of other agents.
//!
//! Two forms:
//!
//! - `pswarm event <name> <event>` — explicit. Surfaces every error
//!   (connection, NotFound, etc.).
//! - `pswarm event self <event>` — best-effort. The name is resolved
//!   from `$PSWARM_AGENT_NAME`; if that is missing, or the daemon is
//!   unreachable, or the daemon does not know the resolved name, the
//!   command exits 0 silently. This way installing the plugin into a
//!   Claude session that is not running under `pswarm run` never
//!   surfaces an error to the user.

use anyhow::{Result, bail};

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient, Event};

const SELF_KEYWORD: &str = "self";
const ENV_AGENT_NAME: &str = "PSWARM_AGENT_NAME";

pub async fn run(name: String, event: Event) -> Result<()> {
    let is_self = name == SELF_KEYWORD;
    let resolved = if is_self {
        match std::env::var(ENV_AGENT_NAME) {
            Ok(n) => n,
            // Plugin loaded but Claude was not launched by pswarm:
            // best-effort no-op, never break the user's session.
            Err(_) => return Ok(()),
        }
    } else {
        name
    };

    if is_self {
        // Hook context: every error is silent.
        drop(send_event(resolved, event).await);
        Ok(())
    } else {
        send_event(resolved, event).await
    }
}

async fn send_event(name: String, event: Event) -> Result<()> {
    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Event { name, event }).await?;
    match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::Ok => Ok(()),
        DaemonToClient::Error { code, message } => bail!("event failed: {code:?} {message}"),
        other => bail!("unexpected response: {other:?}"),
    }
}
