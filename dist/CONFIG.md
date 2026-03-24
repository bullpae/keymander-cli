# Portable config layout

포터블 번들의 `kmd-data/config.toml`은 **아래 조각 파일을 OS별로 이어 붙여** 생성합니다. Windows용 `Cmd+V`와 macOS용 `Cmd+V`가 한 파일에 섞이지 않도록 분리했습니다.

---

The bundle `kmd-data/config.toml` is **assembled** from:

| Fragment | Purpose |
|----------|---------|
| `config.shared.toml` | Shared: `[general]`, `[launcher]` (non-keymap), `[keybindings]`, comments |
| `config.keymap.windows.toml` | Windows daemon keymap (Ctrl, Home/End, etc.) |
| `config.keymap.macos.toml` | macOS daemon keymap (Cmd, Cocoa-friendly macros) |
| `config.keymap.linux.toml` | Linux: `active_profile` only (daemon hook stub; preset in code) |

## Assemble

- **Windows (PowerShell):**  
  `.\scripts\assemble-config.ps1 -Platform windows -OutFile "kmd-data\config.toml"`
- **macOS / Linux (Bash):**  
  `./scripts/assemble-config.sh macos kmd-data/config.toml`  
  `./scripts/assemble-config.sh linux kmd-data/config.toml`

Deploy and portable build scripts call these automatically.
