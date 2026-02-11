# Architecture Document

## 1. 시스템 아키텍처

### 1.1 3-Layer 설계

```mermaid
graph TB
    subgraph Layer3 ["Layer 3: User Interface"]
        TUI["TUI<br/>ratatui + crossterm"]
        CLI["CLI Commands<br/>kmd search / launch / ..."]
        Scripts["Scripts / Automation<br/>kmd search --json | jq"]
    end

    subgraph Layer2 ["Layer 2: CLI Routing (src/cmd/)"]
        ClapRouter["clap Parser<br/>서브커맨드 라우팅"]
        Helpers["Helpers<br/>load_config, open_db, load_or_build_index"]
    end

    subgraph Layer1 ["Layer 1: kmd-core (library)"]
        Index["Index<br/>apps, files, path, system_commands"]
        Search["Search<br/>fuzzy, glob, regex, contains"]
        DB["Database<br/>history, bookmarks, kv_store"]
        Config["Config<br/>TOML load/save, get/set"]
        Plugin["Plugin<br/>Extension trait, loader, calc"]
        Action["Action<br/>execute, open_url, open_with_system"]
        Web["Web<br/>@prefix services, URL builder"]
        History["History<br/>boost_results, record_launch"]
    end

    subgraph Infra ["Infrastructure"]
        SQLite["SQLite (WAL)"]
        TOML["config.toml"]
        JSON["index.json cache"]
        PluginDir["plugins/ directory"]
    end

    TUI --> ClapRouter
    CLI --> ClapRouter
    Scripts --> ClapRouter
    ClapRouter --> Helpers
    Helpers --> Index
    Helpers --> Search
    Helpers --> DB
    Helpers --> Config
    Helpers --> Action
    Helpers --> Web
    Helpers --> History
    Helpers --> Plugin

    DB --> SQLite
    Config --> TOML
    Index --> JSON
    Plugin --> PluginDir
```

### 1.2 핵심 원칙

- **kmd-core는 UI를 모른다**: 순수 라이브러리, TUI/CLI 의존성 없음
- **CLI는 얇은 레이어**: clap으로 인자 파싱 후 kmd-core 함수 호출
- **TUI는 프론트엔드**: kmd-core API만 호출, 비즈니스 로직 없음
- **단방향 의존성**: TUI → CLI helpers → kmd-core (역방향 없음)

### 1.3 의존성 방향

```mermaid
graph LR
    TUI["src/tui/"] --> CMD["src/cmd/"]
    CMD --> Core["kmd-core"]
    TUI --> Core
    Core --> SQLite["rusqlite"]
    Core --> Nucleo["nucleo"]
    Core --> Serde["serde + toml"]
    TUI --> Ratatui["ratatui"]
    TUI --> Crossterm["crossterm"]
    CMD --> Clap["clap"]
```

---

## 2. 모듈 구조

### 2.1 kmd-core 모듈 맵

```mermaid
graph TB
    lib["lib.rs<br/>모듈 선언 + re-export"]

    lib --> config["config.rs<br/>Config, GeneralConfig<br/>LauncherConfig, Keybindings"]
    lib --> db["db.rs<br/>Database (SQLite)<br/>history, bookmarks, kv"]
    lib --> search["search.rs<br/>SearchEngine (Nucleo)<br/>SearchMode, GlobMatcher"]
    lib --> history["history.rs<br/>boost_results<br/>record_launch"]
    lib --> action["action.rs<br/>execute, open_url<br/>open_with_system"]
    lib --> web["web.rs<br/>WebService, parse_web_query<br/>build_search_url"]

    lib --> indexMod["index/mod.rs<br/>Index, IndexItem<br/>ItemKind, Source"]
    indexMod --> apps["index/apps.rs<br/>collect_apps<br/>OS별 앱 발견"]
    indexMod --> files["index/files.rs<br/>ProviderKind<br/>detect_provider, collect_files"]
    indexMod --> path["index/path.rs<br/>collect_executables<br/>PATH 스캔"]
    indexMod --> sysCmds["index/system_commands.rs<br/>SystemCommand 정의<br/>플랫폼별"]
    indexMod --> store["index/store.rs<br/>save_index, load_index<br/>JSON 캐시"]

    lib --> pluginMod["plugin/mod.rs<br/>Extension trait<br/>ExtensionAction"]
    pluginMod --> calc["plugin/builtin_calc.rs<br/>CalcExtension<br/>수식 평가기"]
    pluginMod --> loader["plugin/loader.rs<br/>discover_plugins<br/>manifest.toml 파싱"]
    pluginMod --> protocol["plugin/protocol.rs<br/>PluginRequest/Response<br/>JSON 프로토콜"]
```

