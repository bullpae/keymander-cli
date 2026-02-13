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

    if let Ok(path_env) = std::env::var("PATH") {
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
                    icon: if use_emoji { "\u{26A1}".to_string() } else { "Ex".to_string() },
                    keywords: full_path,
                });
            }
        }
    }

    items
}
