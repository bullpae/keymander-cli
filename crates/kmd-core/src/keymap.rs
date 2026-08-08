//! Keymap backend integration — Kanata 프로세스 관리 + 내장 프로파일 갤러리.
//!
//! keymander는 kanata 프로파일을 관리하는 도구 역할을 한다.
//! kanata 프로세스는 독립적으로 실행되며, PID 파일 기반 loose coupling.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Config;

// ── 내장 프로파일 프리셋 ──────────────────────────────────────────────────────

/// 내장 프리셋 정의: (이름, 설명, .kbd 내용)
pub struct BuiltinPreset {
    pub name: &'static str,
    pub description: &'static str,
    pub content: &'static str,
}

pub const BUILTIN_PRESETS: &[BuiltinPreset] = &[
    BuiltinPreset {
        name: "vim-nav",
        description: "Alt 홀드 → Vim 네비게이션, RAlt 홀드 → 마우스, CapsLock tap/hold → Caps/Ctrl",
        content: VIM_NAV_KBD,
    },
    BuiltinPreset {
        name: "minimal",
        description: "CapsLock → 탭 Esc / 홀드 Ctrl 최소 맵핑",
        content: MINIMAL_KBD,
    },
];

const VIM_NAV_KBD: &str = r#";; ============================================================
;; keymander vim-nav 프로파일
;; Alt 홀드 → Vim 스타일 네비게이션 + Alt+Space → kmd-desktop 실행
;; CapsLock → 짧게 탭 = CapsLock, 홀드 = Ctrl (HHKB 스타일)
;; RAlt 홀드 → 마우스 레이어 (WASD 이동 + Space/J/K/L 클릭)
;;
;; 의존: kanata (https://github.com/jtroo/kanata)
;; 생성: kmd keymap init vim-nav
;; ============================================================

;; ── 1. 환경 설정 ──────────────────────────────────────────────
(defcfg
  process-unmapped-keys yes
  danger-enable-cmd yes
)

;; ── 2. 물리 키 매핑 ──────────────────────────────────────────
(defsrc
  caps alt  ralt lsft spc  w    a    s    d    h    j    k    l    n    m    i    o    .    /    y    p
)

;; ── 3. 기본 레이어 ──────────────────────────────────────────
(deflayer default
  @caps-th @nav-mod @mouse-mod lsft spc  w    a    s    d    h    j    k    l    n    m    i    o    .    /    y    p
)

;; ── 4. 네비게이션 레이어 ────────────────────────────────────
;;    Alt 홀드 상태에서 동작하는 Vim 스타일 키셋
;;
;;    h/j/k/l  = ←↓↑→          n/m = PageUp/PageDown
;;    i        = 단어 앞(1탭) / Home(2탭)
;;    o        = 단어 뒤(1탭) / End(2탭)
;;    .        = Backspace      / = Del(1탭) / 줄 삭제(2탭)
;;    y        = 줄 복사        p = 붙여넣기
;;    Space    = kmd-desktop 실행 (Alt+Space)
(deflayer navigation
  _  _  _  _  @launch-kmd  _  _  _  _  left down up rght pgup pgdn @i-dt @o-dt bspc @sl-dt @y-cp @p-vt
)

;; ── 5. 마우스 레이어 ────────────────────────────────────────
;;    RAlt 홀드 상태 — 홀드 손(오른엄지)과 조작 손(왼손) 분리
;;
;;    w/a/s/d  = 포인터 ↑←↓→ (시간 가속)
;;    Space    = 좌클릭 (홀드 = 드래그)
;;    j/k/l    = 좌/우/중 클릭    LShift = 저속 정밀 모드
;;    나머지 키는 차단 (오타 방지)
(deflayer mouse
  XX _  _  @ms-slow @ms-lclk @ms-u @ms-l @ms-d @ms-r XX mlft mrgt mmid XX XX XX XX XX XX XX XX
)

;; ── 6. 기능 정의 ────────────────────────────────────────────
(defalias
  ;; --- keymander 실행 (Alt+Space) ---
  launch-kmd (cmd kmd-desktop)

  ;; --- 기초 매크로 (편집 기능) ---
  del-line (macro home S-end del)
  y-cp     (macro home S-end C-c)
  p-vt     C-v

  ;; --- 복합 기능 (tap-dance) ---
  ;; 200ms 이내에 연타하면 두 번째 기능이 실행됩니다.
  i-dt  (tap-dance 200 (C-left home))
  o-dt  (tap-dance 200 (C-rght end))
  sl-dt (tap-dance 200 (del @del-line))

  ;; --- Alt 레이어 전환 ---
  ;; Alt 홀드 → navigation 레이어, Alt 짧게 탭 → ESC
  nav-mod (tap-hold 200 200 esc (layer-toggle navigation))

  ;; --- CapsLock 모드탭 (HHKB 스타일) ---
  ;; 짧게 탭 → CapsLock, 홀드 중 다른 키 → Ctrl 조합 (즉시 판정)
  caps-th (tap-hold-press 200 200 caps lctl)

  ;; --- RAlt 마우스 레이어 ---
  ;; 짧게 탭 → RAlt(한영 배열 유지), 홀드 → 마우스 레이어
  mouse-mod (tap-hold-press 200 200 ralt (layer-toggle mouse))

  ;; --- 마우스 이동 (가속: 4ms 간격, 500ms에 걸쳐 1→6px) ---
  ms-u (movemouse-accel-up 4 500 1 6)
  ms-d (movemouse-accel-down 4 500 1 6)
  ms-l (movemouse-accel-left 4 500 1 6)
  ms-r (movemouse-accel-right 4 500 1 6)
  ms-lclk mlft
  ms-slow (movemouse-speed 25)
)
"#;

const MINIMAL_KBD: &str = r#";; ============================================================
;; keymander minimal 프로파일
;; CapsLock → 짧게 탭 = Esc, 홀드 = Ctrl 최소 맵핑
;;
;; 생성: kmd keymap init minimal
;; ============================================================

(defcfg
  process-unmapped-keys yes
)

(defsrc
  caps a s d f j k l ;
)

(deflayer base
  @caps-th  a s d f j k l ;
)

(defalias
  caps-th (tap-hold-press 200 200 esc lctl)
)
"#;

// ── 디렉토리/경로 헬퍼 ───────────────────────────────────────────────────────

fn state_dir() -> PathBuf {
    Config::default_data_dir().join("keymap")
}

fn pid_file() -> PathBuf {
    state_dir().join("kanata.pid")
}

fn default_profile_dir() -> PathBuf {
    Config::default_config_dir().join("keymap")
}

pub fn profile_dir(config: &Config) -> PathBuf {
    config
        .launcher
        .keymap
        .profile_dir
        .clone()
        .unwrap_or_else(default_profile_dir)
}

pub fn active_profile_path(config: &Config) -> PathBuf {
    profile_dir(config).join(&config.launcher.keymap.active_profile)
}

fn kanata_command(config: &Config) -> String {
    config
        .launcher
        .keymap
        .kanata_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "kanata".to_string())
}

