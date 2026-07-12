//! TOML-based configuration management
//!
//! Launcher-focused config with keybindings and provider settings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Top-level configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub launcher: LauncherConfig,
    pub keybindings: KeybindingsConfig,

    /// Config file path (excluded from serialization)
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// TUI render FPS
    pub render_fps: u64,
    /// Show preview panel
    pub show_preview: bool,
    /// Preview panel width percentage
    pub preview_width_percent: u16,
    /// Color theme name
    pub theme: String,
    /// External editor command
    pub editor: Option<String>,
    /// Use emoji icons (true = emoji, false = ASCII 2-char icons)
    /// Set to false for legacy terminals (conhost/cmd.exe)
    pub emoji_icons: bool,
    /// Reset input method to English when the desktop launcher opens.
    /// Useful because most commands start with English characters (@, :, etc.).
    pub reset_ime_on_launch: bool,
    /// Desktop launcher base font size in pixels.
    /// All UI elements scale proportionally. Range: 12–32, default: 16.
    pub font_size: f32,
    /// Desktop launcher visible result rows. Range: 4–20, default: 8.
    pub visible_rows: usize,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            render_fps: 30,
            show_preview: true,
            preview_width_percent: 40,
            theme: "default".to_string(),
            editor: None,
            emoji_icons: true,
            reset_ime_on_launch: true,
            font_size: 16.0,
            visible_rows: 8,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LauncherConfig {
    /// File search provider: auto | builtin | fd | everything | mdfind | locate | winfs
    pub file_search_provider: String,
    /// Everything CLI (es.exe) path (Windows). Empty = auto-detect.
    pub everything_path: Option<PathBuf>,
    /// Directories to search for files
    pub search_paths: Vec<PathBuf>,
    /// Maximum search results from file providers
    pub max_results: usize,
    /// Maximum recursive directory depth for file scanning (default: 4)
    pub search_depth: usize,
    /// Patterns to ignore during file indexing
    pub ignore_patterns: Vec<String>,
    /// Quit kmd after launching a program/file
    pub quit_on_launch: bool,
    /// Whether to index directories (not just files)
    pub index_directories: bool,
    /// Auto-scan available drive roots (Windows: C:\~Z:\, macOS/Linux: /)
    pub scan_drives: bool,
    /// Max depth when scanning drive roots (shallow to avoid system dirs)
    pub drive_scan_depth: usize,
    /// Search result priority weights by item kind (0-100, higher = boosted)
    pub kind_weights: KindWeights,
    /// Custom web services
    #[serde(default)]
    pub web_services: Vec<CustomWebService>,
    /// LLM IDs to open for `@llm` multi-prompt compare.
    /// Supported: chatgpt, gemini, claude, grok, perplexity
    #[serde(default = "default_multi_llm_providers")]
    pub multi_llm_providers: Vec<String>,
    /// Command aliases for multi-LLM compare.
    /// Example: @llm, @ll, @cmp
    #[serde(default = "default_multi_llm_prefixes")]
    pub multi_llm_prefixes: Vec<String>,
    /// Search engine IDs for `@msearch` multi-web search.
    /// Supported: google, naver_search, daum
    #[serde(default = "default_multi_web_providers")]
    pub multi_web_providers: Vec<String>,
    /// Command aliases for multi-web search.
    /// Example: @m, @mw, @msearch
    #[serde(default = "default_multi_web_prefixes")]
    pub multi_web_prefixes: Vec<String>,
    /// Providers used by spelling check command.
    /// Supported: naver_spell, pusan_spell
    #[serde(default = "default_spell_providers")]
    pub spell_providers: Vec<String>,
    /// Command aliases for spelling check.
    /// Example: @sp, @spell
    #[serde(default = "default_spell_prefixes")]
    pub spell_prefixes: Vec<String>,
    /// Providers used by translate command.
    /// Supported: google_translate, papago, deepl
    #[serde(default = "default_translate_providers")]
    pub translate_providers: Vec<String>,
    /// Command aliases for translate.
    /// Example: @tr, @trko, @tren
    #[serde(default = "default_translate_prefixes")]
    pub translate_prefixes: Vec<String>,
    /// Keymap integration settings (prototype).
    #[serde(default)]
    pub keymap: KeymapConfig,
    /// 저장된 프롬프트 템플릿 목록 (`@ll :name query` 형태로 사용).
    #[serde(default)]
    pub prompt_templates: Vec<PromptTemplate>,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            file_search_provider: "auto".to_string(),
            everything_path: None,
            search_paths: default_search_paths(),
            // Conservative defaults to reduce cold-start indexing cost.
            max_results: 5000,
            search_depth: 4,
            ignore_patterns: vec![
                // Version control
                ".git".to_string(),
                ".svn".to_string(),
                // Build artifacts / package caches
                "node_modules".to_string(),
                "target".to_string(),
                "__pycache__".to_string(),
                ".tox".to_string(),
                "dist".to_string(),
                // Rust / Cargo
                ".cargo".to_string(),
                ".rustup".to_string(),
                // Node / npm / yarn / pnpm
                ".npm".to_string(),
                ".yarn".to_string(),
                ".pnpm-store".to_string(),
                // .NET / NuGet
                ".nuget".to_string(),
                // Java / Gradle / Maven
                ".gradle".to_string(),
                ".m2".to_string(),
                // Go
                "go".to_string(), // ~/go module cache
                // General caches
                ".cache".to_string(),
                ".local".to_string(),
                ".tmp".to_string(),
                // IDE / editor state
                ".vscode".to_string(),
                ".cursor".to_string(),
                ".idea".to_string(),
                ".eclipse".to_string(),
                // Windows specific — user profile
                "AppData".to_string(),
                "$Recycle.Bin".to_string(),
                "NTUSER.DAT".to_string(),
                // Windows specific — system directories (C:\ root)
                "Windows".to_string(),
                "Program Files".to_string(),
                "Program Files (x86)".to_string(),
                "ProgramData".to_string(),
                "PerfLogs".to_string(),
                "Recovery".to_string(),
                "System Volume Information".to_string(),
                "inetpub".to_string(),
                // macOS specific
                "Library".to_string(),
                ".Trash".to_string(),
            ],
            quit_on_launch: true,
            index_directories: true,
            scan_drives: false,
            drive_scan_depth: 2,
            kind_weights: KindWeights::default(),
            web_services: vec![],
            multi_llm_providers: default_multi_llm_providers(),
            multi_llm_prefixes: default_multi_llm_prefixes(),
            multi_web_providers: default_multi_web_providers(),
            multi_web_prefixes: default_multi_web_prefixes(),
            spell_providers: default_spell_providers(),
            spell_prefixes: default_spell_prefixes(),
            translate_providers: default_translate_providers(),
            translate_prefixes: default_translate_prefixes(),
            keymap: KeymapConfig::default(),
            prompt_templates: vec![],
        }
    }
}

