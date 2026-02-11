//! Preview panel widget

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::app::AppState;
use crate::tui::theme::Theme;

/// Render the preview panel for the selected item
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let content = if let Some(result) = state.results.get(state.selected_index) {
        let item = &result.item;
        format!(
            "Name: {}\nType: {}\nPath: {}\nKeywords: {}",
            item.name, item.kind, item.path, item.keywords
        )
    } else {
        "No item selected".to_string()
    };

    let preview = Paragraph::new(content)
        .style(theme.list_normal_style())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Preview "),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(preview, area);
}
