//! `pswarm completions <shell>`: print the install snippet that wires
//! the user's shell to `pswarm`'s built-in dynamic completion engine.
//!
//! The actual completion candidates (subcommands, flags, agent names)
//! are produced at runtime by `clap_complete::CompleteEnv`, which the
//! binary handles in `main()` when invoked via `COMPLETE=<shell>
//! pswarm`. The install snippet is just the one-liner that tells the
//! shell to call back into the binary for completions.
//!
//! This module also exposes [`agent_name_candidates`], the dynamic
//! completer attached to subcommand args that take an agent name
//! (`attach`, `rm`, `cwd`, `send`).

use anyhow::{Result, bail};
use clap_complete::CompletionCandidate;

use crate::client::connection;
use crate::protocol::{self, ClientToDaemon, DaemonToClient};

pub async fn run(shell: String) -> Result<()> {
    let snippet = match shell.as_str() {
        "bash" => "source <(COMPLETE=bash pswarm)\n",
        "zsh" => "source <(COMPLETE=zsh pswarm)\n",
        "fish" => "COMPLETE=fish pswarm | source\n",
        "elvish" => "eval (E:COMPLETE=elvish pswarm | slurp)\n",
        "powershell" => "COMPLETE=powershell pswarm | Out-String | Invoke-Expression\n",
        other => {
            bail!("unsupported shell: {other} (supported: bash, zsh, fish, elvish, powershell)")
        }
    };
    print!("{snippet}");
    Ok(())
}

/// Dynamic completion source for agent-name args. Attached via
/// `#[arg(add = ArgValueCandidates::new(agent_name_candidates))]` in
/// `cli.rs` on `attach` / `rm` / `cwd` / `send` name positions.
///
/// Runs synchronously in the completion path. Any failure to reach the
/// daemon (not running, socket missing, protocol mismatch, …) returns
/// an empty candidate list — completion just degrades to "no
/// suggestions" rather than spewing errors at the user.
pub fn agent_name_candidates() -> Vec<CompletionCandidate> {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return Vec::new(),
    };
    runtime.block_on(fetch_agent_names()).unwrap_or_default()
}

async fn fetch_agent_names() -> Option<Vec<CompletionCandidate>> {
    // Crucially: do NOT auto-spawn the daemon from the completion
    // path. If the daemon isn't running, completion silently returns
    // empty rather than launching a long-lived process behind a tab
    // press.
    let mut stream = connection::connect_no_spawn().await.ok()?;
    let (mut reader, mut writer) = stream.split();
    protocol::write_msg(&mut writer, &ClientToDaemon::Ls)
        .await
        .ok()?;
    let agents = match protocol::read_msg::<DaemonToClient, _>(&mut reader)
        .await
        .ok()?
    {
        DaemonToClient::AgentList(v) => v,
        _ => return None,
    };
    Some(
        agents
            .into_iter()
            .map(|a| CompletionCandidate::new(a.name))
            .collect(),
    )
}
