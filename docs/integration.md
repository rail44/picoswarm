# Integrating picoswarm with your environment

picoswarm provides agent primitives (`pswarm run / ls / attach / send / cwd / rm / clean / doctor / completions`). Anything else — opening windows, popping notifications, updating a status bar — is the user's responsibility, composed via your shell and your terminal's own CLI.

This document records the integration patterns that work well and the contract that future built-in hooks (if any are ever added) will follow.

## Philosophy

Composition over abstraction. The picoswarm CLI is the integration surface; everything else is glue you write.

Why not a `PaneHost` abstraction inside picoswarm?

- Modern terminals (kitty, wezterm) ship rich CLI surfaces (`kitty @`, `wezterm cli`). Wrapping a subset of them in a Rust trait inside picoswarm always loses expressiveness.
- Each terminal has its own concept of "tab vs window vs split", different match predicates (title, id, regex), different focus rules. A common abstraction can't capture them all without picking a lowest common denominator.
- A 5-line shell function gets the user exactly the integration they want, without forcing every other user to wear the same shape.
- If the project absorbs window-management responsibility once, every new feature ("auto-focus on attach", "close pane on exit", "split view of two agents") becomes an in-tree feature request. Keeping picoswarm out of this layer keeps its scope bounded.

See `docs/decision-log.md` entry 13 for the full rationale.

## Recipes

### kitty: open a new tab and attach (fish)

```fish
function pswt --description 'pswarm run + attach in a new kitty tab'
    set name $argv[1]
    pswarm run -d $argv
    or return
    kitty @ launch --type=tab --tab-title $name pswarm attach $name
end
```

Usage: `pswt feat-x -- claude`. Spawns the agent, opens a new kitty tab titled `feat-x`, attaches there. The original shell stays free.

### kitty: focus an existing agent's tab

```fish
function pswf --description 'focus the kitty tab attached to an agent'
    kitty @ focus-tab --match title:$argv[1]
end
```

### tmux: spawn into a new window

```bash
pswt() {
    pswarm run -d "$@" && tmux new-window -n "$1" "pswarm attach $1"
}
```

### wezterm: spawn into a new tab

```fish
function pswt
    set name $argv[1]
    pswarm run -d $argv
    or return
    wezterm cli spawn --new-window=false -- pswarm attach $name
end
```

### Notifications on agent exit (manual)

There's no built-in agent-exit hook today. If you want one, poll `pswarm ls` and react when an agent transitions to `dead`:

```fish
while sleep 5
    pswarm ls --json | jq -r '.[] | select(.status=="dead") | .name' | while read name
        notify-send "agent $name exited"
        pswarm rm $name
    end
end
```

(Crude but illustrative. A real implementation would deduplicate so the same exit isn't notified repeatedly.)

## Future hook contract (not yet implemented)

If picoswarm ever absorbs an explicit hook — most likely use case: an agent itself wanting to launch another agent into a pane it doesn't own — the contract will be:

- Configured via `config.toml` (`[hooks]` section), **not** via environment variables. Env propagates into spawned agents and would create a privilege-escalation surface.
- The hook value is the absolute path of an executable script, not a shell command string. picoswarm invokes it as `Command::new(path).arg(<agent-name>)` with no shell interpretation.
- The script's job is to launch `pswarm attach <agent-name>` in whatever pane the user wants. picoswarm does not interpret the script's output; non-zero exit surfaces as a clear error.

This is intentionally minimal: one file path, one positional argument, one stdout side channel. Anything fancier (status callbacks, structured events, etc.) goes through the existing `pswarm` CLI.

## Why no env-based hook

picoswarm's daemon spawns agents that inherit the daemon's full environment (so users get their shell env in agents — see `src/daemon/session.rs`). Any `PSWARM_*_HOOK` variable would propagate into every agent and into every subshell those agents start. Once an agent invokes `pswarm` recursively, the recursive call sees the same hook value and runs it again. That's a foot-gun, not a feature.

Config-file-based hooks dodge this: the daemon reads them once at startup, doesn't broadcast them to children, and the hook script is a stable on-disk artifact the user can audit.