// ── PID 관리 ──────────────────────────────────────────────────────────────────

fn ensure_state_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir())
}

fn read_pid() -> Option<u32> {
    let path = pid_file();
    let text = std::fs::read_to_string(path).ok()?;
    text.trim().parse::<u32>().ok()
}

fn write_pid(pid: u32) -> std::io::Result<()> {
    ensure_state_dir()?;
    std::fs::write(pid_file(), pid.to_string())
}

fn clear_pid() {
    let _ = std::fs::remove_file(pid_file());
}

fn is_pid_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("tasklist");
        cmd.args(["/FI", &format!("PID eq {pid}")]);

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        if let Ok(out) = cmd.output() {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            return text.contains(&pid.to_string());
        }
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

// ── 프로세스 상태/제어 ───────────────────────────────────────────────────────

pub fn status() -> String {
    if let Some(pid) = read_pid() {
        if is_pid_running(pid) {
            format!("running (pid={pid})")
        } else {
            "stale pid file (process not running)".to_string()
        }
    } else {
        "stopped".to_string()
    }
}

pub fn is_running() -> bool {
    read_pid().is_some_and(is_pid_running)
}

pub fn start(config: &Config) -> Result<String, String> {
    if !config
        .launcher
        .keymap
        .backend
        .eq_ignore_ascii_case("kanata")
    {
        return Err("지원하지 않는 keymap backend 입니다. (kanata 만 지원)".to_string());
    }
    if let Some(pid) = read_pid() {
        if is_pid_running(pid) {
            return Err(format!("이미 실행 중입니다. pid={pid}"));
        }
    }

    let profile = active_profile_path(config);
    if !profile.exists() {
        return Err(format!(
            "활성 프로파일이 없습니다: {}\n먼저 `kmd keymap init` 또는 `kmd keymap use <profile>` 를 실행하세요.",
            profile.display()
        ));
    }

    let exe = kanata_command(config);
    let mut cmd = Command::new(exe);
    cmd.args(["--cfg", &profile.to_string_lossy()]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    write_pid(pid).map_err(|e| e.to_string())?;
    Ok(format!(
        "kanata 시작됨 (pid={pid}, profile={})",
        profile.to_string_lossy()
    ))
}

pub fn stop() -> Result<String, String> {
    let Some(pid) = read_pid() else {
        return Ok("실행 중인 keymap 프로세스가 없습니다.".to_string());
    };

    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);

        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let status = cmd.status().map_err(|e| e.to_string())?;
        clear_pid();
        if status.success() {
            return Ok(format!("kanata 중지 완료 (pid={pid})"));
        }
        Err(format!("kanata 중지 실패 (pid={pid})"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|e| e.to_string())?;
        clear_pid();
        if status.success() {
            return Ok(format!("kanata 중지 완료 (pid={pid})"));
        }
        Err(format!("kanata 중지 실패 (pid={pid})"))
    }
}

// ── 프로파일 관리 ────────────────────────────────────────────────────────────

pub fn list_profiles(config: &Config) -> Vec<String> {
    let dir = profile_dir(config);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("kbd") {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names
}

/// 프리셋 이름으로 내장 프리셋의 내용을 반환.
pub fn preset_content(preset_name: &str) -> Option<&'static str> {
    let normalized = preset_name.trim_end_matches(".kbd").to_ascii_lowercase();
    BUILTIN_PRESETS
        .iter()
        .find(|p| p.name == normalized)
        .map(|p| p.content)
}

/// 프로파일 생성: 내장 프리셋이 있으면 해당 내용 사용, 없으면 minimal 템플릿 생성.
pub fn create_profile_template(config: &Config, file_name: &str) -> Result<PathBuf, String> {
    let dir = profile_dir(config);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = with_extension(file_name);
    let path = dir.join(&name);
    if path.exists() {
        return Ok(path);
    }

    let content = preset_content(file_name).unwrap_or(MINIMAL_KBD);
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

/// 사용 가능한 내장 프리셋 목록 (이름, 설명).
pub fn list_presets() -> Vec<(&'static str, &'static str)> {
    BUILTIN_PRESETS
        .iter()
        .map(|p| (p.name, p.description))
        .collect()
}

pub fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("프로파일 이름이 비어 있습니다.".to_string());
    }
    if name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
        return Err("프로파일 이름에 사용할 수 없는 문자가 포함되어 있습니다.".to_string());
    }
    Ok(())
}

pub fn with_extension(name: &str) -> String {
    if name.ends_with(".kbd") {
        name.to_string()
    } else {
        format!("{name}.kbd")
    }
}

