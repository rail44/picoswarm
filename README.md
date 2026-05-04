# picoswarm

A lightweight orchestrator for running multiple Claude Code (and similar
CLI coding agent) sessions in parallel. The CLI binary is `pswarm`.

> **Status: WIP / pre-alpha.** Linux-only. Single-user, single-host.
> The wire protocol, on-disk state, and CLI surface are all unstable
> and may break between commits without notice. Not recommended for
> anyone but the author yet.

## What it does today

A long-running daemon owns each agent's PTY. The CLI starts agents,
attaches to them, lists / kills them, and lets you sweep dead ones.
Detach is `Ctrl-\`. Agents inherit the cwd you invoked `pswarm run`
from, which is what makes `cd <worktree> && pswarm run …` Just Work
for parallel git worktrees.

```
pswarm run -d feat-x -- claude --dangerously-skip-permissions
pswarm ls
pswarm attach feat-x        # Ctrl-\ to detach
pswarm cwd feat-x           # prints the agent's cwd
pswarm clean                # drop dead entries from the registry
pswarm rm feat-x
pswarm doctor               # daemon status / version / agent count
```

Fish tab-completion (subcommands + agent names):

```
pswarm completions fish > ~/.config/fish/completions/pswarm.fish
```

## Build

```
cargo build --release
./target/release/pswarm doctor
```

The daemon is auto-started on first connection. `pswarm daemon
{start,stop,restart}` controls it explicitly.

See `CLAUDE.md` for the project's design constraints and `docs/plan.md`
for the current scope.

## License

MIT — see `LICENSE`.
