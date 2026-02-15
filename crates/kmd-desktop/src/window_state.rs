//! Persist window position and width between sessions.
//!
//! Saved to `{data_dir}/desktop/window_state.json` — a tiny JSON file
//! that lets the launcher remember where the user placed it.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct WindowState {
    /// Window X position (logical pixels).
    pub x: Option<f32>,
    /// Window Y position (logical pixels).
    pub y: Option<f32>,
    /// Window width (logical pixels). Height is dynamic.
    pub width: Option<f32>,
}

impl WindowState {
    /// Path to the state file.
    fn state_path() -> PathBuf {
        kmd_core::Config::default_data_dir()
            .join("desktop")
            .join("window_state.json")
    }

    /// Load saved state, returning defaults if file is missing or corrupt.
    pub fn load() -> Self {
        let path = Self::state_path();
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist the current state to disk.
    pub fn save(&self) {
        let path = Self::state_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, content);
        }
    }

    /// Delete the state file, resetting to defaults on next launch.
    pub fn reset() {
        let _ = std::fs::remove_file(Self::state_path());
    }
}