pub fn exists(path: &Path) -> bool {
    path.exists()
}

/// keymap 상태 요약 정보 (Desktop UI 표시용)
pub fn status_summary(config: &Config) -> (String, String, String) {
    let state = status();
    let backend = config.launcher.keymap.backend.clone();
    let profile = config.launcher.keymap.active_profile.clone();
    (state, backend, profile)
}

// ── IndexItem 생성 (Desktop/TUI UI용) ────────────────────────────────────────

use crate::index::{IndexItem, ItemKind, Source};

/// `:keymap` 명령 결과 아이템 목록 생성
pub fn keymap_items(config: &Config, sub_query: &str, use_emoji: bool) -> Vec<IndexItem> {
    let emoji = use_emoji;
    let sub = sub_query.trim().to_ascii_lowercase();

    if sub == "on" || sub == "start" {
        return vec![action_item(
            "keymap start — kanata 시작",
            &format!("프로파일: {}", config.launcher.keymap.active_profile),
            if emoji { "\u{25B6}\u{FE0F}" } else { "[>]" },
            "kmd:keymap:start",
        )];
    }
    if sub == "off" || sub == "stop" {
        return vec![action_item(
            "keymap stop — kanata 중지",
            &status(),
            if emoji { "\u{23F9}\u{FE0F}" } else { "[S]" },
            "kmd:keymap:stop",
        )];
    }
    if sub.starts_with("use ") {
        let profile_name = sub.strip_prefix("use ").unwrap_or("").trim();
        if profile_name.is_empty() {
            return profile_list_items(config, emoji);
        }
        return vec![action_item(
            &format!("keymap use {profile_name}"),
            "프로파일 전환 (kanata 재시작 필요)",
            if emoji { "\u{1F504}" } else { "[U]" },
            &format!("kmd:keymap:use:{profile_name}"),
        )];
    }
    if sub == "presets" || sub == "list-presets" {
        return preset_list_items(emoji);
    }

    // 기본: 상태 + 프로파일 목록 + 프리셋 안내
    let mut items = vec![status_item(config, emoji)];
    items.extend(profile_list_items(config, emoji));
    items.extend(help_items(emoji));
    items
}

fn status_item(config: &Config, emoji: bool) -> IndexItem {
    let (state, backend, profile) = status_summary(config);
    IndexItem {
        name: format!("Keymap: {state}"),
        path: format!("backend: {backend}, profile: {profile}"),
        kind: ItemKind::SystemCommand,
        source: Source::Plugin,
        icon: if emoji {
            if is_running() {
                "\u{2705}"
            } else {
                "\u{26AA}"
            }
        } else if is_running() {
            "[ON]"
        } else {
            "[--]"
        }
        .to_string(),
        keywords: "kmd:keymap:noop".to_string(),
        icon_path: None,
    }
}

fn profile_list_items(config: &Config, emoji: bool) -> Vec<IndexItem> {
    let profiles = list_profiles(config);
    let active = &config.launcher.keymap.active_profile;
    profiles
        .into_iter()
        .map(|p| {
            let is_active = p == *active;
            IndexItem {
                name: format!("{}  {}", if is_active { "*" } else { " " }, p),
                path: if is_active {
                    "active profile".to_string()
                } else {
                    format!(":keymap use {p}")
                },
                kind: ItemKind::SystemCommand,
                source: Source::Plugin,
                icon: if emoji {
                    if is_active {
                        "\u{1F4CC}"
                    } else {
                        "\u{1F4C4}"
                    }
                } else if is_active {
                    "[*]"
                } else {
                    "[ ]"
                }
                .to_string(),
                keywords: format!("kmd:keymap:use:{p}"),
                icon_path: None,
            }
        })
        .collect()
}

