# Changelog

All notable changes to keymander are documented here.

## [Unreleased]

### Performance
- **인덱싱 소유권을 데몬으로 이동 — 데스크톱 실행 시 인덱싱 비용 제거** —
  기존에는 kmd-desktop이 첫 실행(하루 1회)에 24시간 지난 인덱스를 직접
  재빌드했다. 이제 데몬이 시작 시 + `launcher.index_refresh_minutes` 주기
  (기본 360분, 0=off)로 전체/quick 인덱스를 백그라운드 재빌드해 공유
  캐시를 갱신하고 데몬 검색 엔진도 함께 교체한다. kmd-desktop은 언제 떠도
  캐시 히트로 즉시 로드하며, 24시간 freshness 재빌드는 데몬이 꺼져 있을
  때의 폴백으로만 남는다. IPC `RebuildIndex`도 캐시를 함께 저장한다.
- **quick 인덱스 캐시 신선도 적용** — quick 캐시(앱/PATH)는 영구 캐시라
  새로 설치한 앱이 full 엔진 교체 전까지 안 보였다. 데몬 리프레셔가 quick
  캐시도 주기 갱신하고, 데스크톱 쪽에도 24시간 freshness 폴백을 추가.
- **인덱스 캐시 원자적 쓰기 (tmp+rename)** — 데몬이 백그라운드로 캐시를
  쓰는 동안 데스크톱이 읽어도 잘린 파일을 보지 않는다.

### Bug Fixes
- **(Windows) 카드 테두리를 창 가장자리에 완전 밀착 — 비대칭 마진 제거** —
  카드 바깥 링(1px)+간격(2px)이 불투명 창 배경색 띠로 노출되고, DPI 소수
  배율에서 우측이 서브픽셀로 잘려 상하좌우 마진이 미묘하게 달라 보이던
  문제. Windows에서는 링·간격을 제거해 teal 테두리가 창 가장자리와
  일치하고(라운드 8px = DWM 코너 클립 정합), 창 높이 공식·테스트도
  `CARD_PAD` 상수로 정리. macOS 투명 pill의 이중 테두리는 유지.
- **TUI /exit 입력 중 한글 자모 오입력 수정** — 쿼리가 `/e`가 되는 순간
  이모지 프리픽스로 판정해 내장 한글 조합이 자동 활성화되면서, /exit의
  x가 'ㅌ'로 조합돼 `/eㅌit`이 되던 버그. `:emoji`를 치는 중에도 같은
  사고가 났다. 이제 별칭 뒤 공백이 와서 키워드 입력이 실제로 시작된
  뒤(`:e fire`)에만 자동 활성화한다.
- **deploy-local.ps1 이 `--help`를 배포 경로로 해석하던 사고 방지** —
  PowerShell은 이중 대시 토큰을 위치 인자로 바인딩해 `$DeployDir="--help"`
  가 되고, 리포 옆에 `--help` 폴더를 만들어 배포한 뒤 그 안의 데몬을
  띄워버렸다(실제 설치본은 미갱신). 이제 -h/--help/help는 사용법을
  출력하고, 옵션처럼 생긴 값·상대 경로는 배포 경로로 거부한다.
  deploy-local.sh도 --help와 알 수 없는 인자 거부를 추가.
- **레이어 더블탭 오토리피트 오판정 수정** — Alt+I/O/`/`를 누르고 있으면
  OS 오토리피트 down이 매번 새 탭으로 계산돼 single↔double 액션이 교대
  발사됐다 (Alt+`/` 홀드 시 Delete와 "줄 전체 삭제" 매크로가 번갈아 실행
  되는 파괴적 오작동). 이제 up 없이 반복된 down은 오토리피트로 인식해
  single 액션만 반복하고(연속 단어 이동), double 액션 직후의 리피트는
  억제한다 (Windows/macOS 공통 엔진 수정).
- **트리거 선해제 시 맨키 누출 차단** — Alt+H 홀드 중 Alt를 먼저 떼면
  계속 눌려 있는 H의 오토리피트가 맨키 'h'로 새어나가 문자가 입력되던
  문제. 레이어가 소비한 키는 keyup까지 추적·억제한다.
- **레이어 활성 전부터 눌려 있던 매핑 키의 keyup 억제 해제** — 해당 키의
  up이 OS에 전달되지 않아 stuck 상태가 되던 문제 (우리가 소비한 down의
  up만 억제).
