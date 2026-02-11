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
