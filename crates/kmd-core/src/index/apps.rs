//! OS-specific application discovery
//!
//! Discovers installed applications:
//! - Windows: Start Menu shortcuts (.lnk files)
//! - macOS: /Applications/*.app
//! - Linux: .desktop files in XDG directories

use super::{IndexItem, ItemKind, Source};

/// Collect installed applications from OS-specific locations
pub fn collect_apps() -> Vec<IndexItem> {
    let mut items = Vec::new();

    #[cfg(target_os = "windows")]
    {
        items.extend(collect_windows_apps());
    }

    #[cfg(target_os = "macos")]
    {
        items.extend(collect_macos_apps());
    }

    #[cfg(target_os = "linux")]
    {
        items.extend(collect_linux_apps());
    }

    items
}

/// Windows: Scan Start Menu directories for .lnk files
#[cfg(target_os = "windows")]
fn collect_windows_apps() -> Vec<IndexItem> {
    use std::collections::HashSet;
    use std::path::PathBuf;

    let mut items = Vec::new();
    let mut seen = HashSet::new();

    // Common Start Menu locations
    let mut dirs: Vec<PathBuf> = Vec::new();

    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(
            PathBuf::from(&appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    if let Ok(programdata) = std::env::var("PROGRAMDATA") {
        dirs.push(
            PathBuf::from(&programdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    for dir in dirs {
        scan_lnk_dir(&dir, &mut items, &mut seen);
    }

    items
}

#[cfg(target_os = "windows")]
fn scan_lnk_dir(
    dir: &std::path::Path,
    items: &mut Vec<IndexItem>,
    seen: &mut std::collections::HashSet<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_lnk_dir(&path, items, seen);
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        if ext.to_lowercase() != "lnk" {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if name.is_empty() || !seen.insert(name.clone()) {
            continue;
        }

        // Skip common uninstallers
        let name_lower = name.to_lowercase();
        if name_lower.contains("uninstall") || name_lower.contains("제거") {
            continue;
        }

        let full_path = path.to_string_lossy().to_string();

        items.push(IndexItem {
            name,
            path: full_path.clone(),
            kind: ItemKind::App,
            source: Source::Apps,
            icon: "\u{1F4E6}".to_string(), // 📦
            keywords: full_path,
        });
    }
}

/// macOS: Scan /Applications for .app bundles
#[cfg(target_os = "macos")]
fn collect_macos_apps() -> Vec<IndexItem> {
    use std::path::PathBuf;

    let mut items = Vec::new();

    let app_dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        dirs::home_dir()
            .map(|h| h.join("Applications"))
            .unwrap_or_default(),
    ];

    for dir in app_dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };

            if ext != "app" {
                continue;
            }

            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if name.is_empty() {
                continue;
            }

            let full_path = path.to_string_lossy().to_string();

            items.push(IndexItem {
                name,
                path: full_path.clone(),
                kind: ItemKind::App,
                source: Source::Apps,
                icon: "\u{1F4E6}".to_string(), // 📦
                keywords: full_path,
            });
        }
    }

    items
}

/// Linux: Parse .desktop files from XDG data directories
#[cfg(target_os = "linux")]
fn collect_linux_apps() -> Vec<IndexItem> {
    use std::collections::HashSet;
    use std::path::PathBuf;

    let mut items = Vec::new();
    let mut seen = HashSet::new();

    let mut dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];

    if let Some(data_home) = dirs::data_dir() {
        dirs.push(data_home.join("applications"));
    }

    // XDG_DATA_DIRS
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for dir in xdg.split(':') {
            dirs.push(PathBuf::from(dir).join("applications"));
        }
    }

    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if ext != "desktop" {
                continue;
            }

            if let Some(item) = parse_desktop_file(&path, &mut seen) {
                items.push(item);
            }
        }
    }

    items
}

/// Parse a .desktop file and extract name/exec
#[cfg(target_os = "linux")]
fn parse_desktop_file(
    path: &std::path::Path,
    seen: &mut std::collections::HashSet<String>,
) -> Option<IndexItem> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut name = None;
    let mut exec = None;
    let mut no_display = false;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if line.starts_with('[') {
            in_desktop_entry = false;
            continue;
        }
        if !in_desktop_entry {
            continue;
        }

        if let Some(val) = line.strip_prefix("Name=") {
            name = Some(val.to_string());
        } else if let Some(val) = line.strip_prefix("Exec=") {
            // Remove field codes like %u, %f, %U, etc.
            let clean = val
                .split_whitespace()
                .filter(|s| !s.starts_with('%'))
                .collect::<Vec<_>>()
                .join(" ");
            exec = Some(clean);
        } else if let Some(val) = line.strip_prefix("NoDisplay=") {
            no_display = val.trim().eq_ignore_ascii_case("true");
        }
    }

    if no_display {
        return None;
    }

    let name = name?;
    let exec = exec.unwrap_or_default();

    if !seen.insert(name.clone()) {
        return None;
    }

    Some(IndexItem {
        name: name.clone(),
        path: exec.clone(),
        kind: ItemKind::App,
        source: Source::Apps,
        icon: "\u{1F4E6}".to_string(), // 📦
        keywords: format!("{} {}", name, exec),
    })
}
