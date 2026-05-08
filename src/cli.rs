use clap::{Parser, Subcommand};
use clap_complete::engine::ArgValueCandidates;

use crate::client::completions::agent_name_candidates;
use crate::protocol::Event;

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
        /// Spawn the agent in the background, then fire the `[spawn]
        /// command` template from `config.toml` (typically opens a new
        /// terminal tab/split that attaches to the agent). Mutually
        /// exclusive with `--detach`.
        #[arg(long, conflicts_with = "detach")]
        spawn: bool,
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
    Event {
        /// The lifecycle event to record.
        #[arg(value_enum)]
        event: Event,
    },
    /// Register the calling session as a virtual (PTY-less) agent.
    ///
    /// Used by drivers — Claude sessions that orchestrate other agents
    /// — to take a unique identity in the registry, get an inbox, and
    /// record lifecycle events, without the daemon spawning a child
    /// process. The entry shows in `pswarm ls` with status
    /// `registered`; the PTY-bound subcommands (`send`, `view`,
    /// `attach`, `cwd`) reject it with a clear error. Remove with
    /// `pswarm rm <name>`.
    Register {
        /// Unique name for the registered agent. Same constraints as
        /// `pswarm run` names: 1–64 chars, ASCII alphanumeric plus
        /// `.`, `_`, `-`, and not the literal `self`.
        name: String,
    },
    /// Inter-agent message inbox (file-backed; no daemon mediation).
    ///
    /// Each agent has an append-only JSON Lines file at
    /// `$XDG_STATE_HOME/picoswarm/inbox/<name>.jsonl`. `post` writes
    /// one message; `read` emits unread messages, advancing a sidecar
    /// `<name>.cursor` file. Multi-agent orchestration patterns can
    /// wrap `pswarm inbox read --follow` in Claude Code's `Monitor`
    /// tool to receive notifications mid-conversation; non-Claude
    /// agents poll at turn boundaries.
    Inbox {
        #[command(subcommand)]
        command: InboxCommand,
    },
    /// Manage the picoswarm daemon lifecycle.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Subcommand)]
pub enum InboxCommand {
    /// Post a message to an agent's inbox.
    ///
    /// Sender identity comes from `$PSWARM_AGENT_NAME` when set
    /// (i.e. inside a pswarm-spawned agent), otherwise `--from
    /// <label>` is required. The body may be passed as a positional
    /// argument or piped in via stdin.
    Post {
        /// Recipient agent name.
        #[arg(add = ArgValueCandidates::new(agent_name_candidates))]
        to: String,
        /// Message body. If omitted, read from stdin.
        body: Option<String>,
        /// Sender label. Defaults to `$PSWARM_AGENT_NAME`. Required
        /// when running outside a pswarm-spawned agent (e.g. from a
        /// driver Claude session that uses pswarm to orchestrate
        /// other agents).
        #[arg(long)]
        from: Option<String>,
    },
    /// Read unread messages from the calling agent's inbox.
    ///
    /// The agent name is read from `$PSWARM_AGENT_NAME`; outside a
    /// pswarm-spawned context the command exits 0 silently so
    /// wrappers (e.g. Claude Code's `Monitor` tool on `pswarm inbox
    /// read --follow`) do not error. With `--follow`, stays running
    /// and streams new messages as they arrive (poll-based, ~100 ms).
    Read {
        /// Stay running and stream new messages as they arrive.
        #[arg(long, short)]
        follow: bool,
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
