pub mod config;
pub mod daemon;
pub mod emoji;
pub mod history;
pub mod index;
pub mod launch;
pub mod plugin;
pub mod portable;
pub mod search;

use std::path::PathBuf;

/// Get or create the index, loading from cache if available.
/// Auto-rebuilds when the cache version doesn't match the current binary.
pub fn load_or_build_index(
    launcher_config: &kmd_core::config::LauncherConfig,
    use_emoji: bool,
) -> kmd_core::Index {
    let cache_path = index_cache_path();
    let expected_version = kmd_core::Index::current_version();

    // Try loading cached index (only if version matches)
    if cache_path.exists() {
        if let Ok(index) = kmd_core::index::store::load_index(&cache_path) {
            if index.version == expected_version {
                return index;
            }
            tracing::info!(
                "Index cache version mismatch (cache={:?}, expected={:?}), rebuilding...",
                index.version,
                expected_version
            );
        }
    }

    // Build fresh index
    let index = kmd_core::Index::build(launcher_config, use_emoji);

    // Save to cache
    if let Err(e) = kmd_core::index::store::save_index(&index, &cache_path) {
        tracing::warn!("Failed to save index cache: {}", e);
    }

    index
}

/// Path to the index cache file
pub(crate) fn index_cache_path() -> PathBuf {
    kmd_core::Config::default_data_dir().join(kmd_core::INDEX_CACHE_FILENAME)
}

/// Open the database
pub fn open_db() -> color_eyre::Result<kmd_core::Database> {
    let db_path = kmd_core::Config::default_data_dir().join(kmd_core::DB_FILENAME);
    let db = kmd_core::Database::open(&db_path)?;
    Ok(db)
}

/// Load config
pub fn load_config() -> color_eyre::Result<kmd_core::Config> {
    let config_dir = kmd_core::Config::default_config_dir();
    let config = kmd_core::Config::load(&config_dir)?;
    Ok(config)
}

/// Create a search engine loaded with the full index.
/// Shared helper used by `search` and `launch` subcommands.
pub fn create_search_engine(
    config: &kmd_core::Config,
) -> kmd_core::SearchEngine {
    let index = load_or_build_index(&config.launcher, config.general.emoji_icons);
    let mut engine = kmd_core::SearchEngine::new();
    engine.load(index.items);
    engine
}
