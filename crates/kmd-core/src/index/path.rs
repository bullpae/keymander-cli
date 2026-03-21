//! PATH executable discovery
//!
//! Scans directories in the PATH environment variable for executable files.

use std::collections::HashSet;

use super::{IndexItem, ItemKind, Source};

/// Collect all executables from PATH
pub fn collect_executables(use_emoji: bool) -> Vec<IndexItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();

    let path_separator = if cfg!(target_os = "windows") {
        ';'
    } else {
        ':'
    };

    let path_env = augmented_path();

    for dir in path_env.split(path_separator) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name_str) = name.to_str() else {
                continue;
            };

            // Skip hidden files
            if name_str.starts_with('.') {
                continue;
            }

            // On Windows, only include common executable extensions
            #[cfg(target_os = "windows")]
            {
                let lower = name_str.to_lowercase();
                if !lower.ends_with(".exe")
                    && !lower.ends_with(".cmd")
                    && !lower.ends_with(".bat")
                    && !lower.ends_with(".com")
                {
                    continue;
                }
            }

            // Deduplicate by name
            if !seen.insert(name_str.to_string()) {
                continue;
            }

            let full_path = entry.path().to_string_lossy().to_string();

            items.push(IndexItem {
                name: name_str.to_string(),
                path: full_path.clone(),
                kind: ItemKind::Executable,
                source: Source::Path,
                icon: if use_emoji {
                    "\u{26A1}".to_string()
                } else {
                    "Ex".to_string()
                },
                keywords: full_path,
                icon_path: None,
            });
        }
    }

    items
}

/// macOS/Linux GUI 앱에서 PATH가 제한적일 수 있으므로 공통 디렉토리를 보강
fn augmented_path() -> String {
    let base = std::env::var("PATH").unwrap_or_default();

    #[cfg(not(target_os = "windows"))]
    {
        let mut extra: Vec<String> = Vec::new();

        if let Some(home) = dirs::home_dir() {
            for sub in [".cargo/bin", ".local/bin", "go/bin"] {
                let p = home.join(sub);
                if p.is_dir() {
                    extra.push(p.to_string_lossy().into_owned());
                }
            }
        }

        #[cfg(target_os = "macos")]
        for p in ["/opt/homebrew/bin", "/opt/homebrew/sbin"] {
            if std::path::Path::new(p).is_dir() {
                extra.push(p.to_string());
            }
        }

        for p in ["/usr/local/bin", "/usr/local/sbin"] {
            if std::path::Path::new(p).is_dir() {
                extra.push(p.to_string());
            }
        }

        if extra.is_empty() {
            return base;
        }

        let existing: std::collections::HashSet<&str> = base.split(':').collect();
        let new_dirs: Vec<&str> = extra
            .iter()
            .filter(|d| !existing.contains(d.as_str()))
            .map(|d| d.as_str())
            .collect();

        if new_dirs.is_empty() {
            return base;
        }

        format!("{}:{}", base, new_dirs.join(":"))
    }

    #[cfg(target_os = "windows")]
    {
        base
    }
}
