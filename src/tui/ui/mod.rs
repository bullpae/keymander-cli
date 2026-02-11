//! UI layout and rendering

pub mod input;
pub mod list;
pub mod preview;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::app::AppState;
use super::theme::Theme;

/// Render the entire TUI
pub fn render(frame: &mut Frame, state: &AppState, theme: &Theme) {
    let area = frame.area();

    // Main layout: header + input + content + status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // header
            Constraint::Length(3),  // input bar
            Constraint::Min(3),    // content (list + optional preview)
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_header(frame, chunks[0], state, theme);
    input::render(frame, chunks[1], state, theme);

    // Content: list only, or list + preview
    if state.show_preview && area.width > 60 {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(chunks[2]);

        list::render(frame, content_chunks[0], state, theme);
        preview::render(frame, content_chunks[1], state, theme);
    } else {
        list::render(frame, chunks[2], state, theme);
    }

    render_status(frame, chunks[3], state, theme);
}

fn render_header(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let version = env!("CARGO_PKG_VERSION");
    let text = format!(" kmd v{}  |  {} items indexed", version, state.total_items);
    let header = ratatui::widgets::Paragraph::new(text)
        .style(theme.header_style());
    frame.render_widget(header, area);
}

fn render_status(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let mode_label = state.search_mode_label();
    let result_count = state.results.len();
    let lang_indicator = if state.hangul_mode { "한" } else { "EN" };
    let nav_hints = if state.drill_path.is_some() {
        "↑↓ navigate  Tab/→ open folder  ←/Esc back  Enter launch"
    } else {
        "↑↓ navigate  Tab/→ open folder  Enter launch  Esc quit  Ctrl+Space 한/EN"
    };
    let text = format!(
        " [{}] {} results  |  {} {}  |  {}",
        mode_label,
        result_count,
        lang_indicator,
        if state.hangul_mode {
            "Korean mode"
        } else {
            "English mode"
        },
        nav_hints,
    );
    let status = ratatui::widgets::Paragraph::new(text)
        .style(theme.status_style());
    frame.render_widget(status, area);
}
