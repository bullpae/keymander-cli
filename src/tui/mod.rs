//! TUI interactive launcher frontend
//!
//! Activated when `kmd` is run with no subcommand.

pub mod app;
pub mod event;
pub mod ui;
pub mod theme;

use color_eyre::Result;

/// Run the TUI launcher
pub async fn run() -> Result<()> {
    app::run_app().await
}
