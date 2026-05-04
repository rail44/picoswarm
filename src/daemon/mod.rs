//! Entrypoint for the `pswarm daemon` subcommand.
//!
//! This module owns the long-lived process that holds PTYs and serves
//! clients over a Unix socket. The current implementation does the
//! daemonize dance and accepts connections, but only Ping/Pong is wired
//! through; the remaining message handlers are stubs.

mod lifecycle;
mod server;

pub fn run() -> anyhow::Result<()> {
    lifecycle::start()
}
