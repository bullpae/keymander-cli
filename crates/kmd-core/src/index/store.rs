//! Index persistence — save/load index to/from disk as JSON

use std::path::Path;

use super::Index;

/// Save index to a JSON file
pub fn save_index(index: &Index, path: &Path) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
    }

    let json = serde_json::to_string(index).map_err(StoreError::Serialize)?;
    std::fs::write(path, json).map_err(StoreError::Io)?;
    Ok(())
}

/// Load index from a JSON file
pub fn load_index(path: &Path) -> Result<Index, StoreError> {
    let content = std::fs::read_to_string(path).map_err(StoreError::Io)?;
    let index: Index = serde_json::from_str(&content).map_err(StoreError::Deserialize)?;
    Ok(index)
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("Serialize error: {0}")]
    Serialize(serde_json::Error),
    #[error("Deserialize error: {0}")]
    Deserialize(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{IndexItem, ItemKind, Source};

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.json");

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

        save_index(&index, &path).unwrap();
        let loaded = load_index(&path).unwrap();

        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.items[0].name, "test.rs");
        assert_eq!(loaded.items[1].kind, ItemKind::Directory);
        assert_eq!(loaded.version, index.version);
    }

    #[test]
    fn test_load_nonexistent_file_returns_error() {
        let result = load_index(std::path::Path::new("/nonexistent/path/index.json"));
        assert!(result.is_err());
    }
}
