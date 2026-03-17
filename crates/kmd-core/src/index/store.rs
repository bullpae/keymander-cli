//! Index persistence — save/load index to/from disk (JSON + bincode)

use std::path::Path;

use super::Index;

// ── JSON (레거시 호환) ──────────────────────────────────────────────────────

/// JSON 형식으로 인덱스 저장
pub fn save_index(index: &Index, path: &Path) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
    }
    let json = serde_json::to_string(index).map_err(StoreError::Json)?;
    std::fs::write(path, json).map_err(StoreError::Io)?;
    Ok(())
}

/// JSON 형식에서 인덱스 로드
pub fn load_index(path: &Path) -> Result<Index, StoreError> {
    let content = std::fs::read_to_string(path).map_err(StoreError::Io)?;
    let index: Index = serde_json::from_str(&content).map_err(StoreError::Json)?;
    Ok(index)
}

// ── Bincode (고속 바이너리) ──────────────────────────────────────────────────

/// bincode 형식으로 인덱스 저장
pub fn save_index_bin(index: &Index, path: &Path) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
    }
    let bytes = bincode::serialize(index).map_err(StoreError::Bincode)?;
    std::fs::write(path, bytes).map_err(StoreError::Io)?;
    Ok(())
}

/// bincode 형식에서 인덱스 로드
pub fn load_index_bin(path: &Path) -> Result<Index, StoreError> {
    let bytes = std::fs::read(path).map_err(StoreError::Io)?;
    let index: Index = bincode::deserialize(&bytes).map_err(StoreError::Bincode)?;
    Ok(index)
}

// ── 공용 캐시 헬퍼 ──────────────────────────────────────────────────────────

/// bincode + JSON 캐시 동시 저장 (개별 실패는 경고 로그)
pub fn save_both(index: &Index, bin_path: &Path, json_path: &Path) {
    if let Err(e) = save_index_bin(index, bin_path) {
        tracing::warn!("Failed to save bincode cache: {e}");
    }
    if let Err(e) = save_index(index, json_path) {
        tracing::warn!("Failed to save JSON cache: {e}");
    }
}

/// 캐시에서 인덱스 로드 시도 (bincode → JSON fallback).
/// `expected_version`과 일치해야 반환. JSON 히트 시 bincode 캐시를 자동 생성.
pub fn try_load_cached(
    bin_path: &Path,
    json_path: &Path,
    expected_version: &str,
) -> Option<Index> {
    if bin_path.exists() {
        match load_index_bin(bin_path) {
            Ok(idx) if idx.version == expected_version => return Some(idx),
            Ok(_) => tracing::info!("Bincode cache version mismatch, skipping"),
            Err(e) => tracing::warn!("Failed to read bincode cache: {e}"),
        }
    }
    if json_path.exists() {
        match load_index(json_path) {
            Ok(idx) if idx.version == expected_version => {
                let _ = save_index_bin(&idx, bin_path);
                return Some(idx);
            }
            Ok(_) => tracing::info!("JSON cache version mismatch, skipping"),
            Err(e) => tracing::warn!("Failed to read JSON cache: {e}"),
        }
    }
    None
}

// ── 에러 타입 ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
    #[error("Bincode error: {0}")]
    Bincode(bincode::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{IndexItem, ItemKind, Source};

    fn sample_index() -> Index {
        let mut index = Index::new();
        index.items.push(IndexItem {
            name: "test.rs".to_string(),
            path: "/home/user/test.rs".to_string(),
            kind: ItemKind::File,
            source: Source::FileProvider,
            icon: "Rs".to_string(),
            keywords: "/home/user/test.rs".to_string(),
        });
        index.items.push(IndexItem {
            name: "docs".to_string(),
            path: "/home/user/docs".to_string(),
            kind: ItemKind::Directory,
            source: Source::FileProvider,
            icon: ">>".to_string(),
            keywords: "/home/user/docs".to_string(),
        });
        index
    }

    #[test]
    fn test_json_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.json");
        let index = sample_index();

        save_index(&index, &path).unwrap();
        let loaded = load_index(&path).unwrap();

        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.items[0].name, "test.rs");
        assert_eq!(loaded.items[1].kind, ItemKind::Directory);
        assert_eq!(loaded.version, index.version);
    }

    #[test]
    fn test_bincode_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.bin");
        let index = sample_index();

        save_index_bin(&index, &path).unwrap();
        let loaded = load_index_bin(&path).unwrap();

        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.items[0].name, "test.rs");
        assert_eq!(loaded.items[1].kind, ItemKind::Directory);
        assert_eq!(loaded.version, index.version);
    }

    #[test]
    fn test_bincode_smaller_than_json() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("index.json");
        let bin_path = dir.path().join("index.bin");
        let index = sample_index();

        save_index(&index, &json_path).unwrap();
        save_index_bin(&index, &bin_path).unwrap();

        let json_size = std::fs::metadata(&json_path).unwrap().len();
        let bin_size = std::fs::metadata(&bin_path).unwrap().len();
        assert!(bin_size < json_size, "bincode({bin_size}) >= json({json_size})");
    }

    #[test]
    fn test_load_nonexistent_file_returns_error() {
        let result = load_index(std::path::Path::new("/nonexistent/path/index.json"));
        assert!(result.is_err());
        let result = load_index_bin(std::path::Path::new("/nonexistent/path/index.bin"));
        assert!(result.is_err());
    }
}
