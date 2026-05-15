//! Search engine bootstrap — config loading + index management.
//!
//! Extracted so that `app.rs` focuses only on Elm state/update/view,
//! and this module handles all kmd-core integration concerns.
use std::time::{Duration, Instant};

const QUICK_INDEX_CACHE_FILENAME: &str = "quick-index.json";
const QUICK_INDEX_CACHE_BIN_FILENAME: &str = "quick-index.bin";
const INDEX_FRESHNESS_SECS: u64 = 24 * 60 * 60;

/// Load the user configuration, falling back to defaults on failure.
pub fn load_config() -> kmd_core::Config {
    let config_dir = kmd_core::Config::default_config_dir();
    match kmd_core::Config::load(&config_dir) {
        Ok(config) => config,
        Err(e) => {
            tracing::warn!(
                "Failed to load config from {}: {e} — using defaults",
                config_dir.display()
            );
            kmd_core::Config::default()
        }
    }
}

/// Build a search engine loaded with the full index.
///
/// Tries loading a cached index first; rebuilds and saves when stale or missing.
pub fn create_search_engine(config: &kmd_core::Config) -> kmd_core::SearchEngine {
    let started = Instant::now();
    let index = load_or_build_index(config);

    tracing::info!("Loaded {} items into search engine", index.items.len());

    let mut engine = kmd_core::SearchEngine::new();
    engine.set_kind_weights(config.launcher.kind_weights.clone());
    engine.load(index.items);
    tracing::info!(
        "Full search engine ready in {} ms",
        started.elapsed().as_millis()
    );
    engine
}

/// Build a lightweight engine for instant first interaction.
///
/// Includes fast sources (apps/PATH/system commands) and skips file crawling.
/// The full engine is loaded asynchronously and replaces this shortly after boot.
pub fn create_quick_search_engine(config: &kmd_core::Config) -> kmd_core::SearchEngine {
    let started = Instant::now();

    let index = load_or_build_quick_index(config.general.emoji_icons);
    let count = index.items.len();

    let mut engine = kmd_core::SearchEngine::new();
    engine.set_kind_weights(config.launcher.kind_weights.clone());
    engine.load(index.items);
    tracing::info!(
        "Quick search engine ready in {} ms ({} items)",
        started.elapsed().as_millis(),
        count
    );
    engine
}

fn load_or_build_quick_index(use_emoji: bool) -> kmd_core::Index {
    use kmd_core::index::store;

    let started = Instant::now();
    let desktop_dir = kmd_core::Config::default_data_dir().join("desktop");
    let bin_path = desktop_dir.join(QUICK_INDEX_CACHE_BIN_FILENAME);
    let json_path = desktop_dir.join(QUICK_INDEX_CACHE_FILENAME);
    let expected_version = kmd_core::Index::current_version();

    if let Some(cached) = store::try_load_cached(&bin_path, &json_path, expected_version) {
        tracing::info!(
            "Quick index cache hit in {} ms",
            started.elapsed().as_millis()
        );
        return cached;
    }

    let index = kmd_core::Index::build_quick(use_emoji);
    store::save_both(&index, &bin_path, &json_path);
    tracing::info!(
        "Quick index rebuilt from source in {} ms",
        started.elapsed().as_millis()
    );
    index
}

/// 인덱스 로드: bincode → JSON fallback → 새로 빌드.
/// 캐시가 24시간보다 오래되면 새로 빌드하여 새 앱/CLI를 반영한다.
fn load_or_build_index(config: &kmd_core::Config) -> kmd_core::Index {
    use kmd_core::index::store;

    let started = Instant::now();
    let data_dir = kmd_core::Config::default_data_dir();
    let bin_path = data_dir.join(kmd_core::INDEX_CACHE_BIN_FILENAME);
    let json_path = data_dir.join(kmd_core::INDEX_CACHE_FILENAME);
    let expected_version = kmd_core::Index::current_version();

    let max_age = Some(Duration::from_secs(INDEX_FRESHNESS_SECS));
    if let Some(cached) =
        store::try_load_cached_with_max_age(&bin_path, &json_path, expected_version, max_age)
    {
        tracing::info!("Index cache hit in {} ms", started.elapsed().as_millis());
        return cached;
    }

    tracing::info!("Index cache miss or stale — rebuilding");
    let index = kmd_core::Index::build(&config.launcher, config.general.emoji_icons);
    store::save_both(&index, &bin_path, &json_path);
    tracing::info!(
        "Index built from source in {} ms",
        started.elapsed().as_millis()
    );
    index
}
