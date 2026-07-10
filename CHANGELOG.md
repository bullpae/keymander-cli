# Changelog

All notable changes to keymander are documented here.

## [0.9.0] — 2026-07-10

Command-prefix UX release — 프리픽스 문법을 업계 관례에 맞추고 TUI/데스크톱 명령 표면을 통일.

### Refactoring
- **Prefix parser unified into kmd-core** — the TUI and desktop each carried their own `starts_with` chains that had drifted apart. A single `query_prefix::prefix_of` now serves both, and the `COMMANDS` registry (aliases, help title/usage, quick-template seed, icons) is the single source of truth for command dispatch, the `:help` list, and the docs.
- **Token-boundary alias matching** — aliases match only on exact input or alias-plus-space. `:pto` no longer triggers `:pt`, `:setup` no longer triggers `:set`, `:verbose` no longer triggers `:ver`.

### Features
- **TUI command parity** — `:help`, `:set` (opens the F2 settings modal), `:version`, `:keymap` (with start/stop/profile actions), `:keys` (TUI-specific cheatsheet), and `:f` folder search now work in the TUI, matching the desktop app. Folder search moved to `kmd_core::folder_search` (with `USERPROFILE` fallback for `~` on Windows).
- **Slash command aliases** — every `:` command can be typed with a leading `/` (`/help`, `/set`, `/calc 2+3`), matching Slack/Discord/ChatGPT conventions. The closed `/pattern/` regex form still wins; unknown `/...` falls back to normal search.
- **`>` shell alias** — `>command` works like `!command`, matching the PowerToys Run / Flow Launcher / Alfred convention.
- **DuckDuckGo-bang hint** — typing `!g rust` (a shell command here, a web search there) shows a one-line "switch to `@g rust`" hint under the shell item; Enter switches to the web search.
- **Unknown-command feedback** — a mistyped `:clac` shows an "unknown command → `:help`" hint at the top of the results (suppressed while typing a known command's prefix); normal search still runs underneath.

### Bug Fixes
- **Unix paths no longer misdetected as regex** — `/usr/bin/` (slashes inside the pattern) now falls back to fuzzy search instead of regex mode.
- **Help entries all seed a quick template** — selecting the Fuzzy/Glob/Regex example rows in `:help` now fills a starter query (previously dead rows); detection is keyword-based instead of sniffing the description string.

### Docs
- README prefix table synced with the code: added `:t` `:prompt` `:f` `:keys` `:keymap` `:version`, full multi-search alias list, token-boundary rule, `/` and `>` aliases, bang hint.

---

## [0.8.0] — 2026-07-09

### Refactoring
- **macOS backend unified onto the shared key-binding engine** — `macos.rs`'s own decision logic (~380 lines, a divergent copy of the Windows logic) now delegates to `keybind::engine` (extracted in 0.7.0, 21 unit tests). Both platforms share identical, tested behavior. The CGEventTap callback is now a thin adapter: flagsChanged → down/up translation, OS flag sync, decision execution.
- **Layer Launch deferral promoted to the engine** — launch actions bound inside a layer now wait for the trigger key release on *both* platforms (previously macOS-only; Windows fired immediately while the trigger modifier was still held).
- `KeyDecision::Execute` now carries `layer_trigger` context so macOS can clear residual trigger-modifier flags before synthetic events.

### Bug Fixes (macOS)
- **CapsLock remap now works** — the old flagsChanged branch never consulted `remaps`, so the `minimal` preset (CapsLock → Escape) silently did nothing on macOS.
- **Backend restart applies new config** — same `OnceLock` restart bug fixed on Windows in 0.7.0.

---

## [0.7.1] — 2026-07-08

### Bug Fixes
- **Linux shell timeout was ineffective** (found by CI) — on timeout, `child.kill()` only killed `sh`; grandchildren (e.g. `sleep`) kept the stdout pipe open, so the reader join blocked until the command finished on its own. The child is now spawned as a process-group leader (`setpgid`) and the whole group receives `SIGKILL` on timeout (the Unix counterpart of Windows `taskkill /T`). Reader results are collected via a channel with a 2 s grace `recv_timeout`, so even a process that escaped into a new session can't block the launcher.

### CI
- `cargo fmt` applied to 0.7.0 code (Format check green again).

---

## [0.7.0] — 2026-07-08

Follow-up hardening release — 0.6.0 감사에서 예고된 후속 과제 반영.

### Refactoring
- **Key-binding decision engine extracted** — all binding logic (modifier tracking, toggle, layer tap/hold, layer double-tap, combos, global double-tap, remaps) moved from the unsafe Windows hook callback into a pure, platform-independent `keybind::engine::EngineState`. `process_key(vkey, is_down, tick) → KeyDecision` takes time as a parameter, so timing behavior is now covered by **16 new unit tests** (tap-vs-hold, double-tap timeout, modifier-used-in-combo false-positive guard, toggle keeps Launch combos, u32 tick wraparound). The hook file now only installs the hook, translates events, and queues actions.

### Performance / Reliability
- **Hook actions run on a dedicated worker thread** — the low-level hook callback now queues actions over an mpsc channel (FIFO, key order preserved) and returns immediately. Long macros previously executed inside the callback and risked exceeding Windows' `LowLevelHooksTimeout`, which silently uninstalls the hook.

### Security
- **Windows single instance via named mutex** — replaces the PID-file check (TOCTOU race, PID-recycling false positives). The mutex name is derived from the data directory, so multiple portable installs don't interfere; the OS releases ownership on any kind of process death.

### Dependencies
- **bincode 1 → 2** — bincode 1.x is unmaintained (RUSTSEC advisory). Old-format index caches fail decoding gracefully and fall back to the JSON cache / full rebuild.
- **getrandom 0.2 → 0.3**.

### CI
- **macOS test SIGABRT fixed** — desktop unit tests sending `GotRawWindowId` reached the Carbon TIS API, which requires the main thread + a window-server session and aborts on headless CI runners. TIS calls are now skipped in test builds. (macOS CI has been red since 0.5.0.)

---

## [0.6.0] — 2026-07-08

Stability & security release — 코드 전반 감사에서 발견된 실버그와 보안 보완점 수정.

### Bug Fixes
- **Search engine reload duplication** — `SearchEngine::load()` now calls `nucleo.restart()` before injecting items. Previously every index rebuild (daemon `RebuildIndex`, TUI settings save) left deleted items in fuzzy results, accumulated duplicates, and leaked memory.
- **Filename → URL misdetection** — `is_url()` now uses a curated TLD whitelist. Previously any `name.ext` with a 2–6 letter alphabetic extension (`report.pdf`, `readme.md`, `config.toml`) was classified as a URL, which emptied search results. Extensions clashing with real TLDs (`md`, `rs`, `sh`, `ts`, `zip`, `mov`) are intentionally excluded — use `https://` or `www.` prefix to open those domains explicitly.
- **URL open respects selection** — URL-looking queries now show an "Open <url>" virtual item at the top of normal search results. Enter always executes the selected item (previously it either ignored the selection or did nothing when the list was empty).
- **Shell command timeout** — `!` commands are killed after 10 s (process tree on Windows via `taskkill /T`) with output capped at 256 KB. Previously `!ping -t` froze the launcher permanently.
- **Daemon shutdown hang** — `Shutdown` now wakes the main thread via a channel and unblocks the accept loop with a self-connect. Previously shutdown only completed because `kmd daemon stop` happened to poll with connects; other clients would leave the daemon waiting forever.
- **Keyboard hook restart** — restarting the backend now applies the new config; the `OnceLock` state previously ignored the second `start()` silently.
- **History pruning frequency** — the "5% probabilistic" pruning ran on *every* launch on Windows (`subsec_nanos()` is always a multiple of 100 → always a multiple of 20). Replaced with a deterministic once-per-20-launches counter.

### Security
- **Token file permissions** — `daemon.port` (contains the IPC auth token) is created with `0600` from the start on Unix, eliminating the write-then-chmod exposure window. Stale files are removed before re-creation.
- **IPC request size limit** — client requests are capped at 64 KB, preventing unbounded memory growth from a newline-less stream.
- **LIKE wildcard escaping** — history search no longer interprets `%`/`_` in user queries as SQL wildcards.

### Performance
- **Keyboard hook message loop** — replaced `PeekMessageW` + 1 ms sleep busy-wait (~1000 wakeups/s) with a blocking `GetMessageW` loop; `stop()` posts `WM_QUIT`. Reduces idle CPU/battery drain of the always-on daemon.
- **Daemon main loop** — replaced 200 ms shutdown-flag polling with a blocking channel wait.

### Refactoring
- `IpcError` migrated to `thiserror` (+`#[from]`); `DbError::Io` gains `#[from]`.
- `ProviderConfig` derives `Clone`, removing manual field-by-field copies.
- Silent empty-result fallbacks in history/bookmark queries now emit `tracing::warn!`.

---

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
