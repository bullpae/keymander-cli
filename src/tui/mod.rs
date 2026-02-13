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

/// Run the TUI launcher.
///
/// `center_window` — if true, re-centre the console window after entering
/// the alternate screen buffer (ensures consistent position on hotkey launch).
pub fn run(instance_guard: Option<Guard>, center_window: bool) -> Result<()> {
    app::run_app(instance_guard, center_window)
}
