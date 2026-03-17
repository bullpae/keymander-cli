//! Search engine bootstrap — config loading + index management.
//!
//! Extracted so that `app.rs` focuses only on Elm state/update/view,
//! and this module handles all kmd-core integration concerns.
use std::time::{Duration, Instant, SystemTime};

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

/// Full index 캐시가 24시간 이내에 빌드되었으면 true.
/// bincode → JSON 순으로 확인.
pub fn is_full_index_cache_fresh() -> bool {
    let data_dir = kmd_core::Config::default_data_dir();

    // bincode 캐시 먼저 확인
    let bin_path = data_dir.join(kmd_core::INDEX_CACHE_BIN_FILENAME);
    if let Ok(meta) = std::fs::metadata(&bin_path) {
        if let Ok(modified) = meta.modified() {
            let age = SystemTime::now()
                .duration_since(modified)
                .unwrap_or(Duration::from_secs(u64::MAX));
            if age.as_secs() < INDEX_FRESHNESS_SECS {
                return true;
            }
        }
    }

    // JSON fallback
    let json_path = data_dir.join(kmd_core::INDEX_CACHE_FILENAME);
    let Ok(meta) = std::fs::metadata(&json_path) else {
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
    let desktop_dir = data_dir.join("desktop");
    let bin_path = desktop_dir.join(QUICK_INDEX_CACHE_BIN_FILENAME);
    let json_path = desktop_dir.join(QUICK_INDEX_CACHE_FILENAME);
    let expected_version = kmd_core::Index::current_version();

    // 1) bincode 캐시 시도
    if bin_path.exists() {
        match kmd_core::index::store::load_index_bin(&bin_path) {
            Ok(cached) if cached.version == expected_version => {
                tracing::info!("Quick index bincode cache hit in {} ms", started.elapsed().as_millis());
                return cached;
            }
            Ok(_) => tracing::info!("Quick index bincode version mismatch, rebuilding"),
            Err(e) => tracing::warn!("Failed to read quick index bincode cache: {e}"),
        }
    }

    // 2) JSON fallback
    if json_path.exists() {
        match kmd_core::index::store::load_index(&json_path) {
            Ok(cached) if cached.version == expected_version => {
                tracing::info!("Quick index JSON cache hit in {} ms", started.elapsed().as_millis());
                let _ = kmd_core::index::store::save_index_bin(&cached, &bin_path);
                return cached;
            }
            Ok(_) => tracing::info!("Quick index JSON version mismatch, rebuilding"),
            Err(e) => tracing::warn!("Failed to read quick index JSON cache: {e}"),
        }
    }

    // 3) 새로 빌드
    let index = kmd_core::Index::build_quick(use_emoji);

    if let Err(e) = kmd_core::index::store::save_index_bin(&index, &bin_path) {
        tracing::warn!("Failed to save quick index bincode cache: {e}");
    }
    if let Err(e) = kmd_core::index::store::save_index(&index, &json_path) {
        tracing::warn!("Failed to save quick index JSON cache: {e}");
    }

    tracing::info!("Quick index rebuilt from source in {} ms", started.elapsed().as_millis());
    index
}

/// 인덱스 로드: bincode → JSON fallback → 새로 빌드
fn load_or_build_index(config: &kmd_core::Config) -> kmd_core::Index {
    let started = Instant::now();
    let data_dir = kmd_core::Config::default_data_dir();
    let bin_path = data_dir.join(kmd_core::INDEX_CACHE_BIN_FILENAME);
    let json_path = data_dir.join(kmd_core::INDEX_CACHE_FILENAME);
    let expected_version = kmd_core::Index::current_version();

    // 1) bincode 캐시 시도
    if bin_path.exists() {
        match kmd_core::index::store::load_index_bin(&bin_path) {
            Ok(cached) if cached.version == expected_version => {
                tracing::info!("Index bincode cache hit in {} ms", started.elapsed().as_millis());
                return cached;
            }
            Ok(cached) => tracing::info!(
                "Bincode cache version mismatch (cache={:?}, expected={:?}), rebuilding",
                cached.version, expected_version
            ),
            Err(e) => tracing::warn!("Failed to read bincode cache: {e}"),
        }
    }

    // 2) JSON fallback
    if json_path.exists() {
        match kmd_core::index::store::load_index(&json_path) {
            Ok(cached) if cached.version == expected_version => {
                tracing::info!("Index JSON cache hit in {} ms", started.elapsed().as_millis());
                let _ = kmd_core::index::store::save_index_bin(&cached, &bin_path);
                return cached;
            }
            Ok(cached) => tracing::info!(
                "JSON cache version mismatch (cache={:?}, expected={:?}), rebuilding",
                cached.version, expected_version
            ),
            Err(e) => tracing::warn!("Failed to read JSON cache: {e}"),
        }
    }

    // 3) 새로 빌드
    let idx = kmd_core::Index::build(&config.launcher, config.general.emoji_icons);

    if let Err(e) = kmd_core::index::store::save_index_bin(&idx, &bin_path) {
        tracing::warn!("Failed to save bincode cache: {e}");
    }
    if let Err(e) = kmd_core::index::store::save_index(&idx, &json_path) {
        tracing::warn!("Failed to save JSON cache: {e}");
    }

    tracing::info!("Index built from source in {} ms", started.elapsed().as_millis());
    idx
}
