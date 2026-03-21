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
        description: "Alt 홀드 → Vim 스타일 네비게이션 + Alt+Space → kmd-desktop 실행",
        content: VIM_NAV_KBD,
    },
    BuiltinPreset {
        name: "minimal",
        description: "CapsLock → Esc 최소 맵핑",
        content: MINIMAL_KBD,
    },
];

const VIM_NAV_KBD: &str = r#";; ============================================================
;; keymander vim-nav 프로파일
;; Alt 홀드 → Vim 스타일 네비게이션 + Alt+Space → kmd-desktop 실행
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
  alt  spc  h    j    k    l    n    m    i    o    .    /    y    p
)

;; ── 3. 기본 레이어 ──────────────────────────────────────────
(deflayer default
  @nav-mod  spc  h    j    k    l    n    m    i    o    .    /    y    p
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
  _  @launch-kmd  left down up rght pgup pgdn @i-dt @o-dt bspc @sl-dt @y-cp @p-vt
)

;; ── 5. 기능 정의 ────────────────────────────────────────────
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
)
"#;

const MINIMAL_KBD: &str = r#";; ============================================================
;; keymander minimal 프로파일
;; CapsLock → Esc 최소 맵핑
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
  esc  a s d f j k l ;
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

/// 키맵핑 치트시트 생성 — config 기반 전체 바인딩을 시각화
pub fn keybinding_cheatsheet(config: &Config, use_emoji: bool) -> Vec<IndexItem> {
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

    // ── kmd-desktop 내장 단축키 ──
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
    let km = &config.launcher.keymap;
    if !km.remaps.is_empty() {
        section(
            &mut items,
            "Remaps",
            if e { "\u{1F504}" } else { "[R]" },
        );
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
        section(
            &mut items,
            "Combos",
            if e { "\u{1F3B9}" } else { "[C]" },
        );
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