### 2.2 바이너리 모듈 맵

```mermaid
graph TB
    main["main.rs<br/>clap Parser<br/>서브커맨드 라우팅"]

    main --> cmdMod["cmd/mod.rs<br/>load_config, open_db<br/>load_or_build_index"]
    cmdMod --> cmdSearch["cmd/search.rs<br/>kmd search query"]
    cmdMod --> cmdLaunch["cmd/launch.rs<br/>kmd launch target"]
    cmdMod --> cmdIndex["cmd/index.rs<br/>kmd index --rebuild"]
    cmdMod --> cmdConfig["cmd/config.rs<br/>kmd config get/set"]
    cmdMod --> cmdHistory["cmd/history.rs<br/>kmd history list/clear"]
    cmdMod --> cmdPlugin["cmd/plugin.rs<br/>kmd plugin list"]
    cmdMod --> cmdDaemon["cmd/daemon.rs<br/>kmd daemon start/stop"]

    main --> tuiMod["tui/mod.rs<br/>run()"]
    tuiMod --> app["tui/app.rs<br/>AppState, run_app<br/>handle_key, update_search"]
    tuiMod --> event["tui/event.rs<br/>EventHandler<br/>AppEvent"]
    tuiMod --> theme["tui/theme.rs<br/>Theme colors/styles"]
    tuiMod --> uiMod["tui/ui/mod.rs<br/>render() 레이아웃"]
    uiMod --> input["tui/ui/input.rs<br/>검색 입력바"]
    uiMod --> list["tui/ui/list.rs<br/>결과 리스트"]
    uiMod --> preview["tui/ui/preview.rs<br/>미리보기 패널"]
```

---

## 3. 데이터 흐름

### 3.1 검색 흐름

```mermaid
sequenceDiagram
    actor User
    participant TUI as TUI / CLI
    participant Engine as SearchEngine
    participant Nucleo as Nucleo (fuzzy)
    participant DB as SQLite

    User->>TUI: 입력 "fire"
    TUI->>Engine: search("fire", 50)
    Engine->>Engine: SearchMode::detect("fire") → Fuzzy
    Engine->>Nucleo: update_pattern("fire") + tick()
    Nucleo-->>Engine: matched_items (score 포함)
    Engine-->>TUI: Vec of SearchResult

    TUI->>DB: query_history(500)
    DB-->>TUI: frequency map
    TUI->>TUI: boost_results (score += freq * 100)
    TUI->>TUI: 정렬 후 렌더링
    TUI-->>User: 결과 리스트 표시
```

### 3.2 실행 흐름

```mermaid
sequenceDiagram
    actor User
    participant TUI
    participant Action as action.rs
    participant OS as OS (cmd/open/xdg-open)
    participant DB as SQLite

    User->>TUI: Enter (Firefox 선택)
    TUI->>Action: execute(SearchResult)

    alt App / File / Directory
        Action->>OS: open_with_system(path)
        OS-->>Action: Launched
    else SystemCommand
        Action->>Action: find_by_display_name()
        alt confirm 필요
            Action-->>TUI: NeedsConfirmation
            TUI-->>User: 확인 다이얼로그
        else confirm 불필요
            Action->>OS: Command::new(cmd).spawn()
            OS-->>Action: Launched
        end
    else WebSearch
        Action->>OS: open_url(url)
        OS-->>Action: OpenedUrl
    end

    Action-->>TUI: ActionResult
    TUI->>DB: record_launch(type, path, name)
    DB-->>TUI: OK (frequency +1)

    alt quit_on_launch = true
        TUI-->>User: 종료
    end
```

