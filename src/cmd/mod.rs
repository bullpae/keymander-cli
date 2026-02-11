pub mod config;
pub mod daemon;
pub mod history;
pub mod index;
pub mod launch;
pub mod plugin;
pub mod search;

use std::path::PathBuf;

/// Get or create the index, loading from cache if available
pub fn load_or_build_index(
    launcher_config: &kmd_core::config::LauncherConfig,
) -> kmd_core::Index {
    let cache_path = index_cache_path();

    // Try loading cached index
    if cache_path.exists() {
        if let Ok(index) = kmd_core::index::store::load_index(&cache_path) {
            return index;
        }
    }

    // Build fresh index
    let index = kmd_core::Index::build(launcher_config);

    // Save to cache
    if let Err(e) = kmd_core::index::store::save_index(&index, &cache_path) {
        tracing::warn!("Failed to save index cache: {}", e);
    }

    index
}

/// Path to the index cache file
fn index_cache_path() -> PathBuf {
    kmd_core::Config::default_data_dir().join("index.json")
}

/// Open the database
pub fn open_db() -> color_eyre::Result<kmd_core::Database> {
    let db_path = kmd_core::Config::default_data_dir().join("kmd.db");
    let db = kmd_core::Database::open(&db_path)?;
    Ok(db)
}

/// Load config
pub fn load_config() -> color_eyre::Result<kmd_core::Config> {
    let config_dir = kmd_core::Config::default_config_dir();
    let config = kmd_core::Config::load(&config_dir)?;
    Ok(config)
}
