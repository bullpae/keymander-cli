# Changelog

All notable changes to keymander are documented here.

## [0.5.0] — 2026-05-16

### New Features
- **Frecency-based ranking** — frequently used programs/files float to the top of search results. Launch history is recorded per item and decays over time (1h ×16, 24h ×8, 1w ×4, 1mo ×2, older ×1). Applies to all search modes including relaxed Hangul fallback.
- **Calculator clipboard** — pressing Enter on a calculation result copies the value to the clipboard. Ctrl+4 shortcut and a dedicated "값 복사" copy button appear in the detail panel.
- **Folder search** (`:f`) — type `:f /path query` or `:f ~/path query` to instantly search inside any directory without adding it to the index. Prefix-match results are ranked higher, folders appear before files, and emoji icons indicate file type.
- **Layer Launch deferral** — layer-key bindings that launch apps now wait until the trigger key is released before executing, eliminating modifier-key interference with IME and launched apps.

### UI / Design
- **Keymander theme** — new default color palette: deep ink background, copper accent, signal-cyan border. Replaces "Midnight". Backward-compatible alias kept.
- **Brand mark** — the `»` icon is now rendered inside a subtle copper circular badge. The double-border card (outer copper glow + inner accent) adds depth without clutter.
- **Dynamic border opacity** — card border brightens (0.28 → 0.52) when the search bar is idle, giving a subtle focus cue.

### Improvements
- **Daemon accept loop** — replaced 50 ms busy-wait polling with a blocking `incoming()` loop on a dedicated thread. Eliminates unnecessary context switches when idle.
- **IPC kind field** — `format!("{:?}", kind)` replaced with `kind.to_string()` (stable `Display` impl). Enum renames no longer silently break the protocol.
- **`filter_contains` performance** — path-segment lookup changed from `Vec::contains` O(n) to `HashSet` O(1) per token in multi-token searches.
- **`copy_multi_llm` config** — no longer re-reads config from disk; uses `self.runtime_config` directly, preserving any in-session edits.
- **DB open error logging** — silent in-memory fallback now emits a `tracing::warn!` so users can diagnose missing frecency persistence.
- **`select_services` dedup** — duplicate provider IDs in config are silently deduplicated instead of creating duplicate search rows.
- **Brand click routing** — `BrandClicked` / `BrandRightClicked` now use `toggle_query_mode()` helper backed by `prefix_of()`, removing hardcoded string checks.

### Refactoring
- `app.rs` split from ~3 400 lines into focused modules: `app/settings.rs`, `app/launch.rs`, `app/view.rs` (retains ~1 700 lines).
- Unified two divergent `is_hangul_jamo` implementations — `App::is_hangul_jamo` now delegates to the free function.
- Removed unused `KeyboardBackend::is_running` trait method and all implementations.
- Removed unused `surface` and `corner_radius` fields from `DesktopTheme`.
- `UiScale.brand_icon` field removed (superseded by `view_brand_mark`).

### CI
- Added Linux system-library installation steps (`libx11`, `libxcb`, `libwayland`, `libxkbcommon`, `libfontconfig`) to `ci.yml` and `release.yml` so `kmd-desktop` (iced) builds correctly on `ubuntu-latest`.

---

## [0.4.0] — 2026-05-10

### New Features
- Multi-LLM provider toggle (`:set`)
- Autostart daemon management via IPC (`AutostartStatus`, `AutostartEnable`, `AutostartDisable`)
- Async autostart status refresh — `:set` no longer blocks the UI
- Dynamic detail-panel title width based on panel pixel width and font size
- IME reset delay on macOS (45 ms) to prevent first-keystroke corruption after hotkey launch

### Improvements
- Icon prefetch optimized: deduplication and cache-key-based lookup
- `VirtualBrowseEntry` struct for web browse items
- `parse_combo_vkeys` helper reduces keybind parsing duplication
- `clippy -D warnings` — fixed `collapsible_match` and `unnecessary_sort_by` lints

### Bug Fixes
- Detail panel title overflow on long Korean filenames
- Ctrl+2 shortcut not firing on non-US keyboard layouts (added `logo()` and shifted-numeral matching)

---

## [0.3.x] and earlier

See git log for earlier history.
