# picoswarm

A lightweight orchestrator for running multiple Claude Code (and similar
CLI coding agent) sessions in parallel. The CLI binary is `pswarm`.

> **Status: WIP / pre-alpha.** The wire protocol, on-disk state, and CLI
> surface may break between commits without notice. Not recommended
> for anyone but the author yet.

## Why?

picoswarm sits at a particular corner of the agent-orchestration
design space. The shape comes from a handful of trade-offs:

- **CLI-first, not TUI-first.** A dashboard in front of every
  operation works well in some workflows; ours wants the
  orchestrator to compose with shell scripts and to be callable by
  the agents themselves, so picoswarm exposes everything as
  one-shot subcommands.

- **Agent-shaped vocabulary.** Running agents directly under tmux
  (`tmux new-session -d -s feat-x …`, `attach`, `send-keys`) is
  perfectly viable; picoswarm just packages the same operations
  with verbs that match what you're doing — `run` / `attach` /
  `send` / `view` / `cwd` / `clean` — and tracks each agent's
  identity, cwd, and lifecycle so you don't have to maintain the
  session-to-agent map yourself.

- **Self-contained PTY daemon, not a wrapper around an external
  session manager.** Running on top of tmux / shpool / etc. is
  lighter on code, but it inherits whatever limits that manager has
  and asks every user to install it.

- **Daemon-owned sessions, not tab-owned ones.** Treating a terminal
  tab as the session is simpler when sessions don't need to outlive
  the terminal; we wanted them to.

- **No bundled window management.** Multiplexer-aware orchestrators
  can offer a tighter out-of-the-box experience inside one
  multiplexer; picoswarm leaves the window/pane layer to your shell
  and your terminal's CLI (`kitty @ launch`, `tmux new-window`,
  `wezterm cli spawn`, …) — see `docs/integration.md`.

- **PTY-wrap the agent's TUI, not its SDK or protocol.** Vendor SDKs
  and protocols (Claude Code SDK, ACP, etc.) are the right surface
  for fine-grained programmatic control. The trade is that the
  coding-agent ecosystem currently moves fastest through the
  official user-facing TUIs, and SDK-level surfaces tend to lag.
  picoswarm runs whatever TUI binary the user installs, so upstream
  improvements arrive without us doing anything.

## What it does today

```
pswarm run -d feat-x -- claude --dangerously-skip-permissions
pswarm ls
pswarm attach feat-x        # Ctrl-\ to detach (configurable, see below)
pswarm view feat-x          # one-shot snapshot of recent output (no resize side effect)
pswarm send feat-x "go"     # send text without attaching (or pipe via stdin)
pswarm cwd feat-x           # print the agent's cwd
pswarm clean                # drop dead entries from the registry
pswarm rm feat-x
pswarm doctor               # daemon status / version / agent count
```

`pswarm daemon stop` and `pswarm daemon restart` refuse to terminate
the daemon while any agent is attached; pass `-f` / `--force` to
override.

### Window management

picoswarm itself does not manage terminal tabs / windows / panes.
Compose its primitives with your terminal's CLI (`kitty @ launch`,
`tmux new-window`, `wezterm cli spawn`, …). See `docs/integration.md`
for working recipes (kitty, tmux, wezterm).

### Shell completion

Dynamic completion (subcommand names + live agent-name candidates)
ships for bash, zsh, fish, elvish, and powershell. Install the
single sourcing line for your shell:

```
pswarm completions fish > ~/.config/fish/completions/pswarm.fish
# bash:   pswarm completions bash >> ~/.bashrc
# zsh:    pswarm completions zsh  >> ~/.zshrc
```

### Configuration

Optional file at `~/.config/picoswarm/config.toml` (created by you;
not generated). Two sections today:

```toml
[keybind]
detach = "ctrl+\\"   # default; or 'ctrl+\' as a TOML literal string

[spawn]
# argv array, exec'd directly (no shell). {name} is substituted per
# element. Fired by `pswarm run --spawn <name>`. Unknown placeholders
# error at parse time.
command = ["kitty", "@", "launch", "--type=tab", "--tab-title", "{name}",
           "pswarm", "attach", "{name}"]
```

`[keybind] detach`: `ctrl+<char>` form, single ASCII character.
Named keys (`space`, `enter`) and other notations (`C-\`, `<C-\>`,
…) are not yet supported — extend `src/config.rs::parse_detach_key`
if you need them.

`[spawn] command`: the array runs as you, with no shell
interpretation, every time you call `pswarm run --spawn`. This is
one of many command-execution surfaces a process with write access
to your home directory can already use (`.bashrc`, `$PATH`, …); it
is *not* specially defended. See `docs/decision-log.md` #22 for the
threat model and the per-directory-config deferral.

### Daemon logs

Tracing output is rotated daily at
`$XDG_STATE_HOME/picoswarm/daemon.YYYY-MM-DD.log` (7-day retention).
A small `daemon.crash.log` next to it captures stdout / stderr from
the daemonized process — typically empty unless the daemon panics.

Hook / plugin events go through this log too: every `pswarm event`
arrival is recorded (success at `debug!`, rejection at `warn!`), so
when a plugin's hook seems silent, `tail -f` of the daily log is the
first place to look. Pass `RUST_LOG=picoswarm=debug` to the daemon
process to see the success path as well.

## Build

```
cargo build --release
./target/release/pswarm doctor
```

The daemon is auto-started on first connection. `pswarm daemon
{start,stop,restart}` controls it explicitly.

See `CLAUDE.md` for the project's design constraints, `docs/plan.md`
for the current scope, `docs/protocol.md` for the wire protocol, and
`docs/decision-log.md` for the rationale behind major choices.

## License

MIT — see `LICENSE`.