### 3.3 인덱스 빌드 흐름

```mermaid
flowchart TB
    Start["kmd index --rebuild<br/>또는 첫 실행"] --> Build["Index::build(LauncherConfig)"]

    Build --> Step1["1. path::collect_executables()<br/>PATH 환경변수 스캔<br/>중복 제거, 히든 파일 제외"]
    Build --> Step2["2. system_commands::collect()<br/>플랫폼별 정적 목록<br/>shutdown, lock, ..."]
    Build --> Step3["3. apps::collect_apps()<br/>OS별 앱 발견"]
    Build --> Step4["4. files::collect_files()<br/>파일 프로바이더"]

    Step3 --> Win["Windows: Start Menu .lnk"]
    Step3 --> Mac["macOS: /Applications/*.app"]
    Step3 --> Linux["Linux: .desktop 파일 (XDG)"]

    Step4 --> Detect["auto-detect provider"]
    Detect --> Everything["Everything (es.exe)"]
    Detect --> WinFs["PowerShell"]
    Detect --> Spotlight["mdfind"]
    Detect --> Fd["fd / fdfind"]
    Detect --> Locate["plocate / locate"]
    Detect --> Builtin["builtin (none)"]

    Step1 --> Merge["모든 IndexItem 합치기"]
    Step2 --> Merge
    Win --> Merge
    Mac --> Merge
    Linux --> Merge
    Everything --> Merge
    WinFs --> Merge
    Spotlight --> Merge
    Fd --> Merge
    Locate --> Merge
    Builtin --> Merge

    Merge --> Save["store::save_index()<br/>→ data_dir/kmd/index.json"]
    Save --> Done["다음 실행 시<br/>캐시에서 즉시 로드"]
```

### 3.4 TUI 이벤트 루프

```mermaid
stateDiagram-v2
    [*] --> Init: kmd (인자 없음)
    Init --> LoadConfig: load config + index + db
    LoadConfig --> Render: 첫 렌더링

    Render --> WaitEvent: terminal.draw()
    WaitEvent --> HandleKey: Key event
    WaitEvent --> Render: Tick / Resize

    HandleKey --> Quit: Ctrl+C / Esc (빈 쿼리)
    HandleKey --> ClearQuery: Esc (쿼리 있음)
    HandleKey --> Navigate: Up / Down
    HandleKey --> TogglePreview: Ctrl+P
    HandleKey --> Execute: Enter
    HandleKey --> UpdateSearch: 문자 입력 / Backspace

    ClearQuery --> ShowHistory: 히스토리 로드
    ShowHistory --> Render
    Navigate --> Render
    TogglePreview --> Render
    UpdateSearch --> SearchEngine: engine.search()
    SearchEngine --> BoostResults: history boost
    BoostResults --> Render
    Execute --> ActionExecute: action::execute()
    ActionExecute --> RecordHistory: DB 기록

    RecordHistory --> Quit: quit_on_launch
    RecordHistory --> Render: 계속

    Quit --> [*]: 터미널 복원
```

---

## 4. 파일 시스템 레이아웃

### 4.1 설정/데이터 디렉토리

| OS | Config | Data |
|----|--------|------|
| Linux | `~/.config/kmd/` | `~/.local/share/kmd/` |
| macOS | `~/Library/Application Support/kmd/` | 동일 |
| Windows | `%APPDATA%/kmd/` | `%LOCALAPPDATA%/kmd/` |

### 4.2 파일 구조

```mermaid
graph LR
    subgraph ConfigDir ["config_dir/"]
        ConfigToml["config.toml<br/>사용자 설정"]
    end

    subgraph DataDir ["data_dir/"]
        KmdDB["kmd.db<br/>SQLite (히스토리, 북마크, KV)"]
        IndexJSON["index.json<br/>인덱스 캐시"]
        subgraph PluginsDir ["plugins/"]
            subgraph CalcPlugin ["kmd-calc/"]
                CalcManifest["manifest.toml"]
                CalcScript["calc.py"]
            end
        end
    end
```

