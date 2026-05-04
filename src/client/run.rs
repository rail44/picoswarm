//! `pswarm run`: ask the daemon to spawn a new agent, then attach to it
//! by default (or just print the assigned id when `--detach` is set).

use anyhow::{Result, anyhow, bail};
use std::path::PathBuf;

use crate::client::{attach, connection};
use crate::protocol::{self, ClientToDaemon, DaemonToClient, RunRequest, TermSize};

pub async fn run(
    name: String,
    worktree: Option<PathBuf>,
    detach: bool,
    cmd: Vec<String>,
) -> Result<()> {
    validate_name(&name)?;

    // The agent is spawned independently of this client process. Use a
    // sensible default; the attaching client (whether this process when
    // detach=false, or a later `pswarm attach`) will resize the PTY when
    // it connects.
    let initial_size = TermSize { rows: 24, cols: 80 };

    let request = RunRequest {
        name: name.clone(),
        cmd,
        cwd: worktree,
        env: Vec::new(),
        initial_size,
    };

    let mut stream = connection::connect_with_handshake().await?;
    let assigned = {
        let (mut reader, mut writer) = stream.split();
        protocol::write_msg(&mut writer, &ClientToDaemon::Run(request)).await?;
        match protocol::read_msg::<DaemonToClient, _>(&mut reader).await? {
            DaemonToClient::RunResult { id, name } => (id, name),
            DaemonToClient::Error { code, message } => {
                bail!("daemon refused: {code:?} {message}")
            }
            other => bail!("unexpected response: {other:?}"),
        }
    };
    drop(stream);

    let (id, returned_name) = assigned;

    if detach {
        println!("started {returned_name} ({id})");
        Ok(())
    } else {
        // Default: attach to the freshly-spawned agent.
        attach::run(returned_name).await
    }
}

fn validate_name(name: &str) -> Result<()> {
    let len = name.chars().count();
    if !(1..=64).contains(&len) {
        return Err(anyhow!("name must be 1-64 characters"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(anyhow!(
            "name may only contain a-z, A-Z, 0-9, '.', '_', '-'"
        ));
    }
    Ok(())
}