fn default_multi_llm_providers() -> Vec<String> {
    vec![
        "chatgpt".to_string(),
        "gemini".to_string(),
        "claude".to_string(),
        "grok".to_string(),
        "perplexity".to_string(),
    ]
}

fn default_multi_llm_prefixes() -> Vec<String> {
    vec![
        "@llm".to_string(),
        "@ll".to_string(),
        "@multi".to_string(),
        "@cmp".to_string(),
        "@compare".to_string(),
    ]
}

fn default_multi_web_providers() -> Vec<String> {
    vec![
        "google".to_string(),
        "naver_search".to_string(),
        "daum".to_string(),
    ]
}

fn default_multi_web_prefixes() -> Vec<String> {
    vec![
        "@m".to_string(),
        "@mw".to_string(),
        "@msearch".to_string(),
        "@multisearch".to_string(),
        "@searchall".to_string(),
        "@krsearch".to_string(),
    ]
}

fn default_spell_providers() -> Vec<String> {
    vec!["naver_spell".to_string(), "pusan_spell".to_string()]
}

fn default_spell_prefixes() -> Vec<String> {
    vec!["@sp".to_string(), "@spell".to_string()]
}

fn default_translate_providers() -> Vec<String> {
    vec![
        "google_translate".to_string(),
        "papago".to_string(),
        "deepl".to_string(),
    ]
}

