use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    /// By default, attaches to the new agent in the current terminal
    /// (mirroring `docker run`). Use `-d` / `--detach` to spawn without
    /// attaching.
    Run {
        /// Human-friendly name for the agent.
        name: String,
        /// Working directory the agent should start in (e.g. a git worktree).
        #[arg(long)]
        worktree: Option<PathBuf>,
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
    /// Run the picoswarm daemon (typically auto-started; rarely invoked manually).
    #[command(hide = true)]
    Daemon,
}
