# Crossport

**Stop guessing "What's using port 8080?".** Crossport visualizes your local development ports with available Git/Docker/K8s context and lets you inspect and manage the processes behind them.

[![CI](https://github.com/mt4110/crossport/workflows/CI/badge.svg)](https://github.com/mt4110/crossport/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
<p align="center">
  <img src="./assets/demo.gif" width="650" alt="Crossport demo" />
</p>

[🇯🇵 日本語](./README_ja.md)

## Why Crossport?

Unlike generic process killers, `crossport` is built specifically for developers:

- **Project Awareness** – See which project is using each port, not just `node` or `python`
- **Docker Integration** – Automatically resolves container names for Docker processes  
- **Interactive TUI** – Real-time process manager with keyboard navigation (`crossport ui`)
- **Gentle Kill Strategy** – Tries `SIGINT` → `SIGTERM` → `SIGKILL` for graceful shutdown
- **Smart Port Finder** – Find free ports and auto-update your `.env` files
- **Unix Pipeline Friendly** – JSON output for scripting (`crossport scan --json | jq ...`)

## Installation

### From GitHub Releases (Recommended)

Download the latest binary for your OS from the [Releases](https://github.com/mt4110/crossport/releases) page.

### From Source

```bash
git clone https://github.com/mt4110/crossport.git
cd crossport
cargo install --path .
```

### Using Nix

```bash
nix develop
cargo run -- ui
# Build without entering the shell:
nix build
./result/bin/crossport ui
# Or run directly:
nix run . -- ui
```

The toolchain (Rust 1.98.1) and inputs are pinned with `flake.lock`, for macOS/Linux.
The first run downloads the toolchain and dependencies. Docker/Kubernetes identification
requires host Docker/kubectl and appropriate access separately.

## Quick Start

### Interactive Mode (TUI)

The recommended way to use Crossport:

```bash
crossport ui
```

**Controls:**

- `j` / `k` or ↑/↓ – Navigate
- `Enter` – Open process details
- `/` – Filter; `s` / `S` – Change sort column/direction
- `h` – Show session/saved action history; `r` – Confirm container restart
- `x` – Confirm stopping the entire process
- In details/confirmation: `j` / `k`, arrows, PageUp/PageDown – Scroll
- `Esc` – Close details; `y` – Confirm stop; `n` / `Esc` – Cancel stop
- `q` – Quit

Details show the executable, macOS app bundle, parent process, uptime at selection,
working directory, arguments and all observed TCP listening endpoints for the PID.
Long text wraps. Stop confirmation includes endpoints hidden by the table filter.
PID, start time and executable are checked again before stopping; changed identity or
missing start time blocks the action. This does not eliminate PID reuse races throughout termination.

Listening does not imply active use. Last use and automatic restart are unknown because
they are not monitored. Stopping affects all work in the process and can lose unsaved
state; graceful termination can escalate to forced termination.

### Command Line

#### Scan Ports

```bash
# Scan default range (3000-9999)
crossport scan

# Scan specific range
crossport scan --from 8000 --to 9000

# JSON output for scripting
crossport scan --json | jq '.[] | select(.kind == "docker")'
```

**Output:**
```
PORT   PID      USER     CMD      KIND     PROJ
3000   39338    user     node     dev      my-frontend
5432   12456    user     postgres docker   my-db-container
8080   67890    user     python   other    backend-api
```

#### Kill Process

```bash
# Kill with interactive confirmation (default)
crossport kill 3000

# Force termination (still asks when confirmation is enabled)
crossport kill 3000 --force

# See what would be killed (dry run)
crossport kill 3000 --dry-run
```

#### Suggest Free Port

```bash
# Find next available port starting from 3000
crossport suggest

# Auto-update .env file
crossport suggest --env .env.local --key PORT
```

## Configuration

Crossport selects one configuration file in this order; files are not merged.
Missing explicit files, unknown keys and invalid settings are errors:

1. A file specified with `--config PATH`
2. `./crossport.toml`
3. `~/.config/crossport/config.toml`

Corresponding CLI options override configuration values. Kill settings apply to the CLI;
the TUI always asks for confirmation and uses staged termination. See the
[complete configuration example](config/crossport.example.toml).

**Example `crossport.toml`:**

```toml
[scan]
default_range = "3000-9000"

[kill]
confirm = true
default_signal = "TERM"
```

## Features

### Project Awareness

Crossport automatically detects the git root of each process and displays the **project name** instead of just the command name. This makes it easy to identify which app is using which port.

### Docker Integration

When a port is exposed by a Docker container, Crossport shows the **container name** in the `PROJ` column, making it easy to identify containerized services.

### JSON Export

Perfect for Unix pipelines and automation:

```bash
# Get all dev ports as JSON
crossport scan --json | jq '.[] | select(.kind == "dev") | .port'

# Check if port 3000 is in use
crossport scan --from 3000 --to 3000 --json | jq -e '.[0].pid > 0' && echo "Port in use"
```

### Gentle Kill

Instead of immediately sending `SIGKILL`, Crossport tries:
1. `SIGINT` (like pressing Ctrl+C) – allows cleanup handlers to run
2. `SIGTERM` (after about 1s) – standard termination signal
3. `SIGKILL` (after about 1 more second) – forced termination as last resort

This is the Unix strategy when no explicit signal is selected; cleanup or saved state is not guaranteed.
`--signal` / `kill.default_signal` accepts `INT`, `TERM` or `KILL` and sends only that signal when set.

## Development

### Running Tests

```bash
cargo test
```

Nix environment checks:

```bash
nix develop -c cargo test --locked
nix develop -c cargo clippy --locked --all-targets -- -D warnings
```

`nix build` also runs the tests. Backend E2E tests own their TCP listeners and exercise
CLI scanning, process metadata, kill dry-run and discovery after releasing a socket.
A separate stop E2E sends TERM only to its own dedicated child fixture and verifies disappearance.
Existing services and Docker/Kubernetes operations are not exercised.
The race regression controls the `lsof` reply and starts a real listening child after process metadata capture; discovery must fail as incomplete rather than report a free port.

### Building

```bash
cargo build --release
./target/release/crossport --version
```

## Settings, History and Ownership

- `general.default_port`: inspected by bare `crossport` when no command/port is supplied. Without it, help is shown.
- `backend.strategy`: `auto` uses Unix `lsof` or Windows `netstat`. Explicit backends: `lsof` (Unix), `ss` (Linux), `netstat` (Linux/Windows). Unsupported OS combinations and command failures are errors. Linux parser fixtures are tested; live Linux execution is unverified. Process visibility also depends on OS permissions. If `ss` or Linux `netstat` reports a listener whose PID is unavailable or malformed, the whole scan fails rather than treating partial results as free ports. For every backend, a detected PID missing from process metadata also fails the entire capture instead of discarding the listener.
- `output.verbose` / `--verbose`: adds executable, parent PID, start time (Unix seconds), uptime and UID to CLI text output. JSON is unchanged.
- `output.color`: sets CLI colors and the TUI default. `ui.color` overrides the TUI; disabling colors keeps reverse-video selection.
- Stop actions re-read ownership. Unix compares the owner UID to the caller's effective UID; Windows compares user identifiers. Unknown/foreign owners are rejected. CLI `--all-users` explicitly bypasses this comparison but grants no OS privileges; the TUI has no bypass.
- Container restart failures appear in a scrollable error dialog and are not automatically retried. If a stop/restart succeeds but refreshing ports fails, the TUI distinguishes completed actions from failed display updates, preserves successful history, warns that displayed data may be stale and does not repeat the action.

TUI `h` displays session stop/restart history. Set `history.enabled = true` to save
CLI/TUI events locally and load them next time. Use `crossport history` or `crossport history --json` to read saved events.
The default path is the platform local-data directory plus `crossport/history.json`;
`history.path` overrides it. `history.max_entries` keeps the newest events (default 200, range 1..10000).
Only Unix timestamp, PID, port, action and outcome are stored: no arguments, paths,
usernames or error text. Dry-runs and failed attempts are recorded; canceled actions and scans are not.
Success means the operation returned without error, not a guarantee of workload health.

On Unix, history files require owner-only access (0600); newly created directories use 0700.
Updates use an exclusive lock and atomic file replacement. Corruption and save failures
are reported; lock contention can prevent saving an event. The operation itself is not retried.
Windows access control depends on the destination's OS ACL. Choose a private directory.

## Implementation Status and Limits

TUI, JSON, watch mode, settings, action history, ownership checks and restart errors are implemented.
Docker provides published-port/name mapping and restart. Kubernetes inference uses
`port-forward` arguments and `k8s_` names, with no general Pod enumeration.
Windows uses `netstat` / `taskkill` rather than Unix staged signals.
`kind` is a heuristic classification, not a safety assessment.
Nix is verified on macOS; live Linux/Windows and Docker/Kubernetes operations are outside local verification.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.


## Privacy & Security

**Crossport runs entirely on your local machine.**

- **Local History**: Saving action history requires explicit opt-in, with bounded fields and retention as described above. No analytics are uploaded.
- **Local Operations**: All port scanning and process management happens locally using standard system APIs (`lsof`, `netstat`, Docker API).
- **Open Source**: The code is 100% open source. You can verify exactly what it does by reviewing the source code.

## License

MIT License. See [LICENSE](./LICENSE) for details.

## Acknowledgments

Built with Rust 🦀 and love for developer productivity.
