# picoswarm fish completions.
#
# Install:  pswarm completions fish > ~/.config/fish/completions/pswarm.fish

# Helper: list current agent names. Silent on failure so completion does
# not spew errors when the daemon isn't reachable.
function __pswarm_agent_names
    pswarm ls --names 2>/dev/null
end

# Top-level subcommands.
complete -c pswarm -n __fish_use_subcommand -f -a run         -d 'Run a new agent (and attach)'
complete -c pswarm -n __fish_use_subcommand -f -a ls          -d 'List registered agents'
complete -c pswarm -n __fish_use_subcommand -f -a attach      -d 'Attach to a running agent'
complete -c pswarm -n __fish_use_subcommand -f -a rm          -d 'Remove an agent'
complete -c pswarm -n __fish_use_subcommand -f -a clean       -d 'Remove dead agents'
complete -c pswarm -n __fish_use_subcommand -f -a cwd         -d "Print agent's current working directory"
complete -c pswarm -n __fish_use_subcommand -f -a send        -d 'Send text to a running agent without attaching'
complete -c pswarm -n __fish_use_subcommand -f -a doctor      -d 'Daemon and environment diagnostics'
complete -c pswarm -n __fish_use_subcommand -f -a daemon      -d 'Daemon admin (start / stop / restart)'
complete -c pswarm -n __fish_use_subcommand -f -a completions -d 'Print shell completion script'

# Flags on `run`.
complete -c pswarm -n '__fish_seen_subcommand_from run' -s d -l detach \
    -d 'Spawn detached instead of attaching'

# Flags on `ls`.
complete -c pswarm -n '__fish_seen_subcommand_from ls' -l json \
    -d 'Emit machine-readable JSON'
complete -c pswarm -n '__fish_seen_subcommand_from ls' -l names \
    -d 'Emit only agent names, one per line'

# Flags on `rm`.
complete -c pswarm -n '__fish_seen_subcommand_from rm' -s f -l force \
    -d 'Skip graceful SIGTERM and kill immediately'

# Dynamic agent-name completion for subcommands that take a name.
complete -c pswarm -n '__fish_seen_subcommand_from attach rm cwd send' -f -a '(__pswarm_agent_names)'

# `daemon` subsubcommands.
complete -c pswarm -n '__fish_seen_subcommand_from daemon' -f -a 'start stop restart' \
    -d 'daemon lifecycle'

# `completions` shell argument.
complete -c pswarm -n '__fish_seen_subcommand_from completions' -f -a fish -d 'fish shell'
