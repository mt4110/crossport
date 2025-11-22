# crossport

**The Developer-First Port Manager.**

`crossport` is a CLI tool designed to make managing local development ports less painful. It helps you identify what's running, safely stop processes, and find free ports for your new projects.

## Why crossport?

Unlike generic process killers, `crossport` is built for developers:

1.  **Project Awareness (`PROJ` column)**
    *   Don't just see `node` or `python`. See **which project** is running it (e.g., `my-frontend`, `backend-api`).
    *   It automatically detects the project root (git) and displays the directory name.

2.  **Gentle Kill Strategy**
    *   `crossport kill` doesn't just `SIGKILL`. It sends `SIGINT` (Ctrl+C) first, allowing runtimes like Node.js or Docker to clean up resources (close DB connections, save state) before exiting.
    *   Defaults to **safety first**: it only kills your own processes by default.

3.  **Smart Port Suggestion**
    *   Need a free port? `crossport suggest 3000` finds the next available one (e.g., 3001).
    *   **Auto-update .env**: It can even update your `.env` file automatically!
    *   `crossport suggest 3000 --env .env.local --key PORT`

## Installation

### Using Nix (Recommended)

This project uses [Nix](https://nixos.org/) flakes for a reproducible development environment.

```bash
# Crossport

**Cross-platform Port Management CLI** (macOS, Linux, Windows)

`crossport` is a developer-friendly tool to manage local ports and processes. It helps you find what's running, kill blocking processes, and discover free ports for your apps.

## Features

- **Cross-Platform**: Works on macOS, Linux, and Windows.
- **Project Awareness**: Identifies which project (git root) a process belongs to.
- **Docker Integration**: Resolves container names for Docker processes.
- **Interactive TUI**: `crossport ui` provides a real-time, interactive process manager.
- **Gentle Kill**: Tries `SIGINT` -> `SIGTERM` -> `SIGKILL` to allow graceful shutdown.
- **Smart Suggestion**: Finds available ports and can update your `.env` file automatically.
- **Configuration**: Customizable via `crossport.toml` or `~/.config/crossport/config.toml`.
- **JSON Output**: `crossport scan --json` for easy integration with Unix pipelines.

## Installation

### From GitHub Releases (Binary)

Download the latest binary for your OS from the [Releases](https://github.com/your-repo/crossport/releases) page.

### From Source

```bash
cargo install --path .
```

## Usage

### Interactive Mode (TUI)

The recommended way to use Crossport.

```bash
crossport ui
```

- **Navigate**: `j` / `k` or Arrow keys
- **Kill Process**: `x` (opens confirmation dialog)
- **Quit**: `q`

### Command Line Interface

#### Scan Ports

```bash
# Scan default range (3000-9999)
crossport scan

# Scan specific range
crossport scan --from 8000 --to 9000

# Output as JSON
crossport scan --json
```

#### Kill Process

```bash
# Kill process on port 3000
crossport kill 3000

# Interactive confirmation (default)
crossport kill 3000 -i

# Force kill
crossport kill 3000 --force
```

#### Suggest Port

```bash
# Suggest a free port starting from 3000
crossport suggest

# Update .env file with the found port
crossport suggest --env .env --key PORT
```

## Configuration

Crossport looks for config files in the following order:
1. CLI arguments
2. `crossport.toml` (current directory)
3. `~/.config/crossport/config.toml` (home directory)

Example `crossport.toml`:

```toml
[scan]
default_range = "3000-9000"

[kill]
confirm = true
default_signal = "SIGTERM"

[ui]
color = true
```

## License

MIT

### Kill a Process
Safely terminate a process.

```bash
$ crossport kill 8080
Process 67890 exited after SIGINT
```
Use `--dry-run` to see what would happen without killing.

### Suggest a Free Port
Find a free port starting from 3000.

```bash
$ crossport suggest 3000
Suggested port: 3001
```

Update your `.env` file:
```bash
$ crossport suggest 3000 --env .env --key SERVER_PORT
Updated SERVER_PORT in ".env"
```

## Roadmap

- [ ] **TUI Mode**: Interactive terminal UI for managing ports.
- [ ] **Docker Integration**: Better container name resolution.
- [ ] **CI/CD**: Automated release builds.

## License

MIT License. See [LICENSE](./LICENSE) for details.
