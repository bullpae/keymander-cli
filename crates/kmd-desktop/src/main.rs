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
mod query_prefix;
mod theme;
mod window_state;

use iced::{window, Color, Point, Size};
use std::fs::{self, OpenOptions};
use std::sync::Mutex;

use crate::app::{full_window_height, DEFAULT_WIDTH};
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

    let desktop_data_dir = kmd_core::Config::default_data_dir().join("desktop");
    let log_dir = desktop_data_dir.join("logs");
    let log_path = log_dir.join("desktop.log");

    let make_filter = || {
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("kmd_desktop=warn".parse().unwrap())
            .add_directive("kmd_core=warn".parse().unwrap())
    };

    let log_file = fs::create_dir_all(&log_dir).ok().and_then(|_| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok()
    });

    // 파일 I/O를 백그라운드 스레드로 오프로드 — 키 입력마다 UI 스레드가 디스크
    // 쓰기로 멈추지 않게 한다. WorkerGuard는 main 종료 시점까지 살아 있어야
    // 버퍼에 남은 로그가 flush된다.
    let _log_guard = match log_file {
        Some(file) => {
            let (non_blocking, guard) = tracing_appender::non_blocking(file);
            tracing_subscriber::fmt()
                .with_env_filter(make_filter())
                .with_target(false)
                .with_ansi(false)
                .with_writer(non_blocking)
                .init();
            tracing::info!("로그 파일 경로: {}", log_path.display());
            Some(guard)
        }
        None => {
            tracing_subscriber::fmt()
                .with_env_filter(make_filter())
                .with_target(false)
                .init();
            tracing::warn!("파일 로그 초기화 실패: 콘솔 출력만 사용");
            None
        }
    };

    tracing::info!("Starting keymander Desktop");

    // ── Singleton toggle ──────────────────────────────────────────────────
    let data_dir = desktop_data_dir;
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
    let win_height = full_window_height(config.general.font_size, config.general.visible_rows);
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
            let (guard, config, ws) = lock
                .take()
                .expect("iced 초기화가 두 번 이상 호출됨 — 프레임워크 버그");
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