fn preset_list_items(emoji: bool) -> Vec<IndexItem> {
    list_presets()
        .into_iter()
        .map(|(name, desc)| IndexItem {
            name: format!("preset: {name}"),
            path: desc.to_string(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: if emoji { "\u{1F4E6}" } else { "[P]" }.to_string(),
            keywords: format!("kmd:keymap:noop:{name}"),
            icon_path: None,
        })
        .collect()
}

fn help_items(emoji: bool) -> Vec<IndexItem> {
    vec![
        action_item(
            ":keymap on / off — kanata 시작/중지",
            ":keymap use <profile> — 프로파일 전환",
            if emoji { "\u{2139}\u{FE0F}" } else { "[?]" },
            "kmd:keymap:noop",
        ),
        action_item(
            ":keymap presets — 내장 프리셋 목록",
            "kmd keymap init <preset> — 프리셋 설치 (CLI)",
            if emoji { "\u{1F4E6}" } else { "[P]" },
            "kmd:keymap:noop",
        ),
    ]
}

fn action_item(name: &str, path: &str, icon: &str, keywords: &str) -> IndexItem {
    IndexItem {
        name: name.to_string(),
        path: path.to_string(),
        kind: ItemKind::SystemCommand,
        source: Source::Plugin,
        icon: icon.to_string(),
        keywords: keywords.to_string(),
        icon_path: None,
    }
}

/// vim-nav 프리셋의 TOML 기본 바인딩 — **기본 레이어 정의의 단일 소스**.
/// daemon 엔진과 치트시트 모두 [`effective_keymap`]을 거쳐 이 값을 쓴다.
fn vim_nav_default_layer() -> crate::config::LayerToml {
    use crate::config::{LayerDoubleTapToml, LayerToml};
    let mut mappings = std::collections::HashMap::new();
    mappings.insert("H".into(), "Left".into());
    mappings.insert("J".into(), "Down".into());
    mappings.insert("K".into(), "Up".into());
    mappings.insert("L".into(), "Right".into());
    mappings.insert("N".into(), "PageUp".into());
    mappings.insert("M".into(), "PageDown".into());
    mappings.insert(".".into(), "Backspace".into());
    mappings.insert("Space".into(), "launch:kmd-desktop".into());

    #[cfg(target_os = "macos")]
    {
        mappings.insert("P".into(), "Cmd+V".into());
        mappings.insert("Y".into(), "macro:Cmd+Left;Shift+Cmd+Right;Cmd+C".into());
    }
    #[cfg(not(target_os = "macos"))]
    {
        mappings.insert("P".into(), "Ctrl+V".into());
        mappings.insert("Y".into(), "macro:Home;Shift+End;Ctrl+C".into());
    }

    let mut double_taps = std::collections::HashMap::new();

    #[cfg(target_os = "macos")]
    {
        double_taps.insert(
            "I".into(),
            LayerDoubleTapToml {
                single: "Alt+Left".into(),
                double: "Cmd+Left".into(),
                timeout_ms: Some(300),
            },
        );
        double_taps.insert(
            "O".into(),
            LayerDoubleTapToml {
                single: "Alt+Right".into(),
                double: "Cmd+Right".into(),
                timeout_ms: Some(300),
            },
        );
        double_taps.insert(
            "Slash".into(),
            LayerDoubleTapToml {
                single: "Delete".into(),
                double: "macro:Cmd+Left;Shift+Cmd+Right;Delete".into(),
                timeout_ms: Some(300),
            },
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        double_taps.insert(
            "I".into(),
            LayerDoubleTapToml {
                single: "Ctrl+Left".into(),
                double: "Home".into(),
                timeout_ms: Some(300),
            },
        );
        double_taps.insert(
            "O".into(),
            LayerDoubleTapToml {
                single: "Ctrl+Right".into(),
                double: "End".into(),
                timeout_ms: Some(300),
            },
        );
        double_taps.insert(
            "Slash".into(),
            LayerDoubleTapToml {
                single: "Delete".into(),
                double: "macro:Home;Shift+End;Delete".into(),
                timeout_ms: Some(300),
            },
        );
    }

    LayerToml {
        trigger: "LAlt".into(),
        tap_action: Some("Escape".into()),
        tap_hold_ms: Some(200),
        unmapped: None,
        mappings,
        double_taps,
    }
}

/// vim-nav 프리셋의 마우스 레이어 기본값 — RAlt 홀드 + 왼손 조작.
///
/// 설계 원칙: 홀드 손(오른엄지)과 조작 손(왼손)을 분리한다.
/// - WASD = 포인터 이동 (공간 조작은 게이밍 축, 텍스트는 vim 축으로 분리)
/// - Space(왼엄지) = 좌클릭 — 이동하던 손이 자세를 안 바꾸고 확정
/// - J/K/L = 좌/우/중 클릭 (마우스 파지 미러링 별칭)
/// - LShift = 저속 정밀 모드
fn vim_nav_default_mouse_layer() -> crate::config::LayerToml {
    use crate::config::LayerToml;
    let mut mappings = std::collections::HashMap::new();
    mappings.insert("W".into(), "mouse:up".into());
    mappings.insert("A".into(), "mouse:left".into());
    mappings.insert("S".into(), "mouse:down".into());
    mappings.insert("D".into(), "mouse:right".into());
    mappings.insert("Space".into(), "mouse:click".into());
    mappings.insert("J".into(), "mouse:click".into());
    mappings.insert("K".into(), "mouse:rclick".into());
    mappings.insert("L".into(), "mouse:mclick".into());
    mappings.insert("LShift".into(), "mouse:slow".into());

    // Windows 한국어 배열에서 물리 오른쪽 Alt는 한/영 키 — 짧게 탭하면
    // 한/영 전환을 그대로 살린다 (홀드만 마우스 레이어).
    // macOS는 Hangul 키코드가 없어 탭 무동작.
    #[cfg(target_os = "windows")]
    let tap_action = Some("Hangul".into());
    #[cfg(not(target_os = "windows"))]
    let tap_action = None;

    LayerToml {
        trigger: "RAlt".into(),
        tap_action,
        tap_hold_ms: Some(200),
        // 마우스 조작 중 미매핑 키 오타가 텍스트로 입력되는 사고 방지
        unmapped: Some("block".into()),
        mappings,
        double_taps: std::collections::HashMap::new(),
    }
}

/// 프리셋 레이어 기본값 위에 사용자 정의 레이어를 병합.
/// TOML(Option) 수준 병합이라 "생략"은 기본값 유지, "명시"는 덮어쓴다.
fn merge_layer_with_base(
    base: crate::config::LayerToml,
    user: Option<crate::config::LayerToml>,
) -> crate::config::LayerToml {
    let Some(user) = user else { return base };
    let mut effective = base;
    if !user.trigger.is_empty() {
        effective.trigger = user.trigger;
    }
    if user.tap_action.is_some() {
        effective.tap_action = user.tap_action;
    }
    if user.tap_hold_ms.is_some() {
        effective.tap_hold_ms = user.tap_hold_ms;
    }
    if user.unmapped.is_some() {
        effective.unmapped = user.unmapped;
    }
    for (k, v) in user.mappings {
        effective.mappings.insert(k, v);
    }
    for (k, v) in user.double_taps {
        effective.double_taps.insert(k, v);
    }
    effective
}

/// 사용자가 CapsLock에 자체 정의(리맵 또는 tap-hold)를 갖고 있는지
fn user_defined_capslock(km: &crate::config::KeymapConfig) -> bool {
    let is_caps = |k: &str| k.eq_ignore_ascii_case("capslock") || k.eq_ignore_ascii_case("caps");
    km.remaps.keys().any(|k| is_caps(k)) || km.tap_holds.keys().any(|k| is_caps(k))
}

/// active_profile 문자열이 가리키는 프리셋 종류.
///
/// daemon과 치트시트가 각자 다른 규칙(contains vs 완전 일치)으로 판별하던 것을
/// 통일한 단일 판별 함수. "vim-nav.kbd"처럼 확장자가 붙은 값도 vim-nav로 본다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    /// 키맵 비활성 ("none")
    Disabled,
    /// CapsLock → Escape만 ("minimal")
    Minimal,
    /// vim-nav 레이어 프리셋 (기본)
    VimNav,
}

pub fn profile_kind(profile: &str) -> ProfileKind {
    let p = profile.to_ascii_lowercase();
    if p.contains("none") {
        ProfileKind::Disabled
    } else if p.contains("minimal") {
        ProfileKind::Minimal
    } else {
        ProfileKind::VimNav
    }
}

/// active_profile 기반으로 프리셋 기본값을 사용자 config에 병합한 effective keymap.
///
/// **키맵 기본값·병합의 단일 소스.** daemon은 이 결과를
/// `KeybindConfig::from_config`로 변환해 엔진에 넣고, 치트시트는 그대로
/// 표시한다 — 두 경로가 항상 같은 병합 결과를 본다.
///
/// 병합은 TOML(Option) 수준에서 수행하므로 "사용자가 생략한 필드"와
/// "명시적으로 기본값을 적은 필드"가 구분된다 (생략 시 프리셋 값 유지).
pub fn effective_keymap(km: &crate::config::KeymapConfig) -> crate::config::KeymapConfig {
    let mut merged = km.clone();
    match profile_kind(&km.active_profile) {
        ProfileKind::Disabled => {
            // 비활성: 사용자 정의가 있어도 키맵 전체를 끈다 (global_hotkey는
            // daemon이 프로필과 무관하게 별도 등록)
            merged.remaps.clear();
            merged.layers.clear();
            merged.combos.clear();
            merged.double_taps.clear();
            merged.tap_holds.clear();
            return merged;
        }
        ProfileKind::Minimal => {
            if !user_defined_capslock(&merged) {
                // Windows: tap=Esc / hold=Ctrl 모드탭.
                // macOS: CapsLock 합성 주입이 불안정하고 OS가 자체
                // tap(한영)/hold(캡스락)를 제공하므로 기존 단순 리맵 유지.
                #[cfg(target_os = "windows")]
                merged.tap_holds.insert(
                    "CapsLock".into(),
                    crate::config::TapHoldToml {
                        tap: Some("Escape".into()),
                        hold: "LCtrl".into(),
                        timeout_ms: Some(200),
                    },
                );
                #[cfg(not(target_os = "windows"))]
                merged.remaps.insert("CapsLock".into(), "Escape".into());
            }
        }
        ProfileKind::VimNav => {
            let user_nav = merged.layers.get("nav").cloned();
            merged.layers.insert(
                "nav".into(),
                merge_layer_with_base(vim_nav_default_layer(), user_nav),
            );

            let user_mouse = merged.layers.get("mouse").cloned();
            merged.layers.insert(
                "mouse".into(),
                merge_layer_with_base(vim_nav_default_mouse_layer(), user_mouse),
            );

            // HHKB 스타일 CapsLock: 짧게 탭 = CapsLock, 홀드 = Ctrl.
            // macOS는 OS 기본(한영/캡스락 tap-hold)과 충돌하므로 제외.
            #[cfg(target_os = "windows")]
            if !user_defined_capslock(&merged) {
                merged.tap_holds.insert(
                    "CapsLock".into(),
                    crate::config::TapHoldToml {
                        tap: Some("CapsLock".into()),
                        hold: "LCtrl".into(),
                        timeout_ms: Some(200),
                    },
                );
            }
        }
    }

    // 한/영 전환 기본 제스처 (minimal/vim-nav 공통):
    // Shift+Space는 전 플랫폼, RShift 더블탭은 Windows 전용
    let has_shift_space = merged
        .combos
        .iter()
        .any(|c| c.trigger.eq_ignore_ascii_case("shift+space"));
    if !has_shift_space {
        merged.combos.push(crate::config::ComboToml {
            trigger: "Shift+Space".into(),
            action: "Hangul".into(),
        });
    }

    #[cfg(target_os = "windows")]
    {
        let has_rshift_dt = merged
            .double_taps
            .iter()
            .any(|dt| dt.key.eq_ignore_ascii_case("rshift"));
        if !has_rshift_dt {
            merged.double_taps.push(crate::config::DoubleTapToml {
                key: "RShift".into(),
                action: "Hangul".into(),
                timeout_ms: Some(300),
            });
        }
    }

    merged
}

/// 키맵핑 치트시트 생성 — 프리셋+사용자 config 병합 결과를 시각화
/// 치트시트를 표시하는 프런트엔드 — 내장 단축키 섹션이 달라진다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheatsheetApp {
    Desktop,
    Tui,
}

