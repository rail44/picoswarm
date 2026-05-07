//! `pswarm ls`: list agents the daemon currently knows about.

use anyhow::{Result, bail};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::client::connection;
use crate::protocol::{self, AgentStatus, AgentSummary, ClientToDaemon, DaemonToClient, Event};

pub async fn run(json: bool, names: bool) -> Result<()> {
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
    } else if names {
        for a in &agents {
            println!("{}", a.name);
        }
    } else if agents.is_empty() {
        println!("(no agents)");
    } else {
        let now = now_unix();
        for a in &agents {
            println!(
                "{}\t{}\t{}\t{}",
                a.name,
                status_str(a.status),
                a.id,
                event_cell(a.last_event, now),
            );
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
    last_event: Option<LastEventView>,
}

#[derive(Serialize)]
struct LastEventView {
    event: &'static str,
    at: i64,
}

impl<'a> From<&'a AgentSummary> for AgentView<'a> {
    fn from(a: &'a AgentSummary) -> Self {
        Self {
            id: &a.id,
            name: &a.name,
            status: status_str(a.status),
            cwd: a.cwd.as_deref(),
            created_at: a.created_at,
            last_event: a.last_event.map(|(e, t)| LastEventView {
                event: event_str(e),
                at: t,
            }),
        }
    }
}

const fn status_str(s: AgentStatus) -> &'static str {
    match s {
        AgentStatus::Running => "running",
        AgentStatus::Dead => "dead",
        AgentStatus::Registered => "registered",
    }
}

const fn event_str(e: Event) -> &'static str {
    match e {
        Event::Idle => "idle",
        Event::Attention => "attention",
        Event::Exit => "exit",
    }
}

fn event_cell(last: Option<(Event, i64)>, now: i64) -> String {
    last.map_or_else(String::new, |(e, t)| {
        let age = (now - t).max(0);
        format!("{} ({})", event_str(e), format_age(age))
    })
}

fn format_age(secs: i64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}
