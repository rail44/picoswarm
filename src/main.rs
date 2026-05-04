use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[allow(dead_code)]
mod protocol;

#[derive(Parser)]
#[command(
    name = "pswarm",
    version,
    about = "picoswarm: a lightweight orchestrator for parallel CLI coding agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a new agent under the picoswarm daemon.
    Run {
        /// Human-friendly name for the agent.
        name: String,
        /// Working directory the agent should start in (e.g. a git worktree).
        #[arg(long)]
        worktree: Option<PathBuf>,
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

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Run { .. }
        | Command::Ls { .. }
        | Command::Attach { .. }
        | Command::Rm { .. }
        | Command::Doctor
        | Command::Daemon => {
            anyhow::bail!("not implemented yet");
        }
    }
}
