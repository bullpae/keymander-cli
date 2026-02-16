//! TOML-based configuration management
//!
//! Launcher-focused config with keybindings and provider settings.

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
    /// Maximum recursive directory depth for file scanning (default: 6)
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
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            file_search_provider: "auto".to_string(),
            everything_path: None,
            search_paths: default_search_paths(),
            max_results: 10000,
            search_depth: 6,
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
            scan_drives: true,
            drive_scan_depth: 3,
            kind_weights: KindWeights::default(),
            web_services: vec![],
            multi_llm_providers: default_multi_llm_providers(),
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
    pub quit: String,
    pub next: String,
    pub prev: String,
    pub select: String,
    pub toggle_preview: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            global_hotkey: "alt+space".to_string(),
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
            // keybindings
            "keybindings.global_hotkey" => get!(str self.keybindings.global_hotkey),
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
            // keybindings
            "keybindings.global_hotkey" => self.keybindings.global_hotkey = value.to_string(),
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
        assert_eq!(config.keybindings.global_hotkey, "alt+space");
        assert_eq!(config.launcher.kind_weights.directory, 80);
        assert!(config.launcher.index_directories);
        assert!(config.launcher.scan_drives);
        assert_eq!(config.launcher.drive_scan_depth, 3);
        assert_eq!(
            config.launcher.multi_llm_providers,
            vec!["chatgpt", "gemini", "claude", "grok", "perplexity"]
        );
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
    }

    #[test]
    fn test_unknown_key() {
        let mut config = Config::default();
        assert!(config.set_value("nonexistent.key", "val").is_err());
    }
}
