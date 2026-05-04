//! `pswarm completions <shell>`: print a shell completion script to stdout.
//!
//! Currently supports fish only. Hand-written rather than generated from
//! clap, so dynamic completion (agent names from `pswarm ls --names`)
//! integrates seamlessly with the static completion of subcommands and
//! flags.

use anyhow::{Result, bail};

const FISH: &str = include_str!("completions/pswarm.fish");

pub async fn run(shell: String) -> Result<()> {
    match shell.as_str() {
        "fish" => {
            print!("{FISH}");
            Ok(())
        }
        other => bail!("unsupported shell: {other} (only `fish` is supported)"),
    }
}
