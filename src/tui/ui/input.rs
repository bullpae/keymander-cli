//! Search input bar widget — the primary interaction point

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::tui::app::AppState;
use crate::tui::theme::Theme;

/// Render the search input bar
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let prompt = if state.hangul_mode {
        "\u{D55C}> "  // 한>
    } else {
        "> "
    };

    let effective_query = state.effective_query();
    let is_empty = effective_query.is_empty();

    // Build the input line
    let spans = if is_empty {
        // Show placeholder when empty
        vec![
            Span::styled(prompt, theme.input_prompt_style()),
            Span::styled(
                "Type to search, calculate, @web, @ai...",
                theme.input_placeholder_style(),
            ),
        ]
    } else if let Some(composing) = state.composing {
        vec![
            Span::styled(prompt, theme.input_prompt_style()),
            Span::styled(state.query.clone(), theme.input_style()),
            Span::styled(composing.to_string(), theme.input_composing_style()),
        ]
    } else {
        vec![
            Span::styled(prompt, theme.input_prompt_style()),
            Span::styled(state.query.clone(), theme.input_style()),
        ]
    };

    let input_line = Line::from(spans);

    // Title with icon
    let title = Line::from(vec![
        Span::styled(" \u{1F50D} ", theme.input_title_style()),  // 🔍
        Span::styled("Search ", theme.input_title_style()),
    ]);

    let input = Paragraph::new(input_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.input_border_style())
            .title(title),
    );

    frame.render_widget(input, area);

    // Place cursor
    let mode_width = UnicodeWidthStr::width(prompt);
    let cursor_y = area.y + 1;
    let cursor_x = if !is_empty || state.composing.is_some() {
        let query_width = UnicodeWidthStr::width(state.query.as_str());
        let composing_width = state
            .composing
            .map(|c| unicode_width::UnicodeWidthChar::width(c).unwrap_or(1))
            .unwrap_or(0);
        area.x + 1 + (mode_width + query_width + composing_width) as u16
    } else {
        area.x + 1 + mode_width as u16
    };
    let max_x = area.x + area.width.saturating_sub(1);
    if cursor_x <= max_x {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
