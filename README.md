# dockerrs

A lightweight terminal UI for managing Docker containers and networks, built with Rust.

## Features

- **Containers** - List all containers with name, image, state, status, and ports
- **Networks** - Browse Docker networks and inspect connected containers
- **Logs** - Stream container logs in real time with scroll support
- **Actions** - Start, stop, kill, and remove containers directly from the TUI

## Install

```
cargo install dockerrs
```

Or build from source:

```
git clone https://github.com/sanderkohnstamm/dockerrs
cd dockerrs
cargo build --release
```

## Usage

```
dockerrs
```

Requires a running Docker daemon.

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate up/down |
| `Tab` | Switch between Containers and Networks |
| `Enter` | Open container detail view |
| `l` | Stream container logs |
| `s` | Start/stop container (toggle) |
| `x` | Kill container |
| `r` | Remove container |
| `Esc` | Back to previous view |
| `PageUp` / `PageDown` | Scroll logs |
| `g` / `G` | Jump to top/bottom of logs |
| `q` | Quit |
| `Ctrl+c` | Force quit |

## License

Apache-2.0
