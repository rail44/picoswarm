# Default action: list the available recipes.
default:
    @just --list

# Build the debug binary.
build:
    cargo build

# Build (incrementally) and run pswarm with the given arguments.
# Examples: `just dev doctor`, `just dev daemon restart`, `just dev run feat -- /bin/sh`.
dev *args:
    cargo run --quiet {{args}}

# Run all tests (unit + integration).
test:
    cargo test

# Lint with clippy, treating warnings as errors.
clippy:
    cargo clippy --all-targets -- -D warnings

# Format the codebase.
fmt:
    cargo fmt

# Verify formatting without writing.
fmt-check:
    cargo fmt --check

# All pre-commit checks: clippy + fmt-check + test.
check: clippy fmt-check test

# Tail the daemon log file.
log:
    tail -F "${XDG_STATE_HOME:-$HOME/.local/state}/picoswarm/daemon.log"