fn default_translate_prefixes() -> Vec<String> {
    vec!["@tr".to_string(), "@trko".to_string(), "@tren".to_string()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KeymapConfig {
    /// Backend type. Current prototype supports `kanata`.
    pub backend: String,
    /// Path to kanata executable. Empty = PATH lookup.
    pub kanata_path: Option<PathBuf>,
    /// Directory containing keymap profile files.
    pub profile_dir: Option<PathBuf>,
    /// Active profile: "vim-nav" | "minimal" | "none" | "custom"
    /// "custom"이면 아래 remaps/layers/combos/double_taps 사용
    pub active_profile: String,

    /// 단순 리맵 (항상 활성). 키이름 = "대상키" 또는 "Ctrl+키"
    #[serde(default)]
    pub remaps: HashMap<String, String>,
    /// 레이어 정의. 이름 = { trigger, mappings, ... }
    #[serde(default)]
    pub layers: HashMap<String, LayerToml>,
    /// 수정자+키 콤보 (예: Shift+Space → Hangul)
    #[serde(default)]
    pub combos: Vec<ComboToml>,
    /// 글로벌 더블탭
    #[serde(default)]
    pub double_taps: Vec<DoubleTapToml>,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self {
            backend: "kanata".to_string(),
            kanata_path: None,
            profile_dir: None,
            active_profile: "vim-nav".to_string(),
            remaps: HashMap::new(),
            layers: HashMap::new(),
            combos: Vec::new(),
            double_taps: Vec::new(),
        }
    }
}

/// TOML 레이어 설정
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct LayerToml {
    /// 레이어 활성화 트리거 키 (예: "LAlt")
    pub trigger: String,
    /// 짧게 탭했을 때 보낼 키 (예: "Escape")
    pub tap_action: Option<String>,
    /// tap-hold 판정 시간 (ms, 기본 200)
    pub tap_hold_ms: Option<u32>,
    /// 미매핑 키 동작: "plain"(기본, 맨키 통과) | "passthrough"(트리거 조합
    /// 그대로 OS로, 예: Alt+Tab 유지) | "block"(억제)
    pub unmapped: Option<String>,
    /// 레이어 내 키 매핑: 키이름 = "대상" (예: H = "Left", P = "Ctrl+V")
    pub mappings: HashMap<String, String>,
    /// 레이어 내 더블탭: 키이름 = { single, double, timeout_ms }
    pub double_taps: HashMap<String, LayerDoubleTapToml>,
}

/// TOML 레이어 내 더블탭 설정
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LayerDoubleTapToml {
    /// 한 번 탭 시 액션 (예: "Ctrl+Left")
    pub single: String,
    /// 더블탭 시 액션 (예: "Home")
    pub double: String,
    /// 타임아웃 (ms, 기본 300)
    pub timeout_ms: Option<u32>,
}

/// TOML 콤보 설정
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComboToml {
    /// 트리거 (예: "Shift+Space")
    pub trigger: String,
    /// 실행 액션 (예: "Hangul")
    pub action: String,
}

/// TOML 글로벌 더블탭 설정
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DoubleTapToml {
    /// 키 (예: "RShift")
    pub key: String,
    /// 액션 (예: "Hangul")
    pub action: String,
    /// 타임아웃 (ms, 기본 300)
    pub timeout_ms: Option<u32>,
}

/// 재사용 가능한 프롬프트 템플릿. `@ll :name question` 형태로 LLM에 전달.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PromptTemplate {
    /// 고유 이름 (영문/숫자/하이픈, 예: code-review)
    pub name: String,
    /// 본문 — `{query}` 자리표시자가 있으면 실제 질문으로 치환.
    /// 없으면 `body + "\n\n" + query` 형태로 결합.
    pub body: String,
}

/// Platform-specific default search paths (user directories).
/// These become the default for `launcher.search_paths`, editable in settings.
fn default_search_paths() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let base = PathBuf::from(&profile);
            for name in &["Desktop", "Documents", "Downloads", "OneDrive"] {
                let dir = base.join(name);
                if dir.is_dir() {
                    dirs.push(dir);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            for name in &["Desktop", "Documents", "Downloads"] {
                let dir = home.join(name);
                if dir.is_dir() {
                    dirs.push(dir);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            for name in &["Desktop", "Documents", "Downloads"] {
                let dir = home.join(name);
                if dir.is_dir() {
                    dirs.push(dir);
                }
            }
        }
        for env_key in &["XDG_DESKTOP_DIR", "XDG_DOCUMENTS_DIR", "XDG_DOWNLOAD_DIR"] {
            if let Ok(val) = std::env::var(env_key) {
                let dir = PathBuf::from(val);
                if dir.is_dir() && !dirs.contains(&dir) {
                    dirs.push(dir);
                }
            }
        }
    }

    dirs
}

/// Search result priority weights per item kind.
/// Higher values push results toward the top (0-100 range recommended).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KindWeights {
    pub directory: u32,
    pub app: u32,
    pub file: u32,
    pub executable: u32,
    pub system_cmd: u32,
    pub web_search: u32,
}

