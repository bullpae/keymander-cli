# keymander (kmd)

**키보드 하나로 모든 것을 지휘한다** — CLI-first cross-platform keyboard launcher

A lightweight, portable, keyboard-driven launcher for Windows, macOS, and Linux.
Single binary, fast startup, minimal memory — no Electron, no GUI toolkit overhead.

## Two Interfaces, One Core

| | **kmd** (CLI / TUI) | **kmd-desktop** (Desktop GUI) |
|---|---|---|
| UI | Terminal (ratatui + crossterm) | GPU-accelerated window (iced) |
| Look | Full-screen TUI with preview panel | Floating Spotlight-like search bar |
| Launch | `kmd` or hotkey → terminal | `kmd-desktop` or hotkey → overlay |
| Best for | Terminal power users, scripting | Mouse + keyboard hybrid workflow |

Both share the same **kmd-core** library — identical search, config, index, and plugins.

## Features

- **Cross-platform**: Windows, macOS, Linux with a unified interface
- **CLI-first architecture**: Core logic decoupled from UI — scriptable, testable, extensible
- **Lightweight**: ~3MB binary, instant startup, <5MB RAM
- **Portable**: Single binary + SQLite DB + TOML config = done
- **Smart search**: Fuzzy (Nucleo), glob, regex, substring, URL detection
- **Smart Directory Jump**: Multi-word path matching with frecency-based learning (zoxide-style)
  - `2026 출장이력` → matches `c:\2026\work\출장이력` (each token matches a path segment)
  - Frequently selected directories rank higher in future searches
- **File & folder indexing**: Indexes both files and directories with configurable scan scope
- **Folder drill-down** (TUI): Navigate into folders with Tab/→, go back with ←/Esc
- **Web services**: `@g rust tutorial`, `@gh keymander`, `@yt lofi music`
- **AI services**: `@ai question`, `@gpt prompt`, `@claude query`, `@gemini ask`, `@grok ask`
- **Multi LLM compare**: `@ll same prompt` (or `@llm`) opens selected LLMs at once
- **Multi web search**: `@m same query` (or `@msearch`) opens Google/Naver/Daum together
- **Korean spelling check**: `@sp 문장` opens selected spelling check providers
- **Translate EN↔KO**: `@tr text`, `@trko text`, `@tren text`
- **Keymap management**: Built-in Kanata profiles (vim-nav, minimal) with `kmd keymap init/use/start/stop`
  - `vim-nav` preset: Alt hold → Vim-style HJKL navigation + Alt+Space → launch kmd-desktop
  - Desktop: `:keymap` or `:km` for status, on/off, profile switching
- **Prompt templates**: `:prompt add review "Review this code: {query}"` → `@ll :review code`
- **Quick Transform**: `:t spell text`, `:t trko text` — clipboard → spell check / translate instantly
- **Inline calculator**: Type math expressions anywhere — result appears instantly
- **Emoji search**: `:emoji fire` or `:e 하트` — search & copy Unicode emoji (English + Korean)
- **Shell commands**: `!ip`, `!hostname`, `!uptime` — quick system info, or run any shell command
- **Single-instance toggle**: Hotkey press toggles on/off — no duplicate windows
- **Frecency history**: Recently and frequently used items bubble to the top (time-decay scoring)
- **History auto-pruning**: Aging algorithm prevents database from growing unboundedly
- **Plugin system**: Extension trait + script-based plugins (JSON over stdin/stdout)
- **Settings** (TUI): F2 key opens settings modal; (Desktop): `:set` command
- **Theming** (Desktop): 5 built-in presets — Midnight, Obsidian, Snow, Rose Pine, Nord
- **Korean input** (TUI): Built-in 2-벌식 Hangul composer for direct Korean input
- **Search priority weights**: Configure which item kinds rank higher
- **Index cache versioning**: Auto-rebuilds when binary version changes

## Installation

### From source

```bash
# CLI + TUI
cargo install --path .

# Desktop GUI
cargo install --path crates/kmd-desktop
```

### From releases

