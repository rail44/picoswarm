# picoswarm Claude Code plugin

Forwards Claude Code lifecycle events to the `pswarm` daemon so
`pswarm ls` can show whether each Claude agent is idle, asking for
input, or shutting down — without scraping its TUI.

## What it does

| Claude hook | `pswarm event` value | Means |
|-------------|----------------------|-------|
| `SessionStart` | `idle` | Claude has finished booting (or resumed) and is ready for the first input. |
| `Stop` | `idle` | Claude finished a turn and is waiting for the next user input. |
| `Notification` | `attention` | Claude needs human input out-of-band (typically a permission prompt). |
| `SessionEnd` | `exit` | The Claude session is ending. |

`SessionStart` and `Stop` both map to `idle` because both signal
"ready for input" — the only difference is whether it's the first
input of a session or a continuation. Scripts that want to drive an
agent can therefore wait for `last_event=idle` after spawn without
caring which hook produced it.

The hooks invoke `pswarm event <value>` directly — no shell glue.
`pswarm event` reads the agent's name from `$PSWARM_AGENT_NAME`,
which the daemon sets automatically when the agent is spawned by
`pswarm run`. When the variable is absent (i.e. Claude was not
launched by pswarm), the subcommand exits 0 silently so the plugin
is a true no-op outside its intended environment.

## Install

The picoswarm repo is a Claude Code marketplace
(`.claude-plugin/marketplace.json` at the root). Install once:

```sh
claude plugin marketplace add /absolute/path/to/picoswarm
claude plugin install picoswarm@picoswarm
```

After that, every `pswarm run -d <name> -- claude` agent picks up
the plugin automatically. No `--plugin-dir` needed.

```sh
pswarm run -d demo -- claude
pswarm send demo 'say hi'
pswarm send demo ''
pswarm ls
# demo  running  <id>  idle (2s)
```

For one-off or per-session loading without a permanent install,
`claude --plugin-dir /absolute/path/to/picoswarm/plugins/claude-code`
also works.

## Requirements

- The `pswarm` binary on `$PATH` (i.e. `cargo install --path ..` or
  `target/release/pswarm` symlinked into a bin dir). Spawned agents
  inherit the daemon's `PATH`, so this is automatic when the daemon
  itself was started from a `pswarm` on the user's `PATH`.
- The picoswarm daemon running. Auto-started on first contact, so no
  manual setup.