---

## 5. 기술 스택

| 역할 | 기술 | 선택 이유 |
|------|------|----------|
| 언어 | Rust 2021 | 단일 바이너리, 크로스컴파일, 메모리 안전 |
| TUI | Ratatui 0.29 + Crossterm 0.28 | 가장 성숙한 Rust TUI, 크로스플랫폼 터미널 |
| 퍼지 검색 | Nucleo 0.5 | helix editor와 동일, 빠르고 정확 |
| DB | rusqlite 0.32 (bundled) | 외부 의존성 없는 SQLite, WAL 지원 |
| CLI | clap 4 (derive) | Rust 표준 CLI 파서 |
| 설정 | serde + toml 0.8 | 사람이 읽기 쉬운 포맷 |
| 비동기 | tokio 1 | TUI 이벤트 루프용 |
| 에러 | color-eyre 0.6 + thiserror 2 | 사용자 친화적 에러 메시지 |
| 로깅 | tracing 0.1 | 구조적 로깅, 환경변수 필터 |
| 파일 탐색 | walkdir 2 | 내장 파일 스캔 폴백용 |

---

## 6. 크로스플랫폼 전략

### 6.1 플랫폼별 분기 맵

```mermaid
graph TB
    subgraph Common ["공통 코드"]
        SearchEngine
        Database
        Config
        PluginSystem
    end

    subgraph PlatformSpecific ["플랫폼별 분기 (#[cfg])"]
        subgraph WindowsCode ["Windows"]
            WinApps["apps.rs: Start Menu .lnk 스캔"]
            WinFiles["files.rs: Everything / PowerShell"]
            WinSys["system_commands.rs: shutdown /s"]
            WinAction["action.rs: cmd /c start"]
        end
        subgraph MacCode ["macOS"]
            MacApps["apps.rs: /Applications/*.app"]
            MacFiles["files.rs: mdfind (Spotlight)"]
            MacSys["system_commands.rs: osascript"]
            MacAction["action.rs: open"]
        end
        subgraph LinuxCode ["Linux"]
            LinuxApps["apps.rs: .desktop 파일 (XDG)"]
            LinuxFiles["files.rs: fd / locate"]
            LinuxSys["system_commands.rs: systemctl"]
            LinuxAction["action.rs: xdg-open"]
        end
    end

    Common --> PlatformSpecific
```

### 6.2 Crossterm 이벤트 처리

Windows에서 Crossterm은 KeyPress + KeyRelease 이벤트를 모두 발생시킨다.
중복 방지를 위해 `KeyEventKind::Press`만 처리:

```rust
if key.kind == KeyEventKind::Press {
    Ok(AppEvent::Key(key))
}
```

---

## 7. 보안 고려사항

```mermaid
flowchart LR
    subgraph Threats ["위협"]
        ReDoS["ReDoS<br/>악의적 정규식"]
        PathInjection["경로 인젝션<br/>Everything es.exe"]
        PluginMalware["플러그인 악성코드"]
        SQLInjection["SQL 인젝션"]
        DangerousCmd["위험 시스템 명령"]
    end

    subgraph Mitigations ["대응"]
        RegexLimit["패턴 200자 제한<br/>컴파일 크기 1MB 제한"]
        FileValidation["es.exe/es 파일명만 허용"]
        ProcessIsolation["프로세스 격리<br/>timeout 5초"]
        ParamBinding["파라미터 바인딩 전용<br/>문자열 결합 없음"]
        ConfirmFlag["confirm 플래그<br/>사전 확인 요구"]
    end

    ReDoS --> RegexLimit
    PathInjection --> FileValidation
    PluginMalware --> ProcessIsolation
    SQLInjection --> ParamBinding
    DangerousCmd --> ConfirmFlag
```
