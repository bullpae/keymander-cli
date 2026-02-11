//! File search providers — collect files from various backends
//!
//! Supports: fd, Everything (Windows), mdfind (macOS), locate (Linux),
//! Windows built-in FS scan, and a builtin walkdir fallback.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{IndexItem, ItemKind, Source};

/// Provider kind
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProviderKind {
    /// No file search (PATH executables only)
    Builtin,
    /// fd / fdfind (cross-platform)
    Fd,
    /// voidtools Everything (Windows)
    Everything,
    /// macOS Spotlight (mdfind)
    Spotlight,
    /// plocate / mlocate (Linux)
    Locate,
    /// Windows built-in PowerShell scan
    WinFs,
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin => write!(f, "builtin"),
            Self::Fd => write!(f, "fd"),
            Self::Everything => write!(f, "everything"),
            Self::Spotlight => write!(f, "mdfind"),
            Self::Locate => write!(f, "locate"),
            Self::WinFs => write!(f, "winfs"),
        }
    }
}

/// Provider configuration
pub struct ProviderConfig {
    pub max_results: usize,
    pub search_paths: Vec<PathBuf>,
    pub ignore_patterns: Vec<String>,
    pub everything_path: Option<PathBuf>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            max_results: 5000,
            search_paths: vec![],
            ignore_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
            ],
            everything_path: None,
        }
    }
}

/// Auto-detect the best available provider
pub fn detect_provider(preference: &str, everything_path: Option<&PathBuf>) -> ProviderKind {
    match preference.to_lowercase().as_str() {
        "builtin" => return ProviderKind::Builtin,
        "fd" => {
            if which("fd").is_some() || which("fdfind").is_some() {
                return ProviderKind::Fd;
            }
        }
        "everything" => {
            if cfg!(target_os = "windows") && find_everything_cli(everything_path).is_some() {
                return ProviderKind::Everything;
            }
            if cfg!(target_os = "windows") {
                return ProviderKind::WinFs;
            }
        }
        "winfs" => {
            if cfg!(target_os = "windows") {
                return ProviderKind::WinFs;
            }
        }
        "mdfind" | "spotlight" => {
            if cfg!(target_os = "macos") {
                return ProviderKind::Spotlight;
            }
        }
        "locate" | "plocate" => {
            if which("plocate").is_some() || which("locate").is_some() {
                return ProviderKind::Locate;
            }
        }
        _ => {} // "auto" → fall through to auto-detect
    }

    // Auto-detect priority
    if cfg!(target_os = "windows") && find_everything_cli(everything_path).is_some() {
        return ProviderKind::Everything;
    }
    if cfg!(target_os = "windows") {
        return ProviderKind::WinFs;
    }
    if cfg!(target_os = "macos") {
        return ProviderKind::Spotlight;
    }
    if which("fd").is_some() || which("fdfind").is_some() {
        return ProviderKind::Fd;
    }
    if which("plocate").is_some() || which("locate").is_some() {
        return ProviderKind::Locate;
    }

    ProviderKind::Builtin
}

/// Collect files using the specified provider
pub fn collect_files(kind: ProviderKind, config: &ProviderConfig) -> Vec<IndexItem> {
    match kind {
        ProviderKind::Builtin => Vec::new(),
        ProviderKind::Fd => collect_fd(config),
        ProviderKind::Everything => collect_everything(config),
        ProviderKind::Spotlight => collect_spotlight(config),
        ProviderKind::Locate => collect_locate(config),
        ProviderKind::WinFs => collect_windows_fs(config),
    }
}

// ── fd / fdfind ──────────────────────────────────────────

fn collect_fd(config: &ProviderConfig) -> Vec<IndexItem> {
    let cmd_name = if which("fd").is_some() {
        "fd"
    } else if which("fdfind").is_some() {
        "fdfind"
    } else {
        return Vec::new();
    };

    let mut args = vec![
        ".".to_string(),
        "--type".to_string(),
        "f".to_string(),
        "--max-results".to_string(),
        config.max_results.to_string(),
        "--color".to_string(),
        "never".to_string(),
    ];

    for pattern in &config.ignore_patterns {
        args.push("--exclude".to_string());
        args.push(pattern.clone());
    }

    let search_dirs: Vec<PathBuf> = if config.search_paths.is_empty() {
        dirs::home_dir().into_iter().collect()
    } else {
        config.search_paths.clone()
    };

    let mut all_items = Vec::new();

    for dir in &search_dirs {
        let output = Command::new(cmd_name)
            .args(&args)
            .arg(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output {
            parse_line_output_into(&output.stdout, &mut all_items, config.max_results);
        }

        if all_items.len() >= config.max_results {
            break;
        }
    }

    all_items
}

// ── Everything (Windows) ────────────────────────────────

fn collect_everything(config: &ProviderConfig) -> Vec<IndexItem> {
    let es_path = match find_everything_cli(config.everything_path.as_ref()) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut args = vec![
        "-n".to_string(),
        config.max_results.to_string(),
        "-s".to_string(),
    ];

    if !config.search_paths.is_empty() {
        let path_filter: Vec<String> = config
            .search_paths
            .iter()
            .map(|p| format!("\"{}\"", p.to_string_lossy()))
            .collect();
        args.push(path_filter.join(" | "));
    }

    let mut cmd = Command::new(&es_path);
    cmd.args(&args).stdout(Stdio::piped()).stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    match cmd.output() {
        Ok(output) => {
            let mut items = Vec::new();
            parse_line_output_into(&output.stdout, &mut items, config.max_results);
            items
        }
        Err(_) => Vec::new(),
    }
}

// ── mdfind (macOS) ──────────────────────────────────────

fn collect_spotlight(config: &ProviderConfig) -> Vec<IndexItem> {
    let mut args = vec!["kind:document OR kind:folder".to_string()];

    if let Some(dir) = config.search_paths.first() {
        args.push("-onlyin".to_string());
        args.push(dir.to_string_lossy().to_string());
    }

    match Command::new("mdfind")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => {
            let mut items = Vec::new();
            parse_line_output_into(&output.stdout, &mut items, config.max_results);
            items
        }
        Err(_) => Vec::new(),
    }
}

