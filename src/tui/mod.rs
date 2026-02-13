//! TUI interactive launcher frontend
//!
//! Activated when `kmd` is run with no subcommand.

pub mod app;
pub mod event;
pub mod settings;
pub mod theme;
pub mod ui;

use color_eyre::Result;
use kmd_core::single_instance::Guard;

/// Run the TUI launcher
pub fn run(instance_guard: Option<Guard>) -> Result<()> {
    app::run_app(instance_guard)
}
