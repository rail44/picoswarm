//! Client-side subcommands: connect to the daemon (auto-starting it
//! if necessary) and issue requests.

mod connection;
pub mod doctor;
pub mod ls;
pub mod rm;
pub mod run;
