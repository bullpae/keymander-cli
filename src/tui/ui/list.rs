//! Search results list widget

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::tui::app::AppState;
use crate::tui::theme::Theme;

/// Render the results list
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let items: Vec<ListItem> = state
        .results
        .iter()
        .map(|result| {
            let kind_tag = format!("[{}]", result.item.kind);
            let line = Line::from(vec![
                Span::raw(format!(" {} ", result.item.icon)),
                Span::styled(
                    format!("{:<30}", truncate(&result.item.name, 30)),
                    theme.list_normal_style(),
                ),
                Span::styled(
                    format!(" {:<8}", kind_tag),
                    theme.kind_tag_style(),
                ),
                Span::styled(
                    format!(" {}", truncate(&result.item.path, 40)),
                    theme.path_style(),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Results "),
        )
        .highlight_style(theme.list_selected_style())
        .highlight_symbol("▸ ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_index));

    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Truncate a string to fit within max_len, adding "..." if needed
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