- **Cmd/Ctrl+Alt+매핑 키 = OS 조합 보존** — Cmd+Alt+H("다른 앱 가리기")
  같은 조합이 레이어 매핑(Left 등)으로 오발사되던 문제. 트리거 외의
  비-Shift 수정자가 함께 눌린 키는 매핑 대신 트리거 조합으로 OS에
  투과한다 (passthrough 레이어).

### Features
- **브랜드 아이콘 모노 모드 (`general.brand_icons = "mono"`)** — 구글·네이버·
  GPT 등 웹 서비스 브랜드 아이콘을 풀컬러 로고 대신 Simple Icons(CC0) 단색
  글리프로 렌더링하는 옵션. 테마의 WebSearch 색(teal)으로 틴트되고 시스템
  아이콘과 같은 12% 알파 컨테이너에 얹혀 목록 전체가 한 톤으로 통일된다.
  `:set`의 "Brand Icons: Mono" 토글로 즉시 전환·저장 가능. 글리프가 없는
  서비스(grok/daum/papago)는 시스템 아이콘 폴백으로 흘리되 같은 teal로
  틴트해 톤을 유지. 기본값은 "color"(기존 풀컬러 로고).
- **데스크톱 시스템 아이콘 전면 교체 — 이모지 → 테마 틴트 SVG** — 시스템
  명령·프리픽스 명령·파일 확장자·키맵 치트시트 등 kmd-core가 이모지로
  내려주던 아이콘 90여 종을 데스크톱에서 Lucide SVG(ISC)로 오버라이드.
  아이콘은 카테고리별 시맨틱 컬러(teal/green/yellow/peach/red)로 틴트되고
  12% 알파 라운드 컨테이너 위에 얹혀 렌더링된다. `stroke="currentColor"`
  기반이라 5개 테마 전부 자동 추종. 브랜드 PNG → 시스템 SVG → 이모지
  텍스트 3단계 폴백으로 kmd-core/TUI는 무수정 (`system_icons.rs`,
  brand_icons 패턴 미러). `:emoji` 검색 결과는 실제 이모지를 유지한다.
- **Shift+네비 키 = 선택 확장 (macOS)** — Shift를 누른 채 Alt+H/J/K/L을
  누르면 Shift+화살표로 합성돼 텍스트 선택이 확장된다. 레이어 액션 실행
  시 트리거(Alt) 플래그만 지우고 함께 눌린 물리 수정자는 보존하도록 변경
  (Windows는 물리 Shift가 통과해 이미 동작).

## [0.10.2] — 2026-07-19

Windows 렌더링·성능 정비 — 검은 화면 근본 해결(불투명 창 전환) +
부팅/입력 핫패스 최적화.

### Bug Fixes
- **Windows 검은 화면 근본 해결 — 투명 창 포기, 불투명 창 + DWM 라운드
  코너로 전환** — v0.10.1의 창 접기로도 남아 있던 검은 영역(상단 드래그
  스트립 6px, 좌우 리사이즈 엣지 4px, pill 라운드 모서리 바깥, "No results
  found" 힌트 배경)은 모두 투명 픽셀이었다. iced/wgpu는 Windows(DX12·VM·
  소프트웨어 폴백)에서 창 단위 알파 합성이 신뢰할 수 없어 투명 픽셀이
  검게 그려진다. 이제 Windows에서는 창 배경을 테마 색으로 불투명하게
  칠하고, 라운드 코너는 DWM 네이티브 클립(`DWMWCP_ROUND`)으로 처리한다 —
  렌더러가 무엇으로 폴백하든 검은 픽셀이 나올 수 없는 구조.
  (macOS/Linux는 기존 투명 pill 유지)

### Performance
- **부팅: quick 인덱스 로드를 비동기로** — 기존에는 창을 만들기 전에 PATH
  스캔/캐시 로드를 동기로 수행해, 캐시 미스(첫 실행)나 느린 VM에서 창
  표시가 수백 ms~수 초 지연됐다. 이제 빈 엔진으로 즉시 창을 띄우고 quick
  인덱스는 백그라운드에서 로드 후 교체한다 (quick → full 2단계 워밍업은
  기존 그대로).
- **입력: 키 입력마다 하던 SQLite 히스토리 조회 제거** — frecency 부스트가
  매 검색마다 `query_history(500)` + 맵 재구축을 수행했다. 부팅 시 1회
  로드한 맵을 재사용한다 (`history::boost_results_with_map`).
