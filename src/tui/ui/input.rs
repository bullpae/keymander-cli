//! Search input bar widget

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::tui::app::AppState;
use crate::tui::theme::Theme;

/// Render the search input bar
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    // Mode indicator
    let mode_indicator = if state.hangul_mode { "한> " } else { "> " };

    // Build the input line with composing character highlighted
    let spans = if let Some(composing) = state.composing {
        vec![
            Span::styled(mode_indicator, theme.input_style()),
            Span::styled(state.query.clone(), theme.input_style()),
            Span::styled(
                composing.to_string(),
                Style::default()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]
    } else {
        vec![
            Span::styled(mode_indicator, theme.input_style()),
            Span::styled(state.query.clone(), theme.input_style()),
        ]
    };

    let input_line = Line::from(spans);

    let input = Paragraph::new(input_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.input_border_style())
            .title(" Search "),
    );

    frame.render_widget(input, area);

    // Place cursor after the displayed text
    // Use unicode display width for proper CJK character width handling
    let mode_width = UnicodeWidthStr::width(mode_indicator);
    let query_width = UnicodeWidthStr::width(state.query.as_str());
    let composing_width = state
        .composing
        .map(|c| unicode_width::UnicodeWidthChar::width(c).unwrap_or(1))
        .unwrap_or(0);

    let cursor_x = area.x + 1 + (mode_width + query_width + composing_width) as u16;
    let cursor_y = area.y + 1;
    if cursor_x < area.x + area.width - 1 {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
