//! `kmd daemon` — background daemon with global hotkey support
//!
//! Phase 4: This module provides the daemon infrastructure.
//! Platform-specific hotkey registration will be added in a future version.

use color_eyre::Result;

pub enum Action {
    Start,
    Stop,
    Status,
}

pub fn run(action: Action) -> Result<()> {
    match action {
        Action::Start => {
            println!("Starting kmd daemon...");
            println!();
            println!(
                "  Hotkey: {} (configurable via `kmd config set keybindings.global_hotkey`)",
                kmd_core::Config::default().keybindings.global_hotkey
            );
            println!();
            println!("Note: Global hotkey daemon is planned for a future release.");
            println!("For now, you can bind `kmd` to a keyboard shortcut in your OS or terminal.");
            println!();
            println!("Quick setup suggestions:");
            #[cfg(target_os = "windows")]
            {
                println!("  Windows Terminal: Add to settings.json:");
                println!("    {{ \"command\": \"kmd\", \"keys\": \"alt+space\" }}");
            }
            #[cfg(target_os = "macos")]
            {
                println!("  macOS: Use Automator or Raycast to bind a shortcut to `kmd`");
                println!("  iTerm2: Preferences > Keys > Add hotkey to open kmd");
            }
            #[cfg(target_os = "linux")]
            {
                println!("  GNOME: Settings > Keyboard > Custom Shortcuts > Add `kmd`");
                println!("  KDE: System Settings > Shortcuts > Custom > Add `kmd`");
                println!("  i3/sway: bindsym $mod+space exec --no-startup-id terminal -e kmd");
            }
        }
        Action::Stop => {
            println!("No daemon is currently running.");
        }
        Action::Status => {
            println!("Daemon status: not running");
            println!("(Global hotkey daemon is planned for a future release)");
        }
    }

    Ok(())
}
