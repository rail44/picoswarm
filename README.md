# picoswarm

A lightweight orchestrator for running multiple Claude Code (and similar
CLI coding agent) sessions in parallel. The CLI binary is `pswarm`.

> **Status: WIP / pre-alpha.** Linux-only. Single-user, single-host.
> The wire protocol, on-disk state, and CLI surface are all unstable
> and may break between commits without notice. Not recommended for
> anyone but the author yet.

## What it does today

A long-running daemon owns each agent's PTY. The CLI starts agents,
attaches and detaches, sends input without attaching, snapshots
output, lists / kills agents, and sweeps dead ones. Detach is
`Ctrl-\`. Agents inherit both the cwd and the env you invoked
`pswarm run` from, which is what makes `cd <worktree> && pswarm run
…` Just Work for parallel git worktrees.

```
pswarm run -d feat-x -- claude --dangerously-skip-permissions
pswarm ls
pswarm attach feat-x        # Ctrl-\ to detach
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

### Daemon logs

Tracing output is rotated daily at
`$XDG_STATE_HOME/picoswarm/daemon.YYYY-MM-DD.log` (7-day retention).
A small `daemon.crash.log` next to it captures stdout / stderr from
the daemonized process — typically empty unless the daemon panics.

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
