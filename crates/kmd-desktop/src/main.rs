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

    // ── Preload config (fast — just reads a TOML file) ────────────────────
    // This lets us apply the user's theme immediately instead of defaulting.
    let config = engine::load_config();

    // Wrap boot data in Mutex<Option<>> so the Fn closure can take it once.
    let boot_data = Mutex::new(Some((guard, config)));

    iced::application(
        move || {
            let (guard, config) = boot_data
                .lock()
                .expect("boot mutex poisoned")
                .take()
                .expect("boot called more than once");
            app::App::new(guard, config)
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