- **입력: 불필요한 리렌더 프레임 제거** — 아이콘 prefetch가 새로 추출한
  아이콘이 없어도 매번 `IconsReady` 리렌더를 유발하던 것을, 실제로 새
  아이콘이 생겼을 때만 보내도록 변경. 소프트웨어 렌더러(VM)에서 프레임
  비용이 커 체감 효과가 크다.
- **`general.renderer` 설정 추가** (`auto`/`software`/`gpu`) — VM·원격
  데스크톱처럼 GPU가 부실한 환경에서 `software`로 지정하면 wgpu 어댑터
  프로빙(수 초 소요 가능)을 생략하고 tiny-skia로 직행한다. 부팅 단계별
  소요 시간 로그도 추가 (`desktop.log`).

### Docs
- **README 전면 개편 — 이야기 → 체험 → 습득 구조** — 프로젝트 동기(세 가지
  "이탈")와 철학 3원칙("홀드 손 ≠ 조작 손" 포함)을 서두에, "첫 60초" 최소
  경로와 미션 시나리오 5개(소환·vim-nav·모드탭·마우스 레이어·크로스 OS)를
  체험 코스로 추가. 기존 레퍼런스 표는 뒤로 재배치, v0.3.x 이력은
  CHANGELOG로 이관. 한국어판 `README.ko.md` 신설.
- **`kmd dojo` 계획 문서** (`docs/10_dojo_plan.md`) — 미션을 점수·콤보가
  있는 TUI 연습 게임으로 만드는 인터랙티브 트레이너 설계: 매핑 결과 판정
  아키텍처, 레벨 5종, 마일스톤 M1~M4.

## [0.10.1] — 2026-07-17

데스크톱 런처 검은 화면 핫픽스 — 투명 합성이 안 되는 환경(Windows on ARM
VM, 소프트웨어 렌더러 폴백 등) 대응.

### Bug Fixes
- **결과 없을 때 검색바 아래 거대한 검은 사각형이 보이던 문제** — 창은 항상
  full 높이(검색바+결과 10여 줄)로 만들고 빈 영역을 투명 픽셀로 채우는
  구조였는데, GPU/드라이버가 창 투명 합성(alpha mode)을 지원하지 않으면
  (VMware의 Windows on ARM 등 wgpu가 `Opaque`로 폴백하는 환경) 투명 영역이
  전부 검은색으로 렌더링됐다. 이제 결과가 없으면 창 자체를 검색바(pill)
  높이로 접고, 결과가 생기면 full 높이로 확장한다 — 렌더러와 무관하게 동작.
  부수 효과: 대기 상태에서 화면 1/3을 덮던 보이지 않는 클릭 차단 영역도
  사라져 아래 앱 클릭이 그대로 통과된다.

## [0.10.0] — 2026-07-17

tap-hold(모드탭)·마우스 레이어 릴리스 — HHKB 스타일 CapsLock 모드탭과
RAlt 홀드 마우스 레이어 추가.

### Features
- **HHKB 스타일 CapsLock 모드탭 (tap-hold)** — 짧게 탭 = CapsLock, 홀드 중
  다른 키 = Ctrl 조합. 다른 키를 누르는 순간 즉시 hold로 판정되어
  Ctrl+C 등이 타임아웃 대기 없이 동작한다. vim-nav 프리셋 기본값(Windows),
  minimal 프리셋은 tap=Esc/hold=Ctrl로 진화. macOS는 OS 자체 tap(한영)/
  hold(캡스락)와 충돌하므로 기본값에서 제외. kanata 프리셋에도 동일 반영
  (`tap-hold-press`). `[launcher.keymap.tap_holds.<키>]`로 커스터마이징.
- **마우스 레이어 (VIA 스타일 mouse keys)** — RAlt 홀드 → 왼손 마우스 조작.
  홀드 손(오른엄지)과 조작 손(왼손)을 분리한 배치: WASD 포인터 이동
  (180→1300px/s 시간 가속, 125Hz 워커), Space 좌클릭(홀드=드래그),
  J/K/L 좌/우/중 클릭, LShift 저속 정밀 모드. 미매핑 키는 차단(오타 방지).
  RAlt 짧게 탭 = 한/영 유지 (Windows 한국어 배열의 물리 RAlt=한/영 키 별칭
  매칭 포함). Windows(SendInput)/macOS(CGEvent, 드래그 이벤트 합성) 네이티브
  구현 + kanata 프리셋(`movemouse-accel-*`) 동일 배치. 레이어 트리거 해제·
  keymap 토글 시 이동/버튼 전체 정지(stuck-mouse 방지).

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