impl Default for KindWeights {
    fn default() -> Self {
        Self {
            directory: 80,
            app: 70,
            file: 50,
            executable: 40,
            system_cmd: 30,
            web_search: 20,
        }
    }
}

impl KindWeights {
    /// Get the weight for a given ItemKind
    pub fn weight_for(&self, kind: crate::index::ItemKind) -> u32 {
        use crate::index::ItemKind;
        match kind {
            ItemKind::Directory => self.directory,
            ItemKind::App => self.app,
            ItemKind::File => self.file,
            ItemKind::Executable => self.executable,
            ItemKind::SystemCommand => self.system_cmd,
            ItemKind::WebSearch => self.web_search,
            ItemKind::Calculator => 0, // handled separately
            ItemKind::Emoji => 0,      // handled separately
            ItemKind::Shell => 0,      // handled separately
        }
    }
}

/// User-defined custom web service
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomWebService {
    pub name: String,
    pub prefixes: Vec<String>,
    pub icon: String,
    pub url_template: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    /// Global hotkey for daemon mode
    pub global_hotkey: String,
    /// Toggle daemon keymap processing on/off while keeping this hotkey alive.
    pub toggle_keymap: String,
    pub quit: String,
    pub next: String,
    pub prev: String,
    pub select: String,
    pub toggle_preview: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            global_hotkey: String::new(),
            toggle_keymap: "ctrl+alt+k".to_string(),
            quit: "ctrl+c".to_string(),
            next: "down".to_string(),
            prev: "up".to_string(),
            select: "enter".to_string(),
            toggle_preview: "ctrl+p".to_string(),
        }
    }
}

