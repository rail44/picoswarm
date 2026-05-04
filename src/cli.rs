use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "pswarm",
    version,
    about = "picoswarm: a lightweight orchestrator for parallel CLI coding agents"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run a new agent under the picoswarm daemon.
    ///
    /// The agent inherits the current working directory; `cd` first if
    /// you want it to start somewhere else (a git worktree, a sibling
    /// repo, …). By default, attaches to the new agent in the current
    /// terminal (mirroring `docker run`); use `-d` / `--detach` to spawn
    /// without attaching.
    Run {
        /// Human-friendly name for the agent.
        name: String,
        /// Spawn the agent and return immediately instead of attaching.
        #[arg(long, short)]
        detach: bool,
        /// Command and arguments to run. Defaults to `claude`. Use `--` to separate from pswarm flags.
        #[arg(last = true)]
        cmd: Vec<String>,
    },
    /// List currently registered agents.
    Ls {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Attach to an existing agent in the current terminal.
    Attach {
        /// Name of the agent to attach to.
        name: String,
    },
    /// Remove an agent, terminating its process if it is still running.
    Rm {
        /// Name of the agent to remove.
        name: String,
        /// Skip the graceful SIGTERM and kill the process immediately.
        #[arg(long, short)]
        force: bool,
    },
    /// Report daemon status and adapter availability.
    Doctor,
    /// Manage the picoswarm daemon lifecycle.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Subcommand)]
pub enum DaemonCommand {
    /// Run the picoswarm daemon in the foreground or detached. Typically
    /// auto-started by clients; rarely invoked manually.
    Start,
    /// Ask the running daemon to shut down gracefully.
    Stop,
    /// Stop the running daemon (if any) and start a fresh one. Useful
    /// after rebuilding the binary so the new code takes effect.
    Restart,
}
