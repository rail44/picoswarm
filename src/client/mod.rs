//! Client-side subcommands: connect to the daemon (auto-starting it
//! if necessary) and issue requests.

pub mod attach;
pub mod clean;
pub mod completions;
mod connection;
pub mod cwd;
pub mod daemon;
pub mod doctor;
pub mod event;
pub mod inbox;
pub mod ls;
pub mod rm;
pub mod run;
pub mod send;
pub mod view;
