//! Index system — discovers and caches applications, files, and commands.

pub mod apps;
pub mod files;
pub mod path;
pub mod store;
pub mod system_commands;

use serde::{Deserialize, Serialize};

/// A single indexed item (app, file, executable, system command, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexItem {
    /// Display name (e.g. "Firefox", "report.pdf")
    pub name: String,
    /// Full path or command value
    pub path: String,
    /// Item kind
    pub kind: ItemKind,
    /// Where this item came from
    pub source: Source,
    /// Icon (emoji)
    pub icon: String,
    /// Search keywords (joined subtitle/keywords for matching)
    pub keywords: String,
}

/// Item classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    App,
    File,
    Executable,
    SystemCommand,
    WebSearch,
    Directory,
}

impl std::fmt::Display for ItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::App => write!(f, "App"),
            Self::File => write!(f, "File"),
            Self::Executable => write!(f, "Exe"),
            Self::SystemCommand => write!(f, "System"),
            Self::WebSearch => write!(f, "Web"),
            Self::Directory => write!(f, "Dir"),
        }
    }
}

/// Where the item was discovered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// OS application directories
    Apps,
    /// PATH executables
    Path,
    /// File search provider (fd, Everything, etc.)
    FileProvider,
    /// Built-in system commands
    SystemCommand,
    /// Plugin-provided
    Plugin,
}

/// The full index: a collection of items with metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Index {
    pub items: Vec<IndexItem>,
    /// ISO 8601 timestamp of last rebuild
    pub last_updated: Option<String>,
}

impl Index {
    /// Create a new empty index
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the full index from all sources
    pub fn build(config: &crate::config::LauncherConfig) -> Self {
        let mut items = Vec::new();

        // 1. PATH executables (always included)
        items.extend(path::collect_executables());

        // 2. System commands (always included)
        items.extend(system_commands::collect_system_commands());

        // 3. OS applications
        items.extend(apps::collect_apps());

        // 4. File search provider
        let provider_kind = files::detect_provider(
            &config.file_search_provider,
            config.everything_path.as_ref(),
        );
        let provider_config = files::ProviderConfig {
            max_results: config.max_results,
            search_paths: config.search_paths.clone(),
            ignore_patterns: config.ignore_patterns.clone(),
            everything_path: config.everything_path.clone(),
        };
        items.extend(files::collect_files(provider_kind, &provider_config));

        let now = chrono_now();

        Self {
            items,
            last_updated: Some(now),
        }
    }

    /// Total number of indexed items
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the index is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Simple UTC timestamp without chrono dependency
fn chrono_now() -> String {
    // Use std::time for a basic timestamp
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Simple ISO-like format: just the unix timestamp for now
    // In production, we'd format this properly
    format!("{}", secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_new_is_empty() {
        let idx = Index::new();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn test_item_kind_display() {
        assert_eq!(format!("{}", ItemKind::App), "App");
        assert_eq!(format!("{}", ItemKind::File), "File");
        assert_eq!(format!("{}", ItemKind::SystemCommand), "System");
    }
}
