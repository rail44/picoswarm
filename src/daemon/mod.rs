//! Entrypoint for the `pswarm daemon` subcommand.
//!
//! Holds PTYs and serves clients over a Unix socket.

mod lifecycle;
mod output_session;
mod registry;
mod server;
mod session;
mod spawner;

pub use session::validate_agent_name;

pub fn run() -> anyhow::Result<()> {
    lifecycle::start()
}
