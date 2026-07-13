# Changelog

All notable changes to keymander are documented here.

## [0.9.5] — 2026-07-13

리팩토링·보안 정비 릴리스 — 0.9.4 패스쓰루 진단 과정에서 드러난
구조 문제(설정 에러 무시, 기본값 이원화)와 잠복 리스크를 일괄 해소.

### Bug Fixes
- **config.toml 파싱 에러가 조용히 무시되던 문제** — TOML 문법 오류
  (테이블 중복 정의 등) 시 데몬이 로그 한 줄 없이 전체 기본값으로
  폴백했다. 이제 에러 로그(경로 + 줄 번호)를 남기고 `kmd daemon status`에
  ⚠ 경고로 표시된다. 데몬은 여전히 기본값으로 계속 동작한다.
- **`vim-nav.kbd`처럼 확장자 붙은 프로필에서 치트시트가 프리셋을 안 보여주던
  문제** — 프로필 판별을 daemon과 치트시트가 다르게 하던 것을
  `profile_kind()`로 통일.
- **`none` 프로필이 사용자 커스텀 레이어를 끄지 않던 문제** — 문서된 대로
  키맵 전체가 비활성화된다 (global_hotkey는 유지).

### Security / Privacy
- **IPC 인증 토큰이 포터블 설치 위치에 노출되던 문제** — 런타임 파일
  (daemon.port/pid/log)을 포터블 모드와 무관하게 항상 OS 표준 사용자
  디렉터리에 기록한다. USB·공용 폴더에 설치해도 다른 로컬 계정이 토큰을
  읽을 수 없다. 포터블 모드의 이동성(config·데이터 = kmd-data/)은 그대로.
  ⚠ 업데이트 후 구버전 데몬이 떠 있으면 CLI가 찾지 못한다 — 데몬 재시작 필요.
- **훅 로그에 실제 타이핑 키 비기록** — chord engage 디버그 로그 등이
  사용자가 누른 키 이름을 남기던 것을 제거. 트리거(config 값)만 로그한다.

### Refactoring
- **키맵 기본값·병합을 kmd-core `effective_keymap`으로 단일화** — vim-nav
  기본 레이어가 daemon과 kmd-core 두 곳에 하드코딩되어 드리프트가
  반복되던 구조 해소 (-330줄). 병합이 TOML(Option) 수준에서 수행되어
  "생략"과 "명시적 기본값"이 구분된다 — 프리셋 기본이 바뀌어도 사용자
  레이어가 조용히 되돌아가지 않음.
- **macOS 액션 실행을 워커 스레드로 이관** — 탭 콜백에서 sleep 포함
  액션이 동기 실행되어 kCGEventTapDisabledByTimeout을 유발할 수 있던
  구조를 Windows(0.7.0)와 동일한 큐잉 모델로 통일. 실기기 검증 필요.
- **Windows VK 역변환을 정방향 match에서 자동 생성** — 거울상 match
  두 벌 유지로 인한 불일치 가능성 제거, 왕복 테스트 추가.
- 엔진 핫패스(키 이벤트마다)의 불필요한 Vec 할당 제거.

---

## [0.9.4] — 2026-07-12

Passthrough 진단 릴리스 — 0.9.3의 Windows 검증에서 "Alt+Tab이 Tab처럼 동작"
증상이 보고되어, 설정이 엔진까지 도달했는지 원격으로 확인할 수단을 추가.

### Bug Fixes
- **`:keymap` 치트시트가 사용자의 `unmapped` 설정을 무시하던 문제** —
  vim-nav 프리셋 병합(`effective_keymap`)이 새 필드를 복사하지 않았다.
  엔진(데몬) 경로는 영향 없음 — 표시만 잘못됐다.

### Diagnostics
- **`kmd daemon status`에 실행 중인 레이어 요약 표시** — 트리거·unmapped
  모드·매핑 수를 그대로 보여줘, 설정 파일이 실제 엔진에 적용됐는지 즉시
  확인할 수 있다 (`레이어: nav: LAlt 홀드 · unmapped=Passthrough · …`).
- **데몬 로그를 `<데이터 디렉터리>/daemon.log`로 기록** — 기존에는
  stdout/stderr가 전부 버려져 키맵 파싱 경고를 볼 방법이 없었다.
  시작마다 새로 쓰며, 경로는 status 출력에 표시된다.

---

## [0.9.3] — 2026-07-12

VIA-style layer passthrough (docs/08 P0–P3) — 레이어 트리거(Alt)를 눌러도
Alt+Tab 같은 OS 조합을 잃지 않는 코드(chord) 모드 도입.

### Features
- **Layer passthrough (`unmapped = "passthrough"`)** — while a layer is held,
  pressing a key that has no layer mapping now enters *chord mode*: the trigger
  and the key are injected to the OS in order, so native combos (Alt+Tab,
  Alt+F4 on Windows; Option-key characters on macOS) work exactly as without
  keymander. Everything in that hold passes to the OS until the trigger is
  released; the layer's tap action does not fire. Opt-in per layer — the
  default (`"plain"`) keeps the previous behavior, and `"block"` (VIA `KC_NO`)
  suppresses unmapped keys entirely.
- Engine guarantees: chord release is injected on keymap toggle and daemon
  stop (no stuck modifiers); deferred layer `launch:` actions still run after
  the chord ends. 9 new engine unit tests.

### Packaging
- First release shipping `.deb`/`.rpm` packages (x86_64 Linux) as release
  assets, alongside the SHA256SUMS.txt introduced in 0.9.2.

---

## [0.9.2] — 2026-07-12

### Bug Fixes
- **Long-running shell commands no longer killed after 10 s (TUI)** — `>`/`!` user commands in the TUI now open in a real terminal window (same UX as the desktop app) instead of running hidden with a 10-second timeout that aborted commands like `>winget upgrade --all` mid-run. Quick actions (`!ip`, `!uptime`, …) keep the inline capture + clipboard behavior.
- **macOS terminal launch works without Automation permission** — shell commands now run via a self-deleting temp `.command` script opened with `open -a Terminal`, replacing the osascript/AppleEvent approach that silently failed for non-bundled binaries without a TCC prompt. The window shows the exit status and waits for Enter.
- **Windows: quoted arguments survive `cmd /k`** — the command line is passed via `raw_arg`, fixing commands containing quotes that std's `\"` escaping (which cmd.exe doesn't understand) used to mangle.

### Refactoring
- Terminal launch unified into `kmd_core::plugin::builtin_shell::launch_in_terminal` — TUI and desktop share one implementation; the desktop's private copy is removed.

---

## [0.9.1] — 2026-07-11

### Bug Fixes
- **Windows binaries no longer require the VC++ Redistributable** — 0.9.0's MSVC builds dynamically linked the CRT, so `kmd daemon start` failed with a missing-`VCRUNTIME140.dll` error on a clean Windows install. All MSVC-target builds (x86_64/aarch64) now statically link the CRT via `.cargo/config.toml` (`-C target-feature=+crt-static`); the binaries run standalone.

---

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
