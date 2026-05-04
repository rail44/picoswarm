use clap::Parser;

mod cli;
mod client;
mod daemon;
mod paths;
mod protocol;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Daemon => daemon::run(),
        cli::Command::Doctor => {
            init_client_tracing();
            client_runtime().block_on(client::doctor::run())
        }
        cli::Command::Run {
            name,
            worktree,
            cmd,
        } => {
            init_client_tracing();
            client_runtime().block_on(client::run::run(name, worktree, cmd))
        }
        cli::Command::Ls { json } => {
            init_client_tracing();
            client_runtime().block_on(client::ls::run(json))
        }
        cli::Command::Rm { name, force } => {
            init_client_tracing();
            client_runtime().block_on(client::rm::run(name, force))
        }
        cli::Command::Attach { .. } => {
            init_client_tracing();
            anyhow::bail!("attach is not implemented yet")
        }
    }
}

fn init_client_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();
}

fn client_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
}
