//! TUI interactive launcher frontend
//!
//! Activated when `kmd` is run with no subcommand.

pub mod app;
pub mod event;
pub mod settings;
pub mod theme;
pub mod ui;

use color_eyre::Result;

/// Run the TUI launcher
pub fn run() -> Result<()> {
    app::run_app()
}