pub fn keybinding_cheatsheet(
    config: &Config,
    use_emoji: bool,
    app: CheatsheetApp,
) -> Vec<IndexItem> {
    let mut items: Vec<IndexItem> = Vec::new();

    let section = |items: &mut Vec<IndexItem>, title: &str, icon: &str| {
        items.push(IndexItem {
            name: format!("━━  {title}  ━━"),
            path: String::new(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: icon.to_string(),
            keywords: "kmd:keys:noop".to_string(),
            icon_path: None,
        });
    };
    let entry = |items: &mut Vec<IndexItem>, shortcut: &str, desc: &str, icon: &str| {
        items.push(IndexItem {
            name: shortcut.to_string(),
            path: desc.to_string(),
            kind: ItemKind::SystemCommand,
            source: Source::Plugin,
            icon: icon.to_string(),
            keywords: "kmd:keys:noop".to_string(),
            icon_path: None,
        });
    };

    let e = use_emoji;

    // ── 프런트엔드 내장 단축키 ──
    match app {
        CheatsheetApp::Desktop => {
            section(
                &mut items,
                "KMD Desktop",
                if e { "\u{2328}\u{FE0F}" } else { "[KMD]" },
            );
            entry(
                &mut items,
                "Esc",
                "프로그램 종료",
                if e { "\u{274C}" } else { "[X]" },
            );
            entry(
                &mut items,
                "\u{2191} / \u{2193}",
                "결과 목록 이동",
                if e { "\u{1F503}" } else { "[^v]" },
            );
            entry(
                &mut items,
                "Enter",
                "선택 항목 실행",
                if e { "\u{23CE}" } else { "[>]" },
            );
            entry(
                &mut items,
                "Tab",
                "상세 패널 열기",
                if e { "\u{21E5}" } else { "[T]" },
            );
            entry(
                &mut items,
                "Ctrl+1~4",
                "컨텍스트 액션 실행",
                if e { "\u{1F522}" } else { "[#]" },
            );
            entry(
                &mut items,
                "Ctrl+Shift+Enter",
                "관리자 권한 실행",
                if e { "\u{1F512}" } else { "[!]" },
            );
        }
        CheatsheetApp::Tui => {
            section(
                &mut items,
                "KMD TUI",
                if e { "\u{2328}\u{FE0F}" } else { "[KMD]" },
            );
            entry(
                &mut items,
                "Esc",
                "드릴 뒤로 → 쿼리 지우기 → 종료",
                if e { "\u{274C}" } else { "[X]" },
            );
            entry(
                &mut items,
                "\u{2191} / \u{2193}",
                "결과 목록 이동",
                if e { "\u{1F503}" } else { "[^v]" },
            );
            entry(
                &mut items,
                "Enter",
                "선택 항목 실행",
                if e { "\u{23CE}" } else { "[>]" },
            );
            entry(
                &mut items,
                "Tab / \u{2192}",
                "폴더 드릴다운",
                if e { "\u{21E5}" } else { "[T]" },
            );
            entry(
                &mut items,
                "\u{2190}",
                "드릴 뒤로",
                if e { "\u{2B05}\u{FE0F}" } else { "[<]" },
            );
            entry(
                &mut items,
                "Ctrl+P",
                "미리보기 패널 토글",
                if e { "\u{1F441}\u{FE0F}" } else { "[P]" },
            );
            entry(
                &mut items,
                "F2",
                "설정 모달 열기",
                if e { "\u{2699}\u{FE0F}" } else { "[S]" },
            );
            entry(
                &mut items,
                "Ctrl+C",
                "종료",
                if e { "\u{1F6D1}" } else { "[Q]" },
            );
        }
    }

    // ── 명령어 ──
    section(
        &mut items,
        "Commands",
        if e { "\u{1F4DD}" } else { "[CMD]" },
    );
    entry(
        &mut items,
        ":help / :h",
        "도움말 표시",
        if e { "\u{2753}" } else { "[?]" },
    );
    entry(
        &mut items,
        ":set / :settings",
        "설정 관리",
        if e { "\u{2699}\u{FE0F}" } else { "[S]" },
    );
    entry(
        &mut items,
        ":keys / :k",
        "키 맵핑 시트 (현재 화면)",
        if e { "\u{1F5FA}\u{FE0F}" } else { "[K]" },
    );
    entry(
        &mut items,
        ":keymap / :km",
        "키맵 프로파일 관리",
        if e { "\u{1F3AE}" } else { "[KM]" },
    );
    entry(
        &mut items,
        ":calc",
        "계산기",
        if e { "\u{1F4F1}" } else { "[=]" },
    );
    entry(
        &mut items,
        ":emoji / :e",
        "이모지 검색",
        if e { "\u{1F60A}" } else { "[E]" },
    );
    entry(
        &mut items,
        "@ (prefix)",
        "웹 서비스 검색 (@g, @ai, ...)",
        if e { "\u{1F310}" } else { "[@]" },
    );
    entry(
        &mut items,
        "! (prefix)",
        "셸 명령 실행",
        if e { "\u{1F4BB}" } else { "[!]" },
    );

    // ── daemon keybindings: remaps ──
    let km = &effective_keymap(&config.launcher.keymap);
    if !km.remaps.is_empty() {
        section(&mut items, "Remaps", if e { "\u{1F504}" } else { "[R]" });
        let mut remaps: Vec<_> = km.remaps.iter().collect();
        remaps.sort_by_key(|(k, _)| k.to_lowercase());
        for (from, to) in remaps {
            entry(
                &mut items,
                &format!("{from}  \u{2192}  {to}"),
                "항상 활성",
                if e { "\u{1F517}" } else { "[>]" },
            );
        }
    }

    // ── daemon keybindings: tap-holds (모드탭) ──
    if !km.tap_holds.is_empty() {
        section(&mut items, "Tap-Hold", if e { "\u{1F919}" } else { "[TH]" });
        let mut tap_holds: Vec<_> = km.tap_holds.iter().collect();
        tap_holds.sort_by_key(|(k, _)| k.to_lowercase());
        for (key, th) in tap_holds {
            let tap_desc = th.tap.as_deref().unwrap_or("-");
            entry(
                &mut items,
                key,
                &format!("\u{1F44B} tap: {tap_desc} / \u{270A} hold: {}", th.hold),
                if e { "\u{1F918}" } else { "[>]" },
            );
        }
    }

    // ── daemon keybindings: layers ──
    let mut layer_names: Vec<_> = km.layers.keys().collect();
    layer_names.sort();
    for layer_name in layer_names {
        let layer = &km.layers[layer_name];
        let trigger = &layer.trigger;
        section(
            &mut items,
            &format!("Layer: {layer_name} ({trigger} hold)"),
            if e { "\u{1F4E6}" } else { "[L]" },
        );
        if let Some(ref tap) = layer.tap_action {
            entry(
                &mut items,
                &format!("{trigger} (tap)"),
                &format!("\u{2192} {tap}"),
                if e { "\u{1F44B}" } else { "[T]" },
            );
        }

        let mut mappings: Vec<_> = layer.mappings.iter().collect();
        mappings.sort_by_key(|(k, _)| k.to_lowercase());
        for (key, action) in &mappings {
            entry(
                &mut items,
                &format!("{trigger}+{key}"),
                &format!("\u{2192} {action}"),
                if e { "\u{27A1}\u{FE0F}" } else { "[>]" },
            );
        }

        let mut dt_keys: Vec<_> = layer.double_taps.keys().collect();
        dt_keys.sort_by_key(|k| k.to_lowercase());
        for key in dt_keys {
            let dt = &layer.double_taps[key];
            entry(
                &mut items,
                &format!("{trigger}+{key}"),
                &format!("1\u{00D7}: {} / 2\u{00D7}: {}", dt.single, dt.double),
                if e { "\u{1F4A8}" } else { "[D]" },
            );
        }
    }

    // ── daemon keybindings: combos ──
    if !km.combos.is_empty() {
        section(&mut items, "Combos", if e { "\u{1F3B9}" } else { "[C]" });
        for combo in &km.combos {
            entry(
                &mut items,
                &combo.trigger,
                &format!("\u{2192} {}", combo.action),
                if e { "\u{26A1}" } else { "[>]" },
            );
        }
    }

    // ── daemon keybindings: double-taps ──
    if !km.double_taps.is_empty() {
        section(
            &mut items,
            "Double-Tap",
            if e { "\u{1F91C}" } else { "[DT]" },
        );
        for dt in &km.double_taps {
            entry(
                &mut items,
                &format!("{} \u{00D7}2", dt.key),
                &format!("\u{2192} {}", dt.action),
                if e { "\u{26A1}" } else { "[>]" },
            );
        }
    }

    items
}

/// keymap 아이템 선택 시 실행할 액션 처리 (Desktop/TUI 공용)
pub fn execute_keymap_action(config: &mut Config, keywords: &str) -> Option<String> {
    if keywords == "kmd:keymap:start" {
        return Some(match start(config) {
            Ok(msg) => msg,
            Err(e) => e,
        });
    }
    if keywords == "kmd:keymap:stop" {
        return Some(match stop() {
            Ok(msg) => msg,
            Err(e) => e,
        });
    }
    if let Some(profile) = keywords.strip_prefix("kmd:keymap:use:") {
        let profile_name = with_extension(profile);
        let path = profile_dir(config).join(&profile_name);
        if !path.exists() {
            return Some(format!("프로파일 파일이 없습니다: {}", path.display()));
        }
        config.launcher.keymap.active_profile = profile_name.clone();
        let _ = config.save();
        return Some(format!("active profile 변경: {profile_name}"));
    }
    None
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ComboToml, KeymapConfig, LayerToml};

    #[test]
    fn profile_kind_판별() {
        assert_eq!(profile_kind("vim-nav"), ProfileKind::VimNav);
        assert_eq!(profile_kind("vim-nav.kbd"), ProfileKind::VimNav);
        assert_eq!(profile_kind("custom"), ProfileKind::VimNav);
        assert_eq!(profile_kind("minimal"), ProfileKind::Minimal);
        assert_eq!(profile_kind("minimal.kbd"), ProfileKind::Minimal);
        assert_eq!(profile_kind("none"), ProfileKind::Disabled);
        assert_eq!(profile_kind("NONE"), ProfileKind::Disabled);
    }

    fn keymap_with_profile(profile: &str) -> KeymapConfig {
        KeymapConfig {
            active_profile: profile.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn vim_nav_기본_레이어와_한영_폴백() {
        let e = effective_keymap(&keymap_with_profile("vim-nav"));
        let nav = e.layers.get("nav").expect("nav 레이어");
        assert_eq!(nav.trigger, "LAlt");
        assert_eq!(nav.tap_action.as_deref(), Some("Escape"));
        assert!(nav.mappings.contains_key("H"));
        assert!(nav.double_taps.contains_key("I"));
        assert!(e
            .combos
            .iter()
            .any(|c| c.trigger.eq_ignore_ascii_case("shift+space")));
        #[cfg(target_os = "windows")]
        assert!(e
            .double_taps
            .iter()
            .any(|dt| dt.key.eq_ignore_ascii_case("rshift")));
    }

    #[test]
    fn 사용자_매핑은_기본을_보존하며_덮어쓴다() {
        let mut km = keymap_with_profile("vim-nav");
        let mut layer = LayerToml {
            trigger: "LAlt".into(),
            ..Default::default()
        };
        layer.mappings.insert("H".into(), "Right".into());
        km.layers.insert("nav".into(), layer);

        let e = effective_keymap(&km);
        let nav = &e.layers["nav"];
        assert_eq!(nav.mappings["H"], "Right", "H는 사용자 값으로 덮어씀");
        assert!(nav.mappings.contains_key("."), "미지정 키는 기본 보존");
        assert!(nav.mappings.contains_key("Space"), "미지정 키는 기본 보존");
        assert!(nav.double_taps.contains_key("I"), "더블탭 기본 보존");
    }

    /// 회귀 방지: 과거 daemon 병합은 파싱 시점에 Option이 소실되어
    /// "생략"과 "명시적 기본값"을 구분하지 못했다 (unmapped/tap_hold_ms가
    /// 프리셋 기본값과 다를 때 조용히 되돌아가는 잠복 버그).
    #[test]
    fn 생략된_필드는_기본값을_유지한다() {
        let mut km = keymap_with_profile("vim-nav");
        let layer = LayerToml {
            trigger: "LAlt".into(),
            ..Default::default()
        };
        // tap_action / tap_hold_ms / unmapped 전부 생략
        km.layers.insert("nav".into(), layer);

        let e = effective_keymap(&km);
        let nav = &e.layers["nav"];
        assert_eq!(nav.tap_action.as_deref(), Some("Escape"), "기본 유지");
        assert_eq!(nav.tap_hold_ms, Some(200), "기본 유지");

        // 명시하면 반영
        let mut km2 = keymap_with_profile("vim-nav");
        let layer2 = LayerToml {
            trigger: "LAlt".into(),
            unmapped: Some("passthrough".into()),
            tap_hold_ms: Some(300),
            ..Default::default()
        };
        km2.layers.insert("nav".into(), layer2);

        let e2 = effective_keymap(&km2);
        let nav2 = &e2.layers["nav"];
        assert_eq!(nav2.unmapped.as_deref(), Some("passthrough"));
        assert_eq!(nav2.tap_hold_ms, Some(300));
    }

    #[test]
    fn 사용자_한영_콤보가_있으면_중복_추가하지_않는다() {
        let mut km = keymap_with_profile("vim-nav");
        km.combos.push(ComboToml {
            trigger: "shift+space".into(),
            action: "Hanja".into(),
        });
        let e = effective_keymap(&km);
        let shift_space: Vec<_> = e
            .combos
            .iter()
            .filter(|c| c.trigger.eq_ignore_ascii_case("shift+space"))
            .collect();
        assert_eq!(shift_space.len(), 1);
        assert_eq!(shift_space[0].action, "Hanja", "사용자 액션 유지");
    }

    #[test]
    fn minimal은_capslock_기본만() {
        let e = effective_keymap(&keymap_with_profile("minimal"));
        #[cfg(target_os = "windows")]
        {
            let th = e.tap_holds.get("CapsLock").expect("CapsLock tap-hold");
            assert_eq!(th.tap.as_deref(), Some("Escape"));
            assert_eq!(th.hold, "LCtrl");
            assert!(e.remaps.is_empty());
        }
        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(e.remaps.get("CapsLock").map(String::as_str), Some("Escape"));
            assert!(e.tap_holds.is_empty());
        }
        assert!(e.layers.is_empty());

        // 사용자가 CapsLock을 재정의하면 존중 (기본 tap-hold/리맵 미주입)
        let mut km = keymap_with_profile("minimal");
        km.remaps.insert("capslock".into(), "Tab".into());
        let e2 = effective_keymap(&km);
        assert_eq!(e2.remaps.len(), 1);
        assert_eq!(e2.remaps["capslock"], "Tab");
        assert!(e2.tap_holds.is_empty());
    }

    #[test]
    fn vim_nav_마우스_레이어_기본값() {
        let e = effective_keymap(&keymap_with_profile("vim-nav"));
        let mouse = e.layers.get("mouse").expect("mouse 레이어");
        assert_eq!(mouse.trigger, "RAlt");
        assert_eq!(mouse.unmapped.as_deref(), Some("block"));
        assert_eq!(mouse.mappings["W"], "mouse:up");
        assert_eq!(mouse.mappings["Space"], "mouse:click");
        assert_eq!(mouse.mappings["LShift"], "mouse:slow");

        // 사용자 정의는 기본 위에 병합
        let mut km = keymap_with_profile("vim-nav");
        let mut layer = LayerToml::default();
        layer.mappings.insert("W".into(), "mouse:wheel-up".into());
        km.layers.insert("mouse".into(), layer);
        let e2 = effective_keymap(&km);
        let mouse2 = &e2.layers["mouse"];
        assert_eq!(mouse2.trigger, "RAlt", "trigger 생략 시 기본 유지");
        assert_eq!(mouse2.mappings["W"], "mouse:wheel-up");
        assert_eq!(mouse2.mappings["A"], "mouse:left", "미지정 키 기본 보존");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn vim_nav_capslock_tap_hold_기본값() {
        let e = effective_keymap(&keymap_with_profile("vim-nav"));
        let th = e.tap_holds.get("CapsLock").expect("CapsLock tap-hold");
        assert_eq!(th.tap.as_deref(), Some("CapsLock"));
        assert_eq!(th.hold, "LCtrl");

        // 사용자 자체 tap-hold 정의 존중
        let mut km = keymap_with_profile("vim-nav");
        km.tap_holds.insert(
            "caps".into(),
            crate::config::TapHoldToml {
                tap: Some("Escape".into()),
                hold: "LWin".into(),
                timeout_ms: None,
            },
        );
        let e2 = effective_keymap(&km);
        assert_eq!(e2.tap_holds.len(), 1);
        assert_eq!(e2.tap_holds["caps"].hold, "LWin");
    }

    #[test]
    fn none은_사용자_정의까지_전부_비활성() {
        let mut km = keymap_with_profile("none");
        let layer = LayerToml {
            trigger: "LAlt".into(),
            ..Default::default()
        };
        km.layers.insert("nav".into(), layer);
        km.remaps.insert("CapsLock".into(), "Escape".into());
        km.tap_holds.insert(
            "caps".into(),
            crate::config::TapHoldToml {
                tap: None,
                hold: "LCtrl".into(),
                timeout_ms: None,
            },
        );

        let e = effective_keymap(&km);
        assert!(e.layers.is_empty());
        assert!(e.remaps.is_empty());
        assert!(e.combos.is_empty());
        assert!(e.double_taps.is_empty());
        assert!(e.tap_holds.is_empty());
    }

    #[test]
    fn 새_레이어_추가() {
        let mut km = keymap_with_profile("vim-nav");
        let mut layer = LayerToml {
            trigger: "RAlt".into(),
            ..Default::default()
        };
        layer.mappings.insert("A".into(), "B".into());
        km.layers.insert("sym".into(), layer);

        let e = effective_keymap(&km);
        assert_eq!(e.layers.len(), 3, "nav+mouse(프리셋) + sym(사용자)");
        assert!(e.layers.contains_key("sym"));
    }
}
