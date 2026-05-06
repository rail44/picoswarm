//! `pswarm event`: record a lifecycle event for the calling agent.
//!
//! The intended caller is a hook running inside an agent's environment
//! — Claude Code's `SessionStart` / `Stop` / `Notification` /
//! `SessionEnd` hooks (via the bundled plugin), or the analogous hooks
//! of other agents.
//!
//! The agent name is read from `$PSWARM_AGENT_NAME` (injected by
//! `pswarm run` at spawn). When the variable is unset, or the daemon
//! is unreachable, or the daemon does not know the resolved name,
//! the command exits 0 silently. This means installing the plugin
//! into a Claude session that is not running under pswarm is a true
//! no-op, and a transient daemon issue never breaks an agent's
//! session. To inject an event for a specific agent during debug,
//! override the env: `PSWARM_AGENT_NAME=foo pswarm event idle`.
//!
//! Transitional shim: a leading literal `self` argument is silently
//! dropped. This keeps live Claude sessions that loaded plugin v0.2.0
//! (which still calls `pswarm event self <event>` from its hooks)
//! from spamming Stop-hook errors. To be removed once no live
//! sessions still hold the v0.2.0 plugin.

use anyhow::{Result, bail};
use clap::ValueEnum;

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient, Event};

const ENV_AGENT_NAME: &str = "PSWARM_AGENT_NAME";
const LEGACY_SELF: &str = "self";

pub async fn run(args: Vec<String>) -> Result<()> {
    let event_str: &str = match args.as_slice() {
        [single] => single,
        [first, second] if first == LEGACY_SELF => second,
        _ => bail!("usage: pswarm event <event>; got {} arguments", args.len()),
    };
    let event = Event::from_str(event_str, true)
        .map_err(|e| anyhow::anyhow!("invalid event {event_str:?}: {e}"))?;

    let Ok(name) = std::env::var(ENV_AGENT_NAME) else {
        // Not running under pswarm: best-effort no-op.
        return Ok(());
    };
    // All errors silent — debug via daemon log (#26) or env override.
    drop(send_event(name, event).await);
    Ok(())
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
