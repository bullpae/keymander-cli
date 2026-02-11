# keymander (kmd)

**키보드 하나로 모든 것을 지휘한다** — CLI-first cross-platform keyboard launcher

A lightweight, portable, keyboard-driven launcher for Windows, macOS, and Linux.
Single binary, fast startup, minimal memory — no Electron, no GUI toolkit, just your terminal.

## Features

- **Cross-platform**: Windows, macOS, Linux with a unified interface
- **CLI-first architecture**: Core logic decoupled from UI — scriptable, testable, extensible
- **Lightweight**: ~3MB binary, ~40ms startup, <5MB RAM
- **Portable**: Single binary + SQLite DB + TOML config = done
- **Smart search**: Fuzzy (Nucleo), glob, regex, substring, URL detection
- **File & folder indexing**: Indexes both files and directories with proper icons
- **Folder drill-down**: Navigate into folders with Tab/→, go back with ←/Esc
- **Web services**: `@g rust tutorial`, `@gh keymander`, `@yt lofi music`
- **AI services**: `@ai question`, `@gpt prompt`, `@claude query`, `@gemini ask`
- **History-aware**: Frequently used items bubble to the top, recent launches shown on empty query
- **Plugin system**: Extension trait + script-based plugins (JSON over stdin/stdout)
- **Built-in calculator**: `:calc 2+3*4` → `14`

## Installation

### From source

```bash
cargo install --path .
```

### From releases

Download the latest binary from [GitHub Releases](https://github.com/bullpae/keymander/releases).

## Usage

### TUI Mode (default)

```bash
kmd
```

Launches the interactive TUI launcher. Type to search, arrow keys to navigate, Enter to launch.
Select a folder and press Tab to browse its contents.

### CLI Commands

```bash
# Search
kmd search "firefox"
kmd search "*.pdf" --json
kmd search "@g rust tutorial"

# Launch
kmd launch "Firefox"
kmd launch ~/documents/report.pdf
kmd launch https://github.com

# Index management
kmd index --stats
kmd index --rebuild

# Configuration
kmd config                    # show config path
kmd config get general.theme
kmd config set general.theme "nord"
kmd config edit               # open in $EDITOR

# History
kmd history list
kmd history clear

# Plugins
kmd plugin list

# Daemon (future)
kmd daemon start
kmd daemon status
```

### Search Modes

| Input | Mode | Example |
|-------|------|---------|
| Regular text | Fuzzy | `fire` → Firefox |
| `*` or `?` | Glob | `*.pdf`, `test?.rs` |
| `/pattern/` | Regex | `/test\d+/` |
| `.ext` | Extension | `.pdf` → `*.pdf` |
| Non-ASCII | Contains | `한글` (exact substring) |
| URL-like | URL | `github.com` → opens browser |
| `@prefix` | Web search | `@g rust tutorial` |
| `:calc` | Calculator | `:calc (2+3)*4` |

### Web Services

| Prefix | Service |
|--------|---------|
| `@g` | Google |
| `@yt` | YouTube |
| `@gh` | GitHub |
| `@so` | StackOverflow |
| `@npm` | npm |
| `@crates` | crates.io |
| `@w` | Wikipedia |
| `@x` | X (Twitter) |
| `@map` | Google Maps |
| `@dict` | Naver Dictionary |

### AI Services

| Prefix | Service |
|--------|---------|
| `@ai` / `@pplx` | Perplexity AI |
| `@gpt` / `@chatgpt` | ChatGPT |
| `@claude` | Claude AI |
| `@gemini` | Google Gemini |

### Keybindings (TUI)

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate results |
| `Enter` | Launch selected item |
| `Tab` / `→` | Drill into selected folder |
| `←` | Go back from folder drill-down |
| `Esc` | Exit drill-down / clear query / quit |
| `Ctrl+C` | Quit |
| `Ctrl+P` | Toggle preview panel |
| `Ctrl+Space` | Toggle Korean/English input |

## Architecture

```mermaid
graph TB
    subgraph Layer3 ["Layer 3: TUI"]
        TUI["ratatui + crossterm<br/>Interactive UI"]
    end
    subgraph Layer2 ["Layer 2: CLI"]
        CLI["clap subcommands<br/>kmd search / launch / index / ..."]
    end
    subgraph Layer1 ["Layer 1: kmd-core"]
        Core["Index | Search | Config | DB | Plugin"]
    end

    TUI --> CLI
    CLI --> Core
    TUI --> Core
```

- **kmd-core**: Pure library — indexing, search (Nucleo), SQLite, config, plugin system
- **CLI**: Subcommands that use kmd-core — scriptable, JSON output
- **TUI**: Interactive frontend built on kmd-core — what you see when you run `kmd`

## Configuration

Config file location: `~/.config/kmd/config.toml` (Linux/macOS) or `%APPDATA%/kmd/config.toml` (Windows)

```toml
[general]
render_fps = 30
show_preview = true
theme = "default"

[launcher]
file_search_provider = "auto"  # auto | builtin | fd | everything | mdfind | locate
max_results = 5000
quit_on_launch = false
ignore_patterns = [".git", "node_modules", "target"]

[keybindings]
global_hotkey = "alt+space"
```

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust (edition 2021) |
| TUI | Ratatui + Crossterm |
| Fuzzy search | Nucleo |
| Database | SQLite (rusqlite, bundled) |
| CLI | clap (derive) |
| Config | TOML + serde |
| Async | Tokio |

## License

MIT
