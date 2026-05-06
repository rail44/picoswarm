use clap::{Parser, Subcommand};
use clap_complete::engine::ArgValueCandidates;

use crate::client::completions::agent_name_candidates;

#[derive(Parser)]
#[command(
    name = "pswarm",
    version,
    about = "picoswarm: a lightweight orchestrator for parallel CLI coding agents"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run a new agent under the picoswarm daemon.
    ///
    /// The agent inherits the current working directory; `cd` first if
    /// you want it to start somewhere else (a git worktree, a sibling
    /// repo, …). By default, attaches to the new agent in the current
    /// terminal (mirroring `docker run`); use `-d` / `--detach` to spawn
    /// without attaching.
    Run {
        /// Human-friendly name for the agent.
        name: String,
        /// Spawn the agent and return immediately instead of attaching.
        #[arg(long, short)]
        detach: bool,
        /// Command and arguments to run. Defaults to `claude`. Use `--` to separate from pswarm flags.
        #[arg(last = true)]
        cmd: Vec<String>,
    },
    /// List currently registered agents.
    Ls {
        /// Emit machine-readable JSON.
        #[arg(long, conflicts_with = "names")]
        json: bool,
        /// Emit only the agent names, one per line. Convenient for shell
        /// completion or piping into `xargs`.
        #[arg(long)]
        names: bool,
    },
    /// Attach to an existing agent in the current terminal.
    Attach {
        /// Name of the agent to attach to.
        #[arg(add = ArgValueCandidates::new(agent_name_candidates))]
        name: String,
    },
    /// Remove an agent, terminating its process if it is still running.
    Rm {
        /// Name of the agent to remove.
        #[arg(add = ArgValueCandidates::new(agent_name_candidates))]
        name: String,
        /// Skip the graceful SIGTERM and kill the process immediately.
        #[arg(long, short)]
        force: bool,
    },
    /// Report daemon status and adapter availability.
    Doctor,
    /// Remove agents whose process has already exited.
    Clean,
    /// Print the current working directory of a running agent. Useful in
    /// shell wrappers, e.g. `cd (pswarm cwd feat-x)` in fish.
    Cwd {
        /// Name of the agent to query.
        #[arg(add = ArgValueCandidates::new(agent_name_candidates))]
        name: String,
    },
    /// Generate shell completion scripts.
    Completions {
        /// Target shell. Currently only `fish` is supported.
        shell: String,
    },
    /// Write text to a running agent's PTY without attaching.
    ///
    /// If TEXT is omitted, reads from stdin instead. Trailing line
    /// endings are stripped from the input and a single CR (`\r`) is
    /// appended so the agent sees Enter, not just a newline (TUI
    /// agents in raw mode treat LF as "insert newline" and CR as
    /// "submit"). Allowed while a client is attached; the bytes
    /// interleave with the attached client's typing.
    Send {
        /// Name of the target agent.
        #[arg(add = ArgValueCandidates::new(agent_name_candidates))]
        name: String,
        /// Text to send. If omitted, read from stdin.
        text: Option<String>,
    },
    /// Print the agent's recent PTY output (one-shot snapshot of the
    /// ring buffer) without attaching. Read-only — no resize side
    /// effect on the agent.
    View {
        /// Name of the agent to view.
        #[arg(add = ArgValueCandidates::new(agent_name_candidates))]
        name: String,
    },
    /// Record a lifecycle event for the calling agent.
    ///
    /// Used by hooks running in the agent's environment to signal
    /// `idle` (turn complete, ready for input), `attention` (needs
    /// human input out-of-band, e.g. permission prompt), or `exit`
    /// (session ending). The bundled Claude Code plugin under
    /// `plugins/claude-code/` wires Claude's `SessionStart`, `Stop`,
    /// `Notification`, and `SessionEnd` hooks into this command.
    ///
    /// The agent name is read from `$PSWARM_AGENT_NAME` (set
    /// automatically by `pswarm run`); when the variable is unset the
    /// command silently exits 0, so the bundled plugin is a true
    /// no-op outside a pswarm-spawned session. To inject an event
    /// for a specific agent during debug, override the env:
    /// `PSWARM_AGENT_NAME=foo pswarm event idle`.
    ///
    /// Transitional accept-list: as a back-compat shim for plugin
    /// v0.2.0 hooks that still call `pswarm event self <event>`, a
    /// leading literal `self` argument is silently dropped. Will be
    /// removed once no live sessions still hold the v0.2.0 plugin.
    Event {
        /// `<event>`, optionally preceded by the literal `self`
        /// (v0.2.0 compat). 1 or 2 args; first must be `self` if 2.
        #[arg(num_args = 1..=2)]
        args: Vec<String>,
    },
    /// Manage the picoswarm daemon lifecycle.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Subcommand)]
pub enum DaemonCommand {
    /// Run the picoswarm daemon in the foreground or detached. Typically
    /// auto-started by clients; rarely invoked manually.
    Start,
    /// Ask the running daemon to shut down gracefully. Refuses if any
    /// agent is currently attached, unless `--force` is given.
    Stop {
        /// Shut down even if clients are still attached (their sessions
        /// will be terminated).
        #[arg(long, short)]
        force: bool,
    },
    /// Stop the running daemon (if any) and start a fresh one. Useful
    /// after rebuilding the binary so the new code takes effect.
    /// Refuses if any agent is currently attached, unless `--force` is
    /// given.
    Restart {
        /// Restart even if clients are still attached.
        #[arg(long, short)]
        force: bool,
    },
}
