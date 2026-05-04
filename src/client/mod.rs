//! Client-side subcommands: connect to the daemon (auto-starting it
//! if necessary) and issue requests.

pub mod attach;
mod connection;
pub mod daemon;
pub mod doctor;
pub mod ls;
pub mod rm;
pub mod run;
