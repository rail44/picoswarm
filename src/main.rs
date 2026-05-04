use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "pswarm", version, about = "picoswarm: a lightweight orchestrator for parallel CLI coding agents")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Spawn a new agent under the picoswarm daemon.
    New {
        /// Human-friendly name for the agent.
        name: String,
        /// Working directory the agent should start in (e.g. a git worktree).
        #[arg(long)]
        worktree: Option<String>,
        /// Command to run as the agent. Defaults to `claude`.
        #[arg(long)]
        cmd: Option<String>,
    },
    /// List currently registered agents.
    Ls,
    /// Attach to an existing agent in the current terminal.
    Attach {
        /// Name of the agent to attach to.
        name: String,
    },
    /// Kill an agent and remove it from the registry.
    Kill {
        /// Name of the agent to kill.
        name: String,
    },
    /// Report daemon status and adapter availability.
    Doctor,
    /// Run the picoswarm daemon (typically auto-started; rarely invoked manually).
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
        Command::New { .. }
        | Command::Ls
        | Command::Attach { .. }
        | Command::Kill { .. }
        | Command::Doctor
        | Command::Daemon => {
            anyhow::bail!("not implemented yet");
        }
    }
}
