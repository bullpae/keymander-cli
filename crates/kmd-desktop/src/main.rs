//! keymander Desktop — GPU-accelerated Spotlight-like launcher
//!
//! A borderless, transparent, floating search window powered by iced.
//! Shares the same kmd-core search engine and portable data as the CLI.
//!
//! **Singleton toggle**: launching a second instance signals the first to quit.

// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod app_icons;
mod brand_icons;
mod engine;
mod platform;
mod theme;
mod window_state;

use iced::{window, Color, Point, Size};
use std::sync::Mutex;

use crate::app::{DEFAULT_WIDTH, full_window_height};
use crate::window_state::WindowState;

fn should_print_version() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "--version" | "-V" | "version"))
}

fn print_version() {
    println!("kmd-desktop {}", env!("CARGO_PKG_VERSION"));
    println!("kmd-core {}", kmd_core::Index::current_version());
    println!("target {}", std::env::consts::ARCH);
    println!("os {}", std::env::consts::OS);
}

/// Default position: horizontally centered, vertically at 1/3 from top.
fn default_position(win: Size, monitor: Size) -> Point {
    Point::new(
        (monitor.width - win.width) / 2.0,
        (monitor.height / 3.0).max(0.0),
    )
}

/// 바이너리에 임베드된 아이콘 PNG를 32x32 RGBA로 디코딩하여 윈도우 아이콘 생성.
fn create_icon() -> Option<window::Icon> {
    const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon.png");
    let img = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png).ok()?;
    let resized = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    window::icon::from_rgba(rgba.into_raw(), 32, 32).ok()
}

fn main() -> iced::Result {
    if should_print_version() {
        print_version();
        return Ok(());
    }

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

    let base_width = window_state
        .width
        .unwrap_or(DEFAULT_WIDTH)
        .clamp(420.0, 1200.0);
    let win_height = full_window_height(config.general.font_size);
    let initial_size = Size::new(base_width, win_height);

    let position = match (window_state.x, window_state.y) {
        (Some(x), Some(y)) => window::Position::Specific(Point::new(x, y)),
        _ => window::Position::SpecificWith(default_position),
    };

    let icon = create_icon();

    let boot_data = Mutex::new(Some((guard, config, window_state)));

    iced::application(
        move || {
            let mut lock = boot_data.lock().unwrap_or_else(|e| e.into_inner());
            let (guard, config, ws) = lock.take().expect("boot called more than once");
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
        min_size: Some(Size::new(420.0, win_height)),
        max_size: Some(Size::new(1600.0, win_height)),
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
