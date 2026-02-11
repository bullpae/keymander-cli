//! Search results list widget

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use unicode_width::UnicodeWidthStr;

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
                    pad_display(&truncate(&result.item.name, 30), 30),
                    theme.list_normal_style(),
                ),
                Span::styled(
                    format!(" {}", pad_display(&kind_tag, 8)),
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

    let title = if let Some(ref drill_path) = state.drill_path {
        format!(" \u{1F4C2} {} ", drill_path.display())
    } else if state.query.is_empty() && state.drill_path.is_none() && !state.results.is_empty() {
        " \u{1F552} Recent ".to_string()
    } else {
        " Results ".to_string()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title),
        )
        .highlight_style(theme.list_selected_style())
        .highlight_symbol("▸ ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_index));

    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Truncate a string to fit within max_width display columns, adding "..." if needed.
/// Uses unicode display width so CJK characters (2 columns each) are handled correctly.
fn truncate(s: &str, max_width: usize) -> String {
    let width = UnicodeWidthStr::width(s);
    if width <= max_width {
        s.to_string()
    } else {
        let suffix = "...";
        let target = max_width.saturating_sub(suffix.len());
        let mut current_width = 0;
        let mut end = 0;
        for (i, ch) in s.char_indices() {
            let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_width + cw > target {
                break;
            }
            current_width += cw;
            end = i + ch.len_utf8();
        }
        format!("{}{}", &s[..end], suffix)
    }
}

/// Pad a string to exactly `width` display columns using unicode width.
fn pad_display(s: &str, width: usize) -> String {
    let current = UnicodeWidthStr::width(s);
    if current >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - current))
    }
}