impl Config {
    /// Load config from a directory (reads config.toml)
    pub fn load(config_dir: &Path) -> Result<Self, ConfigError> {
        let config_path = config_dir.join(crate::CONFIG_FILENAME);
        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| ConfigError::Io(config_path.clone(), e))?;
            toml::from_str(&content).map_err(ConfigError::Parse)?
        } else {
            Config::default()
        };
        config.config_path = Some(config_path);
        Ok(config)
    }

    /// Save config to its TOML file
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = self.config_path.as_ref().ok_or(ConfigError::NoPath)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io(path.clone(), e))?;
        }

        let content =
            toml::to_string_pretty(self).map_err(|e| ConfigError::Serialize(e.to_string()))?;
        std::fs::write(path, content).map_err(|e| ConfigError::Io(path.clone(), e))?;
        Ok(())
    }

    /// Get a config value by dot-separated key path
    pub fn get_value(&self, key: &str) -> Option<String> {
        // Macro to reduce duplication between get/set match arms
        macro_rules! get {
            ($field:expr) => {
                Some($field.to_string())
            };
            (str $field:expr) => {
                Some($field.clone())
            };
            (opt $field:expr) => {
                Some($field.clone().unwrap_or_default())
            };
        }
        match key {
            // general
            "general.render_fps" => get!(self.general.render_fps),
            "general.show_preview" => get!(self.general.show_preview),
            "general.preview_width_percent" => get!(self.general.preview_width_percent),
            "general.theme" => get!(str self.general.theme),
            "general.editor" => get!(opt self.general.editor),
            "general.emoji_icons" => get!(self.general.emoji_icons),
            "general.reset_ime_on_launch" => get!(self.general.reset_ime_on_launch),
            // launcher
            "launcher.file_search_provider" => get!(str self.launcher.file_search_provider),
            "launcher.max_results" => get!(self.launcher.max_results),
            "launcher.search_depth" => get!(self.launcher.search_depth),
            "launcher.quit_on_launch" => get!(self.launcher.quit_on_launch),
            "launcher.index_directories" => get!(self.launcher.index_directories),
            "launcher.scan_drives" => get!(self.launcher.scan_drives),
            "launcher.drive_scan_depth" => get!(self.launcher.drive_scan_depth),
            // kind_weights
            "launcher.kind_weights.directory" => get!(self.launcher.kind_weights.directory),
            "launcher.kind_weights.app" => get!(self.launcher.kind_weights.app),
            "launcher.kind_weights.file" => get!(self.launcher.kind_weights.file),
            "launcher.kind_weights.executable" => get!(self.launcher.kind_weights.executable),
            "launcher.kind_weights.system_cmd" => get!(self.launcher.kind_weights.system_cmd),
            "launcher.kind_weights.web_search" => get!(self.launcher.kind_weights.web_search),
            "launcher.multi_llm_providers" => Some(self.launcher.multi_llm_providers.join(",")),
            "launcher.multi_llm_prefixes" => Some(self.launcher.multi_llm_prefixes.join(",")),
            "launcher.multi_web_providers" => Some(self.launcher.multi_web_providers.join(",")),
            "launcher.multi_web_prefixes" => Some(self.launcher.multi_web_prefixes.join(",")),
            "launcher.spell_providers" => Some(self.launcher.spell_providers.join(",")),
            "launcher.spell_prefixes" => Some(self.launcher.spell_prefixes.join(",")),
            "launcher.translate_providers" => Some(self.launcher.translate_providers.join(",")),
            "launcher.translate_prefixes" => Some(self.launcher.translate_prefixes.join(",")),
            "launcher.prompt_templates" => {
                let list: Vec<String> = self
                    .launcher
                    .prompt_templates
                    .iter()
                    .map(|t| format!("{}:{}", t.name, t.body))
                    .collect();
                Some(list.join("|"))
            }
            "launcher.keymap.backend" => Some(self.launcher.keymap.backend.clone()),
            "launcher.keymap.kanata_path" => Some(
                self.launcher
                    .keymap
                    .kanata_path
                    .clone()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            ),
            "launcher.keymap.profile_dir" => Some(
                self.launcher
                    .keymap
                    .profile_dir
                    .clone()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            ),
            "launcher.keymap.active_profile" => Some(self.launcher.keymap.active_profile.clone()),
            // keybindings
            "keybindings.global_hotkey" => get!(str self.keybindings.global_hotkey),
            "keybindings.toggle_keymap" => get!(str self.keybindings.toggle_keymap),
            "keybindings.quit" => get!(str self.keybindings.quit),
            "keybindings.next" => get!(str self.keybindings.next),
            "keybindings.prev" => get!(str self.keybindings.prev),
            "keybindings.select" => get!(str self.keybindings.select),
            "keybindings.toggle_preview" => get!(str self.keybindings.toggle_preview),
            // virtual keys (read-only, computed at runtime)
            "_portable_mode" => Some(
                if crate::portable::is_portable() {
                    "Portable"
                } else {
                    "System"
                }
                .into(),
            ),
            "_data_path" => Some(Self::default_data_dir().to_string_lossy().to_string()),
            _ => None,
        }
    }

    /// Set a config value by dot-separated key path
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        /// Parse `value` into the same type as `current`, returning `current` on failure.
        fn parse_or<T: std::str::FromStr>(value: &str, current: T) -> T {
            value.parse().unwrap_or(current)
        }

        match key {
            // general
            "general.render_fps" => {
                self.general.render_fps = parse_or(value, self.general.render_fps)
            }
            "general.show_preview" => {
                self.general.show_preview = parse_or(value, self.general.show_preview)
            }
            "general.preview_width_percent" => {
                self.general.preview_width_percent =
                    parse_or(value, self.general.preview_width_percent)
            }
            "general.theme" => self.general.theme = value.to_string(),
            "general.editor" => {
                self.general.editor = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                }
            }
            "general.emoji_icons" => {
                self.general.emoji_icons = parse_or(value, self.general.emoji_icons)
            }
            "general.reset_ime_on_launch" => {
                self.general.reset_ime_on_launch = parse_or(value, self.general.reset_ime_on_launch)
            }
            // launcher
            "launcher.file_search_provider" => {
                self.launcher.file_search_provider = value.to_string()
            }
            "launcher.max_results" => {
                self.launcher.max_results = parse_or(value, self.launcher.max_results)
            }
            "launcher.search_depth" => {
                self.launcher.search_depth = parse_or(value, self.launcher.search_depth)
            }
            "launcher.quit_on_launch" => {
                self.launcher.quit_on_launch = parse_or(value, self.launcher.quit_on_launch)
            }
            "launcher.index_directories" => {
                self.launcher.index_directories = parse_or(value, self.launcher.index_directories)
            }
            "launcher.scan_drives" => {
                self.launcher.scan_drives = parse_or(value, self.launcher.scan_drives)
            }
            "launcher.drive_scan_depth" => {
                self.launcher.drive_scan_depth = parse_or(value, self.launcher.drive_scan_depth)
            }
            // kind_weights
            "launcher.kind_weights.directory" => {
                self.launcher.kind_weights.directory =
                    parse_or(value, self.launcher.kind_weights.directory)
            }
            "launcher.kind_weights.app" => {
                self.launcher.kind_weights.app = parse_or(value, self.launcher.kind_weights.app)
            }
            "launcher.kind_weights.file" => {
                self.launcher.kind_weights.file = parse_or(value, self.launcher.kind_weights.file)
            }
            "launcher.kind_weights.executable" => {
                self.launcher.kind_weights.executable =
                    parse_or(value, self.launcher.kind_weights.executable)
            }
            "launcher.kind_weights.system_cmd" => {
                self.launcher.kind_weights.system_cmd =
                    parse_or(value, self.launcher.kind_weights.system_cmd)
            }
            "launcher.kind_weights.web_search" => {
                self.launcher.kind_weights.web_search =
                    parse_or(value, self.launcher.kind_weights.web_search)
            }
            "launcher.multi_llm_providers" => {
                self.launcher.multi_llm_providers = value
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "launcher.multi_llm_prefixes" => {
                self.launcher.multi_llm_prefixes = value
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        if s.starts_with('@') {
                            s
                        } else {
                            format!("@{s}")
                        }
                    })
                    .collect();
            }
            "launcher.multi_web_providers" => {
                self.launcher.multi_web_providers = value
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "launcher.multi_web_prefixes" => {
                self.launcher.multi_web_prefixes = value
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        if s.starts_with('@') {
                            s
                        } else {
                            format!("@{s}")
                        }
                    })
                    .collect();
            }
            "launcher.spell_providers" => {
                self.launcher.spell_providers = value
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "launcher.spell_prefixes" => {
                self.launcher.spell_prefixes = value
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        if s.starts_with('@') {
                            s
                        } else {
                            format!("@{s}")
                        }
                    })
                    .collect();
            }
            "launcher.translate_providers" => {
                self.launcher.translate_providers = value
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "launcher.translate_prefixes" => {
                self.launcher.translate_prefixes = value
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        if s.starts_with('@') {
                            s
                        } else {
                            format!("@{s}")
                        }
                    })
                    .collect();
            }
            "launcher.keymap.backend" => self.launcher.keymap.backend = value.to_string(),
            "launcher.keymap.kanata_path" => {
                self.launcher.keymap.kanata_path = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                }
            }
            "launcher.keymap.profile_dir" => {
                self.launcher.keymap.profile_dir = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                }
            }
            "launcher.keymap.active_profile" => {
                self.launcher.keymap.active_profile = value.to_string()
            }
            // keybindings
            "keybindings.global_hotkey" => self.keybindings.global_hotkey = value.to_string(),
            "keybindings.toggle_keymap" => self.keybindings.toggle_keymap = value.to_string(),
            "keybindings.quit" => self.keybindings.quit = value.to_string(),
            "keybindings.next" => self.keybindings.next = value.to_string(),
            "keybindings.prev" => self.keybindings.prev = value.to_string(),
            "keybindings.select" => self.keybindings.select = value.to_string(),
            "keybindings.toggle_preview" => self.keybindings.toggle_preview = value.to_string(),
            _ => return Err(ConfigError::UnknownKey(key.to_string())),
        }
        Ok(())
    }

    /// Return the OS-standard config directory (ignoring portable mode).
    pub fn system_config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kmd")
    }

    /// Return the OS-standard data directory (ignoring portable mode).
    pub fn system_data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kmd")
    }

    /// Return the config directory for kmd.
    ///
    /// In portable mode (`kmd-data/` next to exe) this returns the portable
    /// directory. Otherwise the OS-standard config location is used.
    pub fn default_config_dir() -> PathBuf {
        if let Some(dir) = crate::portable::portable_data_dir() {
            if dir.is_dir() {
                return dir;
            }
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kmd")
    }

    /// Return the data directory for kmd.
    ///
    /// In portable mode (`kmd-data/` next to exe) this returns the portable
    /// directory. Otherwise the OS-standard data location is used.
    pub fn default_data_dir() -> PathBuf {
        if let Some(dir) = crate::portable::portable_data_dir() {
            if dir.is_dir() {
                return dir;
            }
        }
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kmd")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Config file I/O error at {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("Config parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Config serialize error: {0}")]
    Serialize(String),
    #[error("Config path not set")]
    NoPath,
    #[error("Unknown config key: {0}")]
    UnknownKey(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.render_fps, 30);
        assert_eq!(config.launcher.file_search_provider, "auto");
        assert!(config.keybindings.global_hotkey.is_empty());
        assert_eq!(config.launcher.kind_weights.directory, 80);
        assert!(config.launcher.index_directories);
        assert!(!config.launcher.scan_drives);
        assert_eq!(config.launcher.drive_scan_depth, 2);
        assert_eq!(
            config.launcher.multi_llm_providers,
            vec!["chatgpt", "gemini", "claude", "grok", "perplexity"]
        );
        assert_eq!(
            config.launcher.multi_llm_prefixes,
            vec!["@llm", "@ll", "@multi", "@cmp", "@compare"]
        );
        assert_eq!(
            config.launcher.multi_web_providers,
            vec!["google", "naver_search", "daum"]
        );
        assert_eq!(
            config.launcher.multi_web_prefixes,
            vec![
                "@m",
                "@mw",
                "@msearch",
                "@multisearch",
                "@searchall",
                "@krsearch"
            ]
        );
        assert_eq!(
            config.launcher.spell_providers,
            vec!["naver_spell", "pusan_spell"]
        );
        assert_eq!(config.launcher.spell_prefixes, vec!["@sp", "@spell"]);
        assert_eq!(
            config.launcher.translate_providers,
            vec!["google_translate", "papago", "deepl"]
        );
        assert_eq!(
            config.launcher.translate_prefixes,
            vec!["@tr", "@trko", "@tren"]
        );
        assert_eq!(config.launcher.keymap.backend, "kanata");
        assert_eq!(config.launcher.keymap.active_profile, "vim-nav");
    }

    #[test]
    fn test_get_set_value() {
        let mut config = Config::default();
        config.set_value("general.theme", "nord").unwrap();
        assert_eq!(config.get_value("general.theme"), Some("nord".to_string()));

        config
            .set_value("launcher.kind_weights.directory", "90")
            .unwrap();
        assert_eq!(
            config.get_value("launcher.kind_weights.directory"),
            Some("90".to_string())
        );

        config
            .set_value("launcher.multi_llm_providers", "chatgpt,claude,perplexity")
            .unwrap();
        assert_eq!(
            config.get_value("launcher.multi_llm_providers"),
            Some("chatgpt,claude,perplexity".to_string())
        );
        config
            .set_value("launcher.multi_llm_prefixes", "llm,ll,cmp")
            .unwrap();
        assert_eq!(
            config.get_value("launcher.multi_llm_prefixes"),
            Some("@llm,@ll,@cmp".to_string())
        );

        config
            .set_value("launcher.multi_web_providers", "google,daum")
            .unwrap();
        assert_eq!(
            config.get_value("launcher.multi_web_providers"),
            Some("google,daum".to_string())
        );
        config
            .set_value("launcher.multi_web_prefixes", "m,mw,msearch")
            .unwrap();
        assert_eq!(
            config.get_value("launcher.multi_web_prefixes"),
            Some("@m,@mw,@msearch".to_string())
        );
        config
            .set_value("launcher.spell_prefixes", "sp,spell")
            .unwrap();
        assert_eq!(
            config.get_value("launcher.spell_prefixes"),
            Some("@sp,@spell".to_string())
        );
        config
            .set_value("launcher.translate_prefixes", "tr,trko,tren")
            .unwrap();
        assert_eq!(
            config.get_value("launcher.translate_prefixes"),
            Some("@tr,@trko,@tren".to_string())
        );
        config
            .set_value("launcher.keymap.active_profile", "coding.kbd")
            .unwrap();
        assert_eq!(
            config.get_value("launcher.keymap.active_profile"),
            Some("coding.kbd".to_string())
        );
    }

    #[test]
    fn test_unknown_key() {
        let mut config = Config::default();
        assert!(config.set_value("nonexistent.key", "val").is_err());
    }
}
