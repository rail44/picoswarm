//! Entrypoint for the `pswarm daemon` subcommand.
//!
//! Holds PTYs and serves clients over a Unix socket.

mod lifecycle;
mod registry;
mod server;
mod session;

pub fn run() -> anyhow::Result<()> {
    lifecycle::start()
}