Download the latest binary from [GitHub Releases](https://github.com/bullpae/keymander-cli/releases).

## Usage

### Desktop Mode (kmd-desktop)

```bash
kmd-desktop
```

Launches a floating, borderless, transparent search window (Spotlight-like) with square corners.
Type to search, arrow keys to navigate, Enter to launch, Esc to dismiss.
Drag the top strip to move the window; drag the left or right edges to resize.
The window disappears after launching an item.

**Setting up a global hotkey:**

**Windows (AutoHotkey)**
```ahk
!Space::Run "kmd-desktop"   ; Alt+Space → kmd-desktop overlay
```

**Windows (PowerToys)**
1. Install [PowerToys](https://github.com/microsoft/PowerToys)
2. Open PowerToys → Keyboard Manager → Remap a shortcut
3. Map `Alt+Space` → Run `kmd-desktop`

### TUI Mode (default CLI)

```bash
kmd
```

Launches the interactive TUI launcher. Type to search, arrow keys to navigate, Enter to launch.
Select a folder and press Tab to browse its contents.
By default, kmd exits after launching an item (`quit_on_launch = true`).

**Use as a Global Launcher:**

kmd is designed to be summoned with a hotkey, used, and then disappear. **Single-instance toggle**: pressing the hotkey again while kmd is already open closes the existing instance.

**Windows (AutoHotkey)**
```ahk
!Space::Run "wt" "-w _quake kmd"   ; Alt+Space → kmd in quake terminal
```

**macOS (Hammerspoon)**
```lua
hs.hotkey.bind({"alt"}, "space", function()
  hs.execute("open -a Terminal kmd", true)
end)
```

**Linux (sxhkd)**
```
alt + space
    alacritty --class kmd-float -e kmd
```

**Recommended: Kanata + keymander (Vim navigation + launcher hotkey)**

Install [Kanata](https://github.com/jtroo/kanata), then:

```bash
kmd keymap init vim-nav    # install vim-nav preset → Alt hold = Vim nav, Alt+Space = kmd-desktop
kmd keymap start           # start kanata with the active profile
```

The `vim-nav` preset provides:
- Alt hold + HJKL = arrow keys, N/M = PageUp/Down, I/O = word jump / Home/End
- Alt + Space = launch kmd-desktop (via kanata `cmd`)
- Alt tap (alone) = Esc

Register kanata as an OS startup program for persistent keymap.

See `kmd keymap list-presets` for all built-in presets.

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

# Portable mode
kmd portable enable   # use kmd-data/ next to exe
kmd portable disable  # use standard config/data dirs

# Configuration
kmd config                    # show config path
kmd config get general.theme
kmd config set general.theme "nord"
kmd config edit               # open in $EDITOR

# History
kmd history list
kmd history clear

# Emoji search & copy
kmd emoji fire          # search and copy first result
kmd emoji heart --list  # list all matches
kmd emoji 하트 --json   # JSON output
kmd emoji star -c 3     # copy 3rd result

# Plugins
kmd plugin list

# Keymap management (Kanata)
kmd keymap status          # show keymap process status
kmd keymap init vim-nav    # install preset and set active
kmd keymap list            # list installed profiles
kmd keymap list-presets    # show available built-in presets
kmd keymap use vim-nav     # switch active profile
kmd keymap start           # start kanata with active profile
kmd keymap stop            # stop kanata process
kmd keymap stop

# Daemon (future)
kmd daemon start
kmd daemon status
```

## Search Modes

| Input | Mode | Example |
|-------|------|---------|
| Regular text | Fuzzy | `fire` → Firefox |
| `*` or `?` | Glob | `*.pdf`, `test?.rs` |
| `/pattern/` | Regex | `/test\d+/` |
| `.ext` | Extension | `.pdf` → `*.pdf` |
| Non-ASCII (single word) | Contains | `한글` (exact substring) |
| Non-ASCII (multi-word) | Smart Match | `2026 출장이력` (AND match on path segments) |
| URL-like | URL | `github.com` → opens browser |

## Special Command Prefixes

Type `:help` (or `:h`) in the search bar to see all available commands.
In `:help`, selecting an entry and pressing Enter fills a starter query (quick template).

| Prefix | Mode | Example | Description |
|--------|------|---------|-------------|
| `@prefix` | Web search | `@g rust tutorial` | Search via web service |
| `@ai` | AI search | `@ai why is the sky blue` | Ask Perplexity AI |
| `@ll` / `@llm` / `@cmp` | Multi LLM compare | `@ll explain Rust lifetimes` | Open selected LLMs with same prompt |
| `@m` / `@mw` / `@msearch` | Multi web search | `@m 러스트 소유권` | Open selected search engines with same query |
| `@sp` / `@spell` | Korean spelling check | `@sp 안녕 하세요` | Open selected spell check providers |
| `@tr` / `@trko` / `@tren` | Translate | `@trko hello world` | Translate text in selected providers |
| `:calc` | Calculator | `:calc (2+3)*4` | Evaluate math expression |
| `:emoji` / `:e` | Emoji | `:e fire`, `:e 하트` | Search & copy emoji |
| `:set` / `:settings` | Settings | `:set`, `:settings theme` | Manage config, themes, index |
| `:help` / `:h` | Help | `:help` | Show all available commands |
| `!command` | Shell | `!ip`, `!echo hello` | Run shell command |

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
| `@naver` / `@kr` | Naver Search |
| `@daum` | Daum Search |
| `@dict` | Naver Dictionary |

### AI Services

| Prefix | Service |
|--------|---------|
| `@ai` / `@pplx` | Perplexity AI |
| `@gpt` / `@chatgpt` | ChatGPT |
| `@claude` | Claude AI |
| `@gemini` | Google Gemini |
| `@grok` | xAI Grok |

### Multi LLM Prompting

`@ll` (or `@llm`, `@multi`, `@cmp`) sends one prompt to multiple LLM web UIs by opening each selected provider URL in parallel browser tabs.

Recommended `@` commands:

- `@ll summarize this article` — compare multiple LLM answers quickly
- `@gpt write unit tests for this Rust function`
- `@claude refactor this module for readability`
- `@gemini explain this error stack trace`
- `@grok suggest edge cases for this feature`
- `@ai find latest docs and sources for this topic` (Perplexity)

### Multi Web Search

`@m` (or `@mw`, `@msearch`, `@multisearch`, `@searchall`, `@krsearch`) opens one query on multiple engines in parallel tabs.

Recommended commands:

- `@m rust ownership`
- `@m 러스트 소유권`
- `@m cursor ai 단축키`

### Korean Spelling Check

`@sp` (or `@spell`) checks Korean spelling by opening selected providers in parallel tabs.

Recommended commands:

- `@sp 안녕 하세요 오늘 날씨가 좋내요`
- `@sp 보고서 문장 교정`

### Translate (EN↔KO)

`@tr` (auto detect), `@trko` (English -> Korean), `@tren` (Korean -> English)

Recommended commands:

- `@tr hello world`
- `@trko Please summarize this code`
- `@tren 이 문장을 영어로 번역해줘`

## Keybindings

### Desktop (kmd-desktop)

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate results |
| `Enter` | Launch selected item |
| `Esc` | If query exists: clear + refocus input. If empty: quit |
| Mouse click | Select and launch item |
| Logo left-click (`»`) | Toggle `:help` |
| Logo right-click (`»`) | Toggle `:set` |
| Drag top strip | Move window |
| Drag left/right edges | Resize window |

### TUI (kmd)

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
| `F2` | Open settings modal |

## Architecture

```mermaid
graph TB
    subgraph Layer4 ["Layer 4: Desktop GUI"]
        Desktop["iced (GPU-accelerated)\nFloating overlay launcher"]
    end
    subgraph Layer3 ["Layer 3: TUI"]
        TUI["ratatui + crossterm\nInteractive UI + Settings modal"]
    end
    subgraph Layer2 ["Layer 2: CLI"]
        CLI["clap subcommands\nkmd search / launch / index / ..."]
    end
    subgraph Layer1 ["Layer 1: kmd-core"]
        Core["Index | Search | Config | DB | Plugin | Hangul"]
    end

    Desktop --> Core
    TUI --> CLI
    CLI --> Core
    TUI --> Core
```

- **kmd-core**: Pure library — indexing, search (Nucleo), SQLite, config, plugin system, Hangul composer
- **CLI**: Subcommands that use kmd-core — scriptable, JSON output
- **TUI**: Interactive frontend built on kmd-core — what you see when you run `kmd`
- **Desktop**: GPU-accelerated overlay launcher — what you see when you run `kmd-desktop`

## Desktop Themes

kmd-desktop includes 5 built-in themes. Change via `:set` command or `config.toml`:

| Theme | Description |
|-------|-------------|
| **Midnight** (default) | Deep blue, keymander signature |
| **Obsidian** | OLED black, ultra-minimal |
| **Snow** | Clean light theme |
| **Rose Pine** | Warm, soft palette |
| **Nord** | Calm Scandinavian palette |

```toml
# config.toml — set theme
[general]
theme = "midnight"   # midnight | obsidian | snow | rose_pine | nord
```

## Configuration

Config file location: `~/.config/kmd/config.toml` (Linux/macOS) or `%APPDATA%/kmd/config.toml` (Windows)

- **TUI**: Press **F2** to open the settings modal
- **Desktop**: Type `:set` or `:settings` in the search bar

```toml
[general]
render_fps = 30
show_preview = true
preview_width_percent = 40
theme = "default"        # TUI theme | Desktop theme (midnight/obsidian/snow/rose_pine/nord)
emoji_icons = true       # emoji icons (false = ASCII fallback)
reset_ime_on_launch = true  # Desktop: reset IME to English mode on open

[launcher]
file_search_provider = "auto"  # auto | builtin | fd | everything | mdfind | locate | winfs
max_results = 10000
search_depth = 6
quit_on_launch = true
index_directories = true       # include folders in search index
scan_drives = true             # auto-discover drive roots (C:\, D:\, etc.)
drive_scan_depth = 3           # shallow depth for drive root scanning
multi_llm_providers = ["chatgpt", "gemini", "claude", "grok", "perplexity"]  # providers used by @llm
multi_llm_prefixes = ["@ll", "@llm", "@multi", "@cmp", "@compare"]            # aliases for multi-LLM command
multi_web_providers = ["google", "naver_search", "daum"]  # engines used by @msearch
multi_web_prefixes = ["@m", "@mw", "@msearch", "@multisearch", "@searchall", "@krsearch"]  # aliases for multi-web command
spell_providers = ["naver_spell", "pusan_spell"]
spell_prefixes = ["@sp", "@spell"]
translate_providers = ["google_translate", "papago", "deepl"]
translate_prefixes = ["@tr", "@trko", "@tren"]

[launcher.keymap]
backend = "kanata"
kanata_path = ""              # optional absolute path
profile_dir = ""              # optional profile dir (default: config_dir/keymap)
active_profile = "vim-nav.kbd"

# Search priority weights (0-100, higher = ranked higher)
[launcher.kind_weights]
directory = 80
app = 70
file = 50
executable = 40
system_cmd = 30
web_search = 20

# Directories to scan (platform defaults: Desktop, Documents, Downloads)
# search_paths = ["C:\\Users\\you\\Documents", "D:\\Projects"]

# Patterns to exclude from indexing
# ignore_patterns = [".git", "node_modules", "target", "Windows", "Program Files"]

[keybindings]
global_hotkey = "alt+space"
quit = "ctrl+c"
next = "down"
prev = "up"
select = "enter"
toggle_preview = "ctrl+p"
```

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust (edition 2021) |
| Core | kmd-core (shared library) |
| TUI | Ratatui + Crossterm |
| Desktop GUI | iced 0.14 (GPU-accelerated) |
| Fuzzy search | Nucleo |
| Database | SQLite (rusqlite, bundled) |
| CLI | clap (derive) |
| Config | TOML + serde |
| Theme | Catppuccin Mocha inspired (TUI) / 5 presets (Desktop) |

## Roadmap

**v0.3.2** (current)
- Kanata keymap integration: vim-nav preset (Alt+Space → kmd-desktop), `:keymap` / `:km` in Desktop
- New keymander icon (pixel-art k>>r) for exe and window

**v0.3.1**
- Brand icons: Google, ChatGPT, Naver, etc. — actual PNG logos instead of emojis (kmd-desktop)
- Icon flickering fix: LazyLock cache for stable texture rendering

**v0.3.0**
- Smart Directory Jump — multi-word path matching with frecency learning
- Frecency history scoring (time-decay + frequency, capped boost)
- History auto-pruning (zoxide-style aging algorithm)
- Web module refactoring (`web.rs` → `web/` directory with services, parsers, items)
- Prompt templates: `:prompt add review "Review this code: {query}"` → `@ll :review code`
- Quick Transform: `:t spell text`, `:t trko text` — clipboard → spell check / translate
- Korean spelling check: `@sp 문장` opens selected providers
- Translate: `@tr text`, `@trko text`, `@tren text`
- Multi web search: `@m query` opens Google/Naver/Daum together
- Keymap backend prototype: `kmd keymap` with Kanata integration
- Custom command aliases: user-configurable prefixes for all multi-service commands
- DB migration v2: `executed_at` index for frecency query performance

**v0.2.0**
- Desktop GUI launcher (kmd-desktop) with iced
- 5 built-in themes (Midnight, Obsidian, Snow, Rose Pine, Nord)
- Emoji search & copy (English + Korean)
- Shell command execution & system quick actions
- Single-instance toggle (hotkey on/off)
- Built-in Hangul composer with auto-activation
- Performance optimizations (async engine loading, reduced tick timeout)
- `:help` command for discoverability

**Toward v1.0.0**
- Cloud-synced todo / memo (remote storage integration)
- Clipboard history manager
- Plugin marketplace & script-based plugins
- Global hotkey daemon (cross-platform)
- Custom theming engine (user-defined color schemes)
- Multi-monitor awareness
- Glassmorphism effects (Desktop)
- Auto-growing search input for AI prompts (Desktop)

## License

MIT
