//! File search providers — collect files from various backends
//!
//! Supports: walkdir builtin, fd, Everything (Windows), mdfind (macOS),
//! locate (Linux), and Windows PowerShell fallback.
//!
//! Strategy: "priority directories first" — Desktop, Documents, Downloads
//! are scanned before general home directory traversal so that user documents
//! are always indexed even if max_results is reached.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use walkdir::WalkDir;

use super::{IndexItem, ItemKind, Source};

// ============================================================================
// Public Types
// ============================================================================

/// Provider kind
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProviderKind {
    /// Built-in walkdir scanner (always available, no external tools)
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
            Self::Builtin => write!(f, "builtin (walkdir)"),
            Self::Fd => write!(f, "fd"),
            Self::Everything => write!(f, "everything"),
            Self::Spotlight => write!(f, "mdfind"),
            Self::Locate => write!(f, "locate"),
            Self::WinFs => write!(f, "winfs (PowerShell)"),
        }
    }
}

/// Provider configuration
pub struct ProviderConfig {
    pub max_results: usize,
    pub search_depth: usize,
    pub search_paths: Vec<PathBuf>,
    pub ignore_patterns: Vec<String>,
    pub everything_path: Option<PathBuf>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            max_results: 10000,
            search_depth: 6,
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

// ============================================================================
// Priority Directories
// ============================================================================

/// Get the platform-specific priority directories that users most likely
/// want to search. These are scanned first to guarantee document indexing.
pub fn priority_directories() -> Vec<PathBuf> {
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
            // Also check common non-C: drive roots for user data
            for letter in ['D', 'E', 'F', 'G'] {
                let drive = PathBuf::from(format!("{}:\\", letter));
                if drive.is_dir() {
                    dirs.push(drive);
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
        // XDG user directories
        for env_key in &[
            "XDG_DESKTOP_DIR",
            "XDG_DOCUMENTS_DIR",
            "XDG_DOWNLOAD_DIR",
        ] {
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

/// Collect files from priority directories using walkdir.
/// This guarantees that user documents (Desktop, Documents, Downloads)
/// are always in the index, regardless of the file provider used.
pub fn collect_priority_files(config: &ProviderConfig) -> Vec<IndexItem> {
    let mut priority_dirs = priority_directories();

    // Also include user-configured search_paths as priority
    for p in &config.search_paths {
        if p.is_dir() && !priority_dirs.contains(p) {
            priority_dirs.push(p.clone());
        }
    }

    if priority_dirs.is_empty() {
        return Vec::new();
    }

    let ignore_set: HashSet<&str> = config.ignore_patterns.iter().map(|s| s.as_str()).collect();

    let mut items = Vec::new();
    let mut seen_paths = HashSet::new();

    for dir in &priority_dirs {
        tracing::info!("Scanning priority directory: {}", dir.display());

        let walker = WalkDir::new(dir)
            .max_depth(config.search_depth)
            .follow_links(false)
            .into_iter();

        for entry in walker.filter_entry(|e| !is_ignored_dir(e, &ignore_set)) {
            let Ok(entry) = entry else { continue };

            let is_file = entry.file_type().is_file();
            let is_dir = entry.file_type().is_dir();

            // Skip entries that are neither files nor directories (e.g. symlinks)
            if !is_file && !is_dir {
                continue;
            }

            // Skip the root priority directories themselves (depth 0)
            if is_dir && entry.depth() == 0 {
                continue;
            }

            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            // Deduplicate
            if !seen_paths.insert(path_str.clone()) {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if name.is_empty() || name.starts_with('.') {
                continue;
            }

            items.push(IndexItem {
                name: name.clone(),
                path: path_str.clone(),
                kind: if is_dir {
                    ItemKind::Directory
                } else {
                    ItemKind::File
                },
                source: Source::FileProvider,
                icon: if is_dir {
                    "\u{1F4C1}".to_string() // 📁
                } else {
                    icon_for_path(path)
                },
                keywords: path_str,
            });

            // Reserve space for general scan: use half of max_results for priority
            if items.len() >= config.max_results / 2 {
                break;
            }
        }

        if items.len() >= config.max_results / 2 {
            break;
        }
    }

    tracing::info!("Priority directories: {} items (files + dirs) found", items.len());
    items
}

// ============================================================================
// Provider Detection
// ============================================================================

/// Auto-detect the best available provider
pub fn detect_provider(preference: &str, everything_path: Option<&PathBuf>) -> ProviderKind {
    match preference.to_lowercase().as_str() {
        "builtin" | "walkdir" => return ProviderKind::Builtin,
        "fd" => {
            if which("fd").is_some() || which("fdfind").is_some() {
                return ProviderKind::Fd;
            }
        }
        "everything" => {
            if cfg!(target_os = "windows") && find_everything_cli(everything_path).is_some() {
                return ProviderKind::Everything;
            }
        }
        "winfs" | "powershell" => {
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
    if which("fd").is_some() || which("fdfind").is_some() {
        return ProviderKind::Fd;
    }
    if cfg!(target_os = "macos") {
        return ProviderKind::Spotlight;
    }
    if which("plocate").is_some() || which("locate").is_some() {
        return ProviderKind::Locate;
    }

    // Always fall back to builtin (walkdir) — never return empty
    ProviderKind::Builtin
}

// ============================================================================
// Provider Implementations
// ============================================================================

/// Collect files using the specified provider.
/// NOTE: Priority directory files are collected separately in Index::build().
/// This function collects additional files from the general home directory.
pub fn collect_files(
    kind: ProviderKind,
    config: &ProviderConfig,
    existing_count: usize,
) -> Vec<IndexItem> {
    let remaining = config.max_results.saturating_sub(existing_count);
    if remaining == 0 {
        return Vec::new();
    }

    let limited_config = ProviderConfig {
        max_results: remaining,
        search_depth: config.search_depth,
        search_paths: config.search_paths.clone(),
        ignore_patterns: config.ignore_patterns.clone(),
        everything_path: config.everything_path.clone(),
    };

    tracing::info!("File provider: {} (quota: {} files)", kind, remaining);

    match kind {
        ProviderKind::Builtin => collect_builtin(&limited_config),
        ProviderKind::Fd => collect_fd(&limited_config),
        ProviderKind::Everything => collect_everything(&limited_config),
        ProviderKind::Spotlight => collect_spotlight(&limited_config),
        ProviderKind::Locate => collect_locate(&limited_config),
        ProviderKind::WinFs => collect_windows_fs(&limited_config),
    }
}

// ── Builtin (walkdir) ────────────────────────────────────

fn collect_builtin(config: &ProviderConfig) -> Vec<IndexItem> {
    let roots: Vec<PathBuf> = if config.search_paths.is_empty() {
        // Scan user home directory (excluding priority dirs already scanned)
        dirs::home_dir().into_iter().collect()
    } else {
        config.search_paths.clone()
    };

    let ignore_set: HashSet<&str> = config.ignore_patterns.iter().map(|s| s.as_str()).collect();
    let mut items = Vec::new();
    let mut seen = HashSet::new();

    // Collect paths from priority dirs to skip (already indexed)
    let priority = priority_directories();
    let priority_set: HashSet<PathBuf> = priority.into_iter().collect();

    for root in &roots {
        let walker = WalkDir::new(root)
            .max_depth(config.search_depth)
            .follow_links(false)
            .into_iter();

        for entry in walker.filter_entry(|e| {
            // Skip ignored directories
            if is_ignored_dir(e, &ignore_set) {
                return false;
            }
            // Skip priority directories (already scanned)
            if e.file_type().is_dir() {
                let p = e.path().to_path_buf();
                if priority_set.contains(&p) {
                    return false;
                }
            }
            true
        }) {
            let Ok(entry) = entry else { continue };

            let is_file = entry.file_type().is_file();
            let is_dir = entry.file_type().is_dir();

            if !is_file && !is_dir {
                continue;
            }

            // Skip the root directories themselves (depth 0)
            if is_dir && entry.depth() == 0 {
                continue;
            }

            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            if !seen.insert(path_str.clone()) {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if name.is_empty() || name.starts_with('.') {
                continue;
            }

            items.push(IndexItem {
                name: name.clone(),
                path: path_str.clone(),
                kind: if is_dir {
                    ItemKind::Directory
                } else {
                    ItemKind::File
                },
                source: Source::FileProvider,
                icon: if is_dir {
                    "\u{1F4C1}".to_string() // 📁
                } else {
                    icon_for_path(path)
                },
                keywords: path_str,
            });

            if items.len() >= config.max_results {
                break;
            }
        }

        if items.len() >= config.max_results {
            break;
        }
    }

    tracing::info!("Builtin provider: {} files found", items.len());
    items
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
        "--max-depth".to_string(),
        config.search_depth.to_string(),
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
        .args([
            "--limit",
            &config.max_results.to_string(),
            "--existing",
            "/",
        ])
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
        // Scan home directory (priority dirs already indexed separately)
        std::env::var("USERPROFILE")
            .ok()
            .map(PathBuf::from)
            .into_iter()
            .collect()
    } else {
        config.search_paths.clone()
    };

    // Build exclude list for PowerShell
    let exclude_dirs: String = config
        .ignore_patterns
        .iter()
        .map(|p| format!("'{}'", p))
        .collect::<Vec<_>>()
        .join(",");

    let mut items = Vec::new();

    for root in roots {
        let root_s = root.to_string_lossy().replace('\'', "''");
        // Use Where-Object to filter out ignored directories from the path
        let script = format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
             $excludeDirs = @({}); \
             Get-ChildItem -LiteralPath '{}' -File -Recurse -Depth {} \
             -ErrorAction SilentlyContinue | \
             Where-Object {{ $p = $_.FullName; -not ($excludeDirs | Where-Object {{ $p -like \"*\\$_\\*\" }}) }} | \
             Select-Object -First {} -ExpandProperty FullName",
            exclude_dirs,
            root_s,
            config.search_depth,
            config.max_results,
        );

        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", &script])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        if let Ok(output) = cmd.output() {
            parse_line_output_into(&output.stdout, &mut items, config.max_results);
        }

        if items.len() >= config.max_results {
            items.truncate(config.max_results);
            break;
        }
    }

    items
}

// ============================================================================
// Helpers
// ============================================================================

/// Check if a walkdir entry should be ignored (directory name matches ignore pattern)
fn is_ignored_dir(entry: &walkdir::DirEntry, ignore_set: &HashSet<&str>) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let name = entry.file_name().to_str().unwrap_or("");
    // Skip hidden directories (starting with '.')
    if name.starts_with('.') && name.len() > 1 {
        return true;
    }
    // Skip system directories starting with '$' (e.g. $Recycle.Bin, $WinREAgent)
    if name.starts_with('$') {
        return true;
    }
    ignore_set.contains(name)
}

/// Parse line-by-line command output into IndexItems
fn parse_line_output_into(stdout: &[u8], items: &mut Vec<IndexItem>, max: usize) {
    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let path = PathBuf::from(&line);
        let is_dir = path.is_dir();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&line)
            .to_string();

        items.push(IndexItem {
            name: name.clone(),
            path: line.clone(),
            kind: if is_dir {
                ItemKind::Directory
            } else {
                ItemKind::File
            },
            source: Source::FileProvider,
            icon: if is_dir {
                "\u{1F4C1}".to_string() // 📁
            } else {
                icon_for_path(&path)
            },
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
pub fn icon_for_path(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        // Programming
        "rs" => "\u{1F980}".to_string(),                           // 🦀
        "py" | "pyw" => "\u{1F40D}".to_string(),                   // 🐍
        "js" | "ts" | "jsx" | "tsx" | "mjs" => "\u{1F4DC}".to_string(), // 📜
        "go" => "\u{1F535}".to_string(),                            // 🔵
        "java" | "kt" | "kts" => "\u{2615}".to_string(),           // ☕
        "c" | "cpp" | "h" | "hpp" | "cc" | "cxx" => "\u{2699}\u{FE0F}".to_string(), // ⚙️
        "cs" => "\u{1F7E3}".to_string(),                           // 🟣
        "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" => "\u{1F41A}".to_string(), // 🐚

        // Documents — Text
        "md" | "txt" | "rtf" | "log" => "\u{1F4DD}".to_string(),   // 📝
        "pdf" => "\u{1F4D5}".to_string(),                          // 📕

        // Documents — Korean / Office
        "hwp" | "hwpx" => "\u{1F4D8}".to_string(),                 // 📘
        "doc" | "docx" | "odt" => "\u{1F4C4}".to_string(),         // 📄
        "xls" | "xlsx" | "ods" | "csv" => "\u{1F4CA}".to_string(), // 📊
        "ppt" | "pptx" | "odp" => "\u{1F4CA}".to_string(),         // 📊

        // Data / Config
        "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "conf" => {
            "\u{1F4CB}".to_string() // 📋
        }
        "sql" | "db" | "sqlite" | "sqlite3" => "\u{1F5C3}\u{FE0F}".to_string(), // 🗃️
        "html" | "htm" | "css" | "scss" | "less" => "\u{1F310}".to_string(), // 🌐

        // Images
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "ico" | "tiff" => {
            "\u{1F5BC}\u{FE0F}".to_string() // 🖼️
        }

        // Audio
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" => {
            "\u{1F3B5}".to_string() // 🎵
        }

        // Video
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" => {
            "\u{1F3AC}".to_string() // 🎬
        }

        // Archives
        "zip" | "tar" | "gz" | "7z" | "rar" | "bz2" | "xz" | "zst" => {
            "\u{1F4E6}".to_string() // 📦
        }

        // Executables / Installers
        "exe" | "msi" | "appimage" | "deb" | "rpm" | "dmg" => {
            "\u{1F4E6}".to_string() // 📦
        }

        // Fonts
        "ttf" | "otf" | "woff" | "woff2" => "\u{1F524}".to_string(), // 🔤

        // Directory
        "" if path.is_dir() => "\u{1F4C1}".to_string(), // 📁

        _ => "\u{1F4C4}".to_string(), // 📄
    }
}
