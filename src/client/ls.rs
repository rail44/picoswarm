//! `pswarm ls`: list agents the daemon currently knows about.

use anyhow::{Result, bail};
use serde::Serialize;

use crate::client::connection;
use crate::protocol::{self, AgentStatus, AgentSummary, ClientToDaemon, DaemonToClient};

pub async fn run(json: bool) -> Result<()> {
    let mut stream = connection::connect_with_handshake().await?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Ls).await?;
    let agents = match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
        DaemonToClient::AgentList(v) => v,
        DaemonToClient::Error { code, message } => bail!("{code:?}: {message}"),
        other => bail!("unexpected response: {other:?}"),
    };

    if json {
        let view: Vec<AgentView<'_>> = agents.iter().map(AgentView::from).collect();
        println!("{}", serde_json::to_string(&view)?);
    } else if agents.is_empty() {
        println!("(no agents)");
    } else {
        for a in &agents {
            println!("{}\t{}\t{}", a.name, status_str(a.status), a.id);
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct AgentView<'a> {
    id: &'a uuid::Uuid,
    name: &'a str,
    status: &'static str,
    cwd: Option<&'a std::path::Path>,
    created_at: i64,
}

impl<'a> From<&'a AgentSummary> for AgentView<'a> {
    fn from(a: &'a AgentSummary) -> Self {
        Self {
            id: &a.id,
            name: &a.name,
            status: status_str(a.status),
            cwd: a.cwd.as_deref(),
            created_at: a.created_at,
        }
    }
}

fn status_str(s: AgentStatus) -> &'static str {
    match s {
        AgentStatus::Running => "running",
        AgentStatus::Idle => "idle",
        AgentStatus::Dead => "dead",
        AgentStatus::Unknown => "unknown",
    }
}
