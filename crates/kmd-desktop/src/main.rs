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
mod platform;
mod theme;
mod window_state;

use iced::{window, Color, Point, Size};
use std::sync::Mutex;

use crate::app::{DEFAULT_WIDTH, SEARCH_BAR_HEIGHT};
use crate::window_state::WindowState;

/// Default position: horizontally centered, vertically at 1/3 from top.
fn default_position(win: Size, monitor: Size) -> Point {
    Point::new(
        (monitor.width - win.width) / 2.0,
        (monitor.height / 3.0).max(0.0),
    )
}

/// Create a simple 32x32 RGBA icon (accent-colored square).
fn create_icon() -> Option<window::Icon> {
    let size = 32u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let (r, g, b) = (0x56u8, 0xD2u8, 0xFFu8);

    let center = size as f32 / 2.0;
    let outer = center - 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - center).abs();
            let dy = (y as f32 - center).abs();
            let inside = dx <= outer && dy <= outer;
            if inside {
                rgba.extend_from_slice(&[r, g, b, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    window::icon::from_rgba(rgba, size, size).ok()
}

fn main() -> iced::Result {
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
    let data_dir = kmd_core::Config::default_data_dir().join("desktop");
    let guard = match kmd_core::single_instance::acquire_or_toggle(&data_dir) {
        kmd_core::single_instance::InstanceAction::Acquired(guard) => guard,
        kmd_core::single_instance::InstanceAction::SignalledExisting => {
            tracing::info!("Signalled existing desktop instance to quit — exiting");
            return Ok(());
        }
    };

    // ── Preload config + window state ─────────────────────────────────────
    let config = engine::load_config();
    let window_state = WindowState::load();

    let width = window_state.width.unwrap_or(DEFAULT_WIDTH);
    let initial_size = Size::new(width, SEARCH_BAR_HEIGHT);

    let position = match (window_state.x, window_state.y) {
        (Some(x), Some(y)) => window::Position::Specific(Point::new(x, y)),
        _ => window::Position::SpecificWith(default_position),
    };

    let icon = create_icon();

    let boot_data = Mutex::new(Some((guard, config, window_state)));

    iced::application(
        move || {
            let (guard, config, ws) = boot_data
                .lock()
                .expect("boot mutex poisoned")
                .take()
                .expect("boot called more than once");
            app::App::new(guard, config, ws)
        },
        app::App::update,
        app::App::view,
    )
    .window(window::Settings {
        size: initial_size,
        decorations: false,
        transparent: true,
        level: window::Level::AlwaysOnTop,
        position,
        resizable: true,
        visible: true,
        exit_on_close_request: true,
        min_size: Some(Size::new(420.0, SEARCH_BAR_HEIGHT)),
        max_size: Some(Size::new(1200.0, 800.0)),
        icon,
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
