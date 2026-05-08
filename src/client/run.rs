//! `pswarm run`: ask the daemon to spawn a new agent, then attach to it
//! by default (or just print the assigned id when `--detach` is set,
//! or fire the `[spawn]` template from `config.toml` when `--spawn` is
//! set).

use anyhow::{Context, Result, anyhow, bail};

use crate::client::{attach, connection};
use crate::config;
use crate::protocol::{self, ClientToDaemon, DaemonToClient, RunRequest, TermSize};

pub async fn run(name: String, detach: bool, spawn: bool, cmd: Vec<String>) -> Result<()> {
    validate_name(&name)?;

    // Resolve the spawn template up front so a config error is reported
    // *before* we register the agent — otherwise we'd leave a stranded
    // agent the user has to clean up.
    let spawn_argv = if spawn {
        let cfg = config::load()?;
        let template = cfg.spawn.ok_or_else(|| {
            anyhow!("--spawn requested but no [spawn] section in ~/.config/picoswarm/config.toml")
        })?;
        Some(config::resolve_spawn_argv(&template.command, &name)?)
    } else {
        None
    };

    // The agent is spawned independently of this client process. Use a
    // sensible default; the attaching client (whether this process when
    // detach=false, or a later `pswarm attach`) will resize the PTY when
    // it connects.
    let initial_size = TermSize { rows: 24, cols: 80 };

    // The agent inherits the directory the user ran `pswarm run` from.
    // Without this, portable-pty falls back to $HOME because the daemon's
    // own cwd is `/` after daemonize. If the user wants a different
    // directory, they `cd` there first — picoswarm doesn't manage
    // worktrees itself.
    let cwd = std::env::current_dir().ok();

    // Propagate the client's env so `pswarm run` behaves like the
    // user's shell extended with persistence (tmux model). The daemon
    // applies this on top of its own env and then re-sets
    // PSWARM_DAEMON=1 last so the client cannot override it.
    let env: Vec<(String, String)> = std::env::vars().collect();

    let request = RunRequest {
        name: name.clone(),
        cmd,
        cwd,
        env,
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

    if let Some(argv) = spawn_argv {
        // α (transparent argv printout): show the user exactly what we
        // are about to exec. Goes to stderr so it doesn't pollute
        // scripts that capture stdout (the assigned name/id).
        eprintln!("[spawn] {}", config::display_argv(&argv));
        // L1 (no shell): exec argv directly.
        // Fork-and-forget: terminal-launching commands like `kitty @
        // launch` are short-lived (talk to the kitty server, exit), and
        // anything longer-lived gets reaped by init when this process
        // exits a moment later.
        let _child = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .spawn()
            .with_context(|| {
                format!(
                    "failed to launch [spawn] template (argv[0] = {:?}). Is the binary on $PATH?",
                    argv[0]
                )
            })?;
        println!("started {returned_name} ({id})");
        Ok(())
    } else if detach {
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
