//! UI layout and rendering
//!
//! Layout structure:
//!   ┌──────────────────────────────────────┐
//!   │  ◆ kmd v0.2.0  ·  5997 items        │  header (1 row)
//!   ├──────────────────────────────────────┤
//!   │  🔍 Search                           │  input (3 rows, rounded)
//!   │  > query_                             │
//!   │                                      │
//!   ├──────────────────────────────────────┤
//!   │  Results               ╭─ Preview ──╮│
//!   │  ▸ item 1              │ Name: ...  ││  content (flex)
//!   │    item 2              │ Type: ...  ││
//!   │    ...                 ╰────────────╯│
//!   ├──────────────────────────────────────┤
//!   │  fuzzy  42 results  ↑↓ Tab Enter Esc│  status (1 row)
//!   └──────────────────────────────────────┘

pub mod input;
pub mod list;
pub mod preview;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};

use super::app::AppState;
use super::theme::Theme;

/// Render the entire TUI
pub fn render(frame: &mut Frame, state: &AppState, theme: &Theme) {
    let area = frame.area();

    // Main layout: header + input + content + status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(3), // input bar
            Constraint::Min(3),   // content (list + optional preview)
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_header(frame, chunks[0], state, theme);
    input::render(frame, chunks[1], state, theme);

    // Content: list only, or list + preview
    if state.show_preview && area.width > 80 {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(55),
                Constraint::Percentage(45),
            ])
            .split(chunks[2]);

        list::render(frame, content_chunks[0], state, theme);
        preview::render(frame, content_chunks[1], state, theme);
    } else {
        list::render(frame, chunks[2], state, theme);
    }

    render_status(frame, chunks[3], state, theme);
}

// ── Header ───────────────────────────────────────────────────────────────────

fn render_header(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let version = env!("CARGO_PKG_VERSION");

    let line = Line::from(vec![
        Span::styled(" \u{25C6} ", theme.header_accent_style()),  // ◆
        Span::styled("kmd", theme.header_style()),
        Span::styled(format!(" v{}", version), theme.header_dim_style()),
        Span::styled("  \u{00B7}  ", theme.header_dim_style()),   // ·
        Span::styled(
            format!("{}", state.total_items),
            theme.header_accent_style(),
        ),
        Span::styled(" items indexed", theme.header_dim_style()),
    ]);

    let header = ratatui::widgets::Paragraph::new(line).style(
        ratatui::style::Style::default().bg(theme.mantle),
    );
    frame.render_widget(header, area);
}

// ── Status Bar ───────────────────────────────────────────────────────────────

fn render_status(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let mode_label = state.search_mode_label();
    let result_count = state.results.len();

    let is_drill = state.drill_path.is_some();

    let mut spans = vec![
        // Mode badge
        Span::styled(format!(" {} ", mode_label), theme.status_mode_style()),
        Span::styled(" ", theme.status_style()),
        // Result count
        Span::styled(format!("{}", result_count), theme.status_count_style()),
        Span::styled(" results ", theme.status_style()),
        Span::styled("\u{2502} ", theme.status_style()),  // │
    ];

    // Navigation hints (context-aware)
    if is_drill {
        spans.extend(vec![
            Span::styled("\u{2191}\u{2193}", theme.status_hint_key_style()),   // ↑↓
            Span::styled(" navigate ", theme.status_hint_desc_style()),
            Span::styled("Tab", theme.status_hint_key_style()),
            Span::styled("/", theme.status_hint_desc_style()),
            Span::styled("\u{2192}", theme.status_hint_key_style()),            // →
            Span::styled(" open ", theme.status_hint_desc_style()),
            Span::styled("\u{2190}", theme.status_hint_key_style()),            // ←
            Span::styled("/", theme.status_hint_desc_style()),
            Span::styled("Esc", theme.status_hint_key_style()),
            Span::styled(" back ", theme.status_hint_desc_style()),
            Span::styled("Enter", theme.status_hint_key_style()),
            Span::styled(" launch", theme.status_hint_desc_style()),
        ]);
    } else {
        spans.extend(vec![
            Span::styled("\u{2191}\u{2193}", theme.status_hint_key_style()),
            Span::styled(" navigate ", theme.status_hint_desc_style()),
            Span::styled("Tab", theme.status_hint_key_style()),
            Span::styled(" open folder ", theme.status_hint_desc_style()),
            Span::styled("Enter", theme.status_hint_key_style()),
            Span::styled(" launch ", theme.status_hint_desc_style()),
            Span::styled("Esc", theme.status_hint_key_style()),
            Span::styled(" quit ", theme.status_hint_desc_style()),
        ]);
    }

    // Language indicator on the right
    let lang = if state.hangul_mode { "\u{D55C}" } else { "EN" }; // 한 / EN
    spans.push(Span::styled(
        format!("\u{2502} {} ", lang),  // │
        if state.hangul_mode {
            theme.status_count_style()
        } else {
            theme.status_style()
        },
    ));

    let line = Line::from(spans);
    let status = ratatui::widgets::Paragraph::new(line)
        .style(ratatui::style::Style::default().bg(theme.mantle));
    frame.render_widget(status, area);
}
