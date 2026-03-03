//! Search engine bootstrap — config loading + index management.
//!
//! Extracted so that `app.rs` focuses only on Elm state/update/view,
//! and this module handles all kmd-core integration concerns.
use std::time::{Duration, Instant, SystemTime};

const QUICK_INDEX_CACHE_FILENAME: &str = "quick-index.json";
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

/// Full index 캐시가 24시간 이내에 빌드되었으면 true.
/// true이면 캐시에서 직접 로드할 수 있어 2-stage startup이 불필요.
/// 버전 미스매치는 `load_or_build_index()`에서 자동 리빌드되므로 여기서는 파일 age만 확인.
pub fn is_full_index_cache_fresh() -> bool {
    let data_dir = kmd_core::Config::default_data_dir();
    let cache_path = data_dir.join(kmd_core::INDEX_CACHE_FILENAME);

    let Ok(meta) = std::fs::metadata(&cache_path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };

    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::from_secs(u64::MAX));

    age.as_secs() < INDEX_FRESHNESS_SECS
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
    let started = Instant::now();
    let data_dir = kmd_core::Config::default_data_dir();
    let cache_path = data_dir.join("desktop").join(QUICK_INDEX_CACHE_FILENAME);
    let expected_version = kmd_core::Index::current_version();

    if cache_path.exists() {
        match kmd_core::index::store::load_index(&cache_path) {
            Ok(cached) if cached.version == expected_version => {
                tracing::info!(
                    "Quick index cache hit in {} ms",
                    started.elapsed().as_millis()
                );
                return cached;
            }
            Ok(cached) => {
                tracing::info!(
                    "Quick index cache version mismatch (cache={:?}, expected={:?}), rebuilding",
                    cached.version,
                    expected_version
                );
            }
            Err(e) => {
                tracing::warn!("Failed to read quick index cache: {e}, rebuilding");
            }
        }
    }

    let index = kmd_core::Index::build_quick(use_emoji);

    if let Err(e) = kmd_core::index::store::save_index(&index, &cache_path) {
        tracing::warn!("Failed to save quick index cache: {e}");
    }

    tracing::info!(
        "Quick index rebuilt from source in {} ms",
        started.elapsed().as_millis()
    );

    index
}

/// Load a cached index or build a fresh one.
fn load_or_build_index(config: &kmd_core::Config) -> kmd_core::Index {
    let started = Instant::now();
    let data_dir = kmd_core::Config::default_data_dir();
    let cache_path = data_dir.join(kmd_core::INDEX_CACHE_FILENAME);
    let expected_version = kmd_core::Index::current_version();

    // Try loading cached index (only if version matches).
    if cache_path.exists() {
        match kmd_core::index::store::load_index(&cache_path) {
            Ok(cached) if cached.version == expected_version => {
                tracing::info!("Index cache hit in {} ms", started.elapsed().as_millis());
                return cached;
            }
            Ok(cached) => {
                tracing::info!(
                    "Index cache version mismatch (cache={:?}, expected={:?}), rebuilding",
                    cached.version,
                    expected_version
                );
            }
            Err(e) => {
                tracing::warn!("Failed to read index cache: {e}, rebuilding");
            }
        }
    }

    // Build fresh index.
    let idx = kmd_core::Index::build(&config.launcher, config.general.emoji_icons);

    if let Err(e) = kmd_core::index::store::save_index(&idx, &cache_path) {
        tracing::warn!("Failed to save index cache: {e}");
    }

    tracing::info!(
        "Index built from source in {} ms",
        started.elapsed().as_millis()
    );

    idx
}
