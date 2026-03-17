pub mod config;
pub mod daemon;
pub mod emoji;
pub mod history;
pub mod index;
pub mod keymap;
pub mod launch;
pub mod plugin;
pub mod portable;
pub mod prompt;
pub mod search;
pub mod version;

use std::path::{Path, PathBuf};

/// 인덱스 로드 순서: bincode → JSON fallback → 새로 빌드.
/// 빌드 후에는 bincode + JSON 둘 다 저장.
pub fn load_or_build_index(
    launcher_config: &kmd_core::config::LauncherConfig,
    use_emoji: bool,
) -> kmd_core::Index {
    let data_dir = kmd_core::Config::default_data_dir();
    let bin_path = data_dir.join(kmd_core::INDEX_CACHE_BIN_FILENAME);
    let json_path = data_dir.join(kmd_core::INDEX_CACHE_FILENAME);
    let expected_version = kmd_core::Index::current_version();

    // 1) bincode 캐시 시도
    if bin_path.exists() {
        match kmd_core::index::store::load_index_bin(&bin_path) {
            Ok(index) if index.version == expected_version => return index,
            Ok(index) => tracing::info!(
                "Bincode cache version mismatch (cache={:?}, expected={:?}), rebuilding",
                index.version, expected_version
            ),
            Err(e) => tracing::warn!("Failed to read bincode cache: {e}"),
        }
    }

    // 2) JSON fallback
    if json_path.exists() {
        match kmd_core::index::store::load_index(&json_path) {
            Ok(index) if index.version == expected_version => {
                // JSON에서 로드 성공 → bincode 캐시 생성 후 반환
                if let Err(e) = kmd_core::index::store::save_index_bin(&index, &bin_path) {
                    tracing::warn!("Failed to write bincode cache from JSON: {e}");
                }
                return index;
            }
            Ok(index) => tracing::info!(
                "JSON cache version mismatch (cache={:?}, expected={:?}), rebuilding",
                index.version, expected_version
            ),
            Err(e) => tracing::warn!("Failed to read JSON cache: {e}"),
        }
    }

    // 3) 새로 빌드
    let index = kmd_core::Index::build(launcher_config, use_emoji);
    save_both_caches(&index, &bin_path, &json_path);
    index
}

/// bincode + JSON 캐시 동시 저장
fn save_both_caches(index: &kmd_core::Index, bin_path: &Path, json_path: &Path) {
    if let Err(e) = kmd_core::index::store::save_index_bin(index, bin_path) {
        tracing::warn!("Failed to save bincode cache: {e}");
    }
    if let Err(e) = kmd_core::index::store::save_index(index, json_path) {
        tracing::warn!("Failed to save JSON cache: {e}");
    }
}

/// JSON 인덱스 캐시 경로 (index 명령 등에서 사용)
pub(crate) fn index_cache_path() -> PathBuf {
    kmd_core::Config::default_data_dir().join(kmd_core::INDEX_CACHE_FILENAME)
}

/// bincode 인덱스 캐시 경로
pub(crate) fn index_cache_bin_path() -> PathBuf {
    kmd_core::Config::default_data_dir().join(kmd_core::INDEX_CACHE_BIN_FILENAME)
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
pub fn create_search_engine(config: &kmd_core::Config) -> kmd_core::SearchEngine {
    let index = load_or_build_index(&config.launcher, config.general.emoji_icons);
    let mut engine = kmd_core::SearchEngine::new();
    engine.set_kind_weights(config.launcher.kind_weights.clone());
    engine.load(index.items);
    engine
}
