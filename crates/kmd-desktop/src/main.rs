//! keymander Desktop — GPU-accelerated Spotlight-like launcher
//!
//! A borderless, transparent, floating search window powered by iced.
//! Shares the same kmd-core search engine and portable data as the CLI.
//!
//! **Singleton toggle**: launching a second instance signals the first to quit.

// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod engine;
mod theme;

use iced::{window, Color};
use std::sync::Mutex;

fn main() -> iced::Result {
    // Logging — in debug mode goes to console, in release suppressed by windows_subsystem.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kmd_desktop=info".parse().unwrap())
                .add_directive("kmd_core=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    tracing::info!("Starting keymander Desktop");

    // ── Singleton toggle ──────────────────────────────────────────────────
    // Use a separate sub-directory so desktop and CLI don't conflict.
    let data_dir = kmd_core::Config::default_data_dir().join("desktop");
    let guard = match kmd_core::single_instance::acquire_or_toggle(&data_dir) {
        kmd_core::single_instance::InstanceAction::Acquired(guard) => guard,
        kmd_core::single_instance::InstanceAction::SignalledExisting => {
            tracing::info!("Signalled existing desktop instance to quit — exiting");
            return Ok(());
        }
    };

    // Wrap in Mutex<Option<>> so the boot closure (Fn, not FnOnce) can take it.
    let guard_cell = Mutex::new(Some(guard));

    iced::application(
        move || {
            let guard = guard_cell
                .lock()
                .expect("guard mutex poisoned")
                .take()
                .expect("boot called more than once");
            app::App::new(guard)
        },
        app::App::update,
        app::App::view,
    )
    .window(window::Settings {
        size: iced::Size::new(680.0, 56.0),
        decorations: false,
        transparent: true,
        level: window::Level::AlwaysOnTop,
        position: window::Position::Centered,
        resizable: false,
        visible: true,
        exit_on_close_request: true,
        ..Default::default()
    })
    .theme(app::App::theme)
    .style(|_state, _theme| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .subscription(app::App::subscription)
    .antialiasing(true)
    .run()
}
