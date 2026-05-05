use clap::{CommandFactory, Parser};
use picoswarm::{cli, client, daemon};

fn main() -> anyhow::Result<()> {
    // When the binary is invoked via `COMPLETE=<shell> pswarm` (the
    // dynamic-completion entry point set up by `pswarm completions`),
    // generate candidates and exit before any other startup work
    // touches stdout. No-op for normal CLI invocations.
    clap_complete::CompleteEnv::with_factory(cli::Cli::command).complete();

    let parsed = cli::Cli::parse();

    match parsed.command {
        cli::Command::Daemon { command } => match command {
            cli::DaemonCommand::Start => daemon::run(),
            cli::DaemonCommand::Stop => {
                init_client_tracing();
                client_runtime().block_on(client::daemon::stop())
            }
            cli::DaemonCommand::Restart => {
                init_client_tracing();
                client_runtime().block_on(client::daemon::restart())
            }
        },
        cli::Command::Doctor => {
            init_client_tracing();
            client_runtime().block_on(client::doctor::run())
        }
        cli::Command::Run { name, detach, cmd } => {
            init_client_tracing();
            client_runtime().block_on(client::run::run(name, detach, cmd))
        }
        cli::Command::Ls { json, names } => {
            init_client_tracing();
            client_runtime().block_on(client::ls::run(json, names))
        }
        cli::Command::Rm { name, force } => {
            init_client_tracing();
            client_runtime().block_on(client::rm::run(name, force))
        }
        cli::Command::Attach { name } => {
            init_client_tracing();
            client_runtime().block_on(client::attach::run(name))
        }
        cli::Command::Clean => {
            init_client_tracing();
            client_runtime().block_on(client::clean::run())
        }
        cli::Command::Cwd { name } => {
            init_client_tracing();
            client_runtime().block_on(client::cwd::run(name))
        }
        cli::Command::Completions { shell } => {
            init_client_tracing();
            client::completions::run(&shell)
        }
        cli::Command::Send { name, text } => {
            init_client_tracing();
            client_runtime().block_on(client::send::run(name, text))
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
    // Runtime build only fails if the OS won't give us threads/timers.
    // Nothing else in the binary can recover from that, so panic with
    // a clear message at startup.
    #[allow(clippy::expect_used)]
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
}
