//! Search results list widget — the main results display

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation,
    ScrollbarState,
};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::tui::app::AppState;
use crate::tui::theme::Theme;

// ── Layout constants ──────────────────────────────────────────────────────────
const ICON_COL_WIDTH: usize = 4; // " XX " — space + 2-char icon + space
const NAME_WIDTH_PCT: usize = 35; // % of inner width for name column
const NAME_MIN: usize = 15;
const NAME_MAX: usize = 40;
const KIND_COL_WIDTH: usize = 8;
const DRILL_PATH_DISPLAY_LEN: usize = 50;

/// Render the results list with scrollbar
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    // Calculate available width for dynamic column sizing
    let inner_width = area.width.saturating_sub(4) as usize; // borders + highlight symbol
    let name_width = (inner_width * NAME_WIDTH_PCT / 100).clamp(NAME_MIN, NAME_MAX);
    let path_width = inner_width.saturating_sub(name_width + KIND_COL_WIDTH + ICON_COL_WIDTH + 2);

    let items: Vec<ListItem> = state
        .results
        .iter()
        .map(|result| {
            let kind_tag = format!("[{}]", result.item.kind);
            let kind_style = kind_style_for(&result.item.kind.to_string(), theme);

            let line = Line::from(vec![
                Span::raw(format!(" {} ", result.item.icon)),
                Span::styled(
                    pad_display(&truncate(&result.item.name, name_width), name_width),
                    theme.list_normal_style(),
                ),
                Span::styled(
                    format!(" {}", pad_display(&kind_tag, KIND_COL_WIDTH)),
                    kind_style,
                ),
                Span::styled(
                    format!(" {}", truncate(&result.item.path, path_width)),
                    theme.path_style(),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    // Dynamic title
    let title = build_title(state, theme);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.list_border_style())
                .title(title),
        )
        .highlight_style(theme.list_selected_style())
        .highlight_symbol("\u{25B8} "); // ▸

    let mut list_state = ListState::default();
    let safe_index = if state.results.is_empty() {
        0
    } else {
        state.selected_index.min(state.results.len() - 1)
    };
    list_state.select(if state.results.is_empty() {
        None
    } else {
        Some(safe_index)
    });

    frame.render_stateful_widget(list, area, &mut list_state);

    // Scrollbar (only if results exceed visible area)
    let visible_rows = area.height.saturating_sub(2) as usize; // minus borders
    if state.results.len() > visible_rows {
        let mut scrollbar_state =
            ScrollbarState::new(state.results.len()).position(state.selected_index);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(theme.scrollbar_style())
            .thumb_style(theme.scrollbar_thumb_style())
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{2502}")); // │

        frame.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

/// Build the title line for the list block
fn build_title<'a>(state: &AppState, theme: &'a Theme) -> Line<'a> {
    if let Some(ref drill_path) = state.drill_path {
        let path_str = drill_path.display().to_string();
        let display = truncate(&path_str, DRILL_PATH_DISPLAY_LEN);
        Line::from(vec![
            Span::styled(" \u{1F4C2} ", theme.list_title_style()), // 📂
            Span::styled(display, theme.list_title_style()),
            Span::raw(" "),
        ])
    } else if state.query.is_empty() && state.drill_path.is_none() && !state.results.is_empty() {
        Line::from(vec![
            Span::styled(" \u{1F4C1} ", theme.list_title_style()), // 📁
            Span::styled("Recent", theme.list_title_style()),
            Span::raw(" "),
        ])
    } else {
        Line::from(vec![Span::styled(" Results ", theme.list_title_style())])
    }
}

/// Get the appropriate kind tag style based on item kind
fn kind_style_for(kind_str: &str, theme: &Theme) -> ratatui::style::Style {
    match kind_str {
        "App" => theme.kind_app_style(),
        "File" => theme.kind_file_style(),
        "Dir" => theme.kind_dir_style(),
        "Exe" => theme.kind_exe_style(),
        "System" => theme.kind_system_style(),
        "Web" => theme.kind_web_style(),
        "Calc" => theme.kind_calc_style(),
        _ => theme.kind_tag_style(),
    }
}

/// Truncate a string to fit within max_width display columns, adding "..." if needed.
/// Uses unicode display width so CJK characters (2 columns each) are handled correctly.
fn truncate(s: &str, max_width: usize) -> String {
    let width = UnicodeWidthStr::width(s);
    if width <= max_width {
        s.to_string()
    } else {
        let suffix = "\u{2026}"; // …
        let target = max_width.saturating_sub(1); // ellipsis is 1 column
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
