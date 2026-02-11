//! Search input bar widget

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::app::AppState;
use crate::tui::theme::Theme;

/// Render the search input bar
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let input_text = format!("> {}", state.query);

    let input = Paragraph::new(input_text)
        .style(theme.input_style())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.input_border_style())
                .title(" Search "),
        );

    frame.render_widget(input, area);

    // Place cursor after the query text
    let cursor_x = area.x + 3 + state.query.len() as u16;
    let cursor_y = area.y + 1;
    if cursor_x < area.x + area.width - 1 {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
