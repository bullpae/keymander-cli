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
    const MIN_WIDTH: f32 = 420.0;
    const MAX_WIDTH: f32 = 1200.0;

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
            Ok(content) => match serde_json::from_str(&content) {
                Ok(state) => Self::sanitize(state),
                Err(e) => {
                    tracing::warn!("Failed to parse window state ({}): {e}", path.display());
                    Self::default()
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read window state ({}): {e}", path.display());
                Self::default()
            }
        }
    }

    /// Persist the current state to disk.
    pub fn save(&self) {
        let path = Self::state_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!(
                    "Failed to create window state directory ({}): {e}",
                    parent.display()
                );
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!("Failed to write window state ({}): {e}", path.display());
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize window state ({}): {e}", path.display());
            }
        }
    }

    /// Delete the state file, resetting to defaults on next launch.
    pub fn reset() {
        let _ = std::fs::remove_file(Self::state_path());
    }

    fn sanitize(mut state: Self) -> Self {
        state.x = state.x.filter(|v| v.is_finite());
        state.y = state.y.filter(|v| v.is_finite());
        state.width = state
            .width
            .filter(|v| v.is_finite())
            .map(|v| v.clamp(Self::MIN_WIDTH, Self::MAX_WIDTH));
        state
    }
}