// ── locate / plocate (Linux) ────────────────────────────

fn collect_locate(config: &ProviderConfig) -> Vec<IndexItem> {
    let cmd = if which("plocate").is_some() {
        "plocate"
    } else if which("locate").is_some() {
        "locate"
    } else {
        return Vec::new();
    };

    match Command::new(cmd)
        .args(["--limit", &config.max_results.to_string(), "--existing", "/"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => {
            let mut items = Vec::new();
            parse_line_output_into(&output.stdout, &mut items, config.max_results);
            items
        }
        Err(_) => Vec::new(),
    }
}

// ── Windows FS (PowerShell fallback) ────────────────────

fn collect_windows_fs(config: &ProviderConfig) -> Vec<IndexItem> {
    if !cfg!(target_os = "windows") {
        return Vec::new();
    }

    let roots: Vec<PathBuf> = if config.search_paths.is_empty() {
        std::env::var("USERPROFILE")
            .ok()
            .map(PathBuf::from)
            .into_iter()
            .collect()
    } else {
        config.search_paths.clone()
    };

    let mut items = Vec::new();

    for root in roots {
        let root_s = root.to_string_lossy().replace('\'', "''");
        let script = format!(
            "Get-ChildItem -LiteralPath '{}' -File -Recurse -ErrorAction SilentlyContinue | Select-Object -First {} -ExpandProperty FullName",
            root_s, config.max_results
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output {
            parse_line_output_into(&output.stdout, &mut items, config.max_results);
        }

        if items.len() >= config.max_results {
            items.truncate(config.max_results);
            break;
        }
    }

    items
}

// ── Helpers ─────────────────────────────────────────────

/// Parse line-by-line command output into IndexItems
fn parse_line_output_into(stdout: &[u8], items: &mut Vec<IndexItem>, max: usize) {
    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let path = PathBuf::from(&line);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&line)
            .to_string();

        items.push(IndexItem {
            name: name.clone(),
            path: line.clone(),
            kind: ItemKind::File,
            source: Source::FileProvider,
            icon: icon_for_path(&path),
            keywords: line,
        });

        if items.len() >= max {
            break;
        }
    }
}

/// Check if an executable exists in PATH
fn which(name: &str) -> Option<PathBuf> {
    let candidates: Vec<String> = if cfg!(target_os = "windows") {
        vec![
            name.to_string(),
            format!("{}.exe", name),
            format!("{}.cmd", name),
        ]
    } else {
        vec![name.to_string()]
    };

    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            candidates.iter().find_map(|candidate| {
                let full = dir.join(candidate);
                if full.is_file() {
                    Some(full)
                } else {
                    None
                }
            })
        })
    })
}

/// Find Everything CLI (es.exe)
fn find_everything_cli(configured: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(path) = configured {
        if path.is_file() {
            return Some(path.clone());
        }
    }

    if let Some(path) = which("es") {
        return Some(path);
    }

    if cfg!(target_os = "windows") {
        let common_paths = [
            r"C:\Program Files\Everything\es.exe",
            r"C:\Program Files (x86)\Everything\es.exe",
            r"C:\Program Files\Everything 1.5a\es.exe",
        ];
        for path_str in &common_paths {
            let path = PathBuf::from(path_str);
            if path.is_file() {
                return Some(path);
            }
        }

        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let path = PathBuf::from(&local).join("Everything").join("es.exe");
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

/// Determine icon by file extension
fn icon_for_path(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "rs" => "\u{1F980}".to_string(),             // 🦀
        "py" => "\u{1F40D}".to_string(),             // 🐍
        "js" | "ts" | "jsx" | "tsx" => "\u{1F4DC}".to_string(), // 📜
        "go" => "\u{1F535}".to_string(),             // 🔵
        "java" | "kt" => "\u{2615}".to_string(),    // ☕
        "c" | "cpp" | "h" | "hpp" => "\u{2699}\u{FE0F}".to_string(), // ⚙️
        "sh" | "bash" | "zsh" => "\u{1F41A}".to_string(), // 🐚
        "md" | "txt" | "doc" | "docx" => "\u{1F4DD}".to_string(), // 📝
        "pdf" => "\u{1F4D5}".to_string(),            // 📕
        "json" | "yaml" | "yml" | "toml" | "xml" => "\u{1F4CB}".to_string(), // 📋
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => "\u{1F5BC}\u{FE0F}".to_string(), // 🖼️
        "mp3" | "wav" | "flac" => "\u{1F3B5}".to_string(), // 🎵
        "mp4" | "mkv" | "avi" | "mov" => "\u{1F3AC}".to_string(), // 🎬
        "zip" | "tar" | "gz" | "7z" | "rar" => "\u{1F4E6}".to_string(), // 📦
        "exe" | "msi" => "\u{1F4E6}".to_string(),   // 📦
        "" if path.is_dir() => "\u{1F4C1}".to_string(), // 📁
        _ => "\u{1F4C4}".to_string(),               // 📄
    }
}
