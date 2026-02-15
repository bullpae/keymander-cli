//! keymander Desktop — GPU-accelerated Spotlight-like launcher
//!
//! A borderless, transparent, floating search window powered by iced.
//! Shares the same kmd-core search engine and portable data as the CLI.

mod app;
mod theme;

use iced::window;

fn main() -> iced::Result {
    // Logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kmd_desktop=info".parse().unwrap())
                .add_directive("kmd_core=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    tracing::info!("Starting keymander Desktop");

    iced::application(app::App::new, app::App::update, app::App::view)
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
        .subscription(app::App::subscription)
        .run()
}
