//! Settings modal rendering

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::items::{self, WidgetKind};
use super::{SettingsState, SettingsTab};
use crate::tui::theme::Theme;

/// Render the settings modal overlay
pub fn render_modal(frame: &mut Frame, area: Rect, state: &SettingsState, theme: &Theme) {
    // Center the modal at ~80% of screen
    let modal_area = centered_rect(80, 85, area);

    // Clear the background
    frame.render_widget(Clear, modal_area);

    // Outer block — branded title with two-tone colors
    let dirty_marker = if state.dirty { " *" } else { "" };
    let title_line = Line::from(vec![
        Span::styled(
            " key",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "\u{00BB}",                                          // »
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "mander",
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" Settings (F2){} ", dirty_marker),
            Style::default().fg(theme.subtext),
        ),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(title_line)
        .style(Style::default().bg(theme.mantle));
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    // Layout: tab bar (1) + content (flex) + help bar (2)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Min(3),   // content
            Constraint::Length(2), // help + description
        ])
        .split(inner);

    render_tab_bar(frame, chunks[0], state, theme);
    render_content(frame, chunks[1], state, theme);
    render_help_bar(frame, chunks[2], state, theme);
}

/// Render the tab bar
fn render_tab_bar(frame: &mut Frame, area: Rect, state: &SettingsState, theme: &Theme) {
    let mut spans = Vec::new();
    spans.push(Span::raw(" "));

    for tab in SettingsTab::ALL {
        let is_active = *tab == state.active_tab;
        let label = format!(" {} ", tab.label());

        if is_active {
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(theme.mantle)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                label,
                Style::default().fg(theme.subtext).bg(theme.mantle),
            ));
        }
        spans.push(Span::raw(" "));
    }

    let line = Line::from(spans);
    let tabs = Paragraph::new(line).style(Style::default().bg(theme.mantle));
    frame.render_widget(tabs, area);
}

/// Render the content area for the current tab
fn render_content(frame: &mut Frame, area: Rect, state: &SettingsState, theme: &Theme) {
    match state.active_tab {
        SettingsTab::SearchPaths => render_list_content(
            frame,
            area,
            state,
            theme,
            &state
                .config
                .launcher
                .search_paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
        ),
        SettingsTab::IgnorePatterns => render_list_content(
            frame,
            area,
            state,
            theme,
            &state.config.launcher.ignore_patterns,
        ),
        _ => render_items_content(frame, area, state, theme),
    }
}

/// Render setting items (Priority, SearchTool, Display, Keybindings)
fn render_items_content(
    frame: &mut Frame,
    area: Rect,
    state: &SettingsState,
    theme: &Theme,
) {
    let setting_items = items::items_for_tab(&state.active_tab);
    let mut lines = Vec::new();
    lines.push(Line::raw("")); // top padding

    for (i, item) in setting_items.iter().enumerate() {
        let is_selected = i == state.selected_item;
        let is_editing = is_selected && state.editing;

        let indicator = if is_selected { "\u{25B8} " } else { "  " }; // ▸
        let value_str = if is_editing {
            format!("{}|", state.edit_buffer)
        } else {
            state
                .config
                .get_value(item.key)
                .unwrap_or_else(|| "-".to_string())
        };

        let widget_display = match &item.widget {
            WidgetKind::Slider => {
                let val: u32 = value_str.parse().unwrap_or(0);
                format_slider(val, 100, 20)
            }
            WidgetKind::Toggle => {
                let is_on = value_str == "true";
                if is_on {
                    "\u{25C9} ON ".to_string() // ◉
                } else {
                    "\u{25CB} OFF".to_string() // ○
                }
            }
            WidgetKind::Select(_) => {
                format!("< {} >", value_str)
            }
            WidgetKind::Number | WidgetKind::Text => {
                if is_editing {
                    format!("[{}]", value_str)
                } else {
                    value_str.clone()
                }
            }
            WidgetKind::ListAdd => String::new(),
        };

        let label_style = if is_selected {
            Style::default()
                .fg(theme.text)
                .bg(theme.surface2)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        let value_style = if is_editing {
            Style::default()
                .fg(theme.yellow)
                .bg(theme.surface2)
                .add_modifier(Modifier::UNDERLINED)
        } else if is_selected {
            Style::default().fg(theme.accent).bg(theme.surface2)
        } else {
            Style::default().fg(theme.accent_dim)
        };

        // Pad label to align values
        let label_text = format!("{}{:<24}", indicator, item.label);

        lines.push(Line::from(vec![
            Span::styled(label_text, label_style),
            Span::styled(format!(" {} ", widget_display), value_style),
        ]));
    }

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(theme.mantle))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render list content (search_paths or ignore_patterns)
fn render_list_content(
    frame: &mut Frame,
    area: Rect,
    state: &SettingsState,
    theme: &Theme,
    list_items: &[String],
) {
    let mut lines = Vec::new();
    let is_editing = state.editing;

    if list_items.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "  (empty — press A to add)",
            Style::default().fg(theme.overlay),
        ));
    } else {
        for (i, item_text) in list_items.iter().enumerate() {
            let is_selected = i == state.selected_item;
            let indicator = if is_selected { "\u{25B8} " } else { "  " };

            let style = if is_selected {
                Style::default()
                    .fg(theme.text)
                    .bg(theme.surface2)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.subtext)
            };

            lines.push(Line::styled(
                format!("{}{}", indicator, item_text),
                style,
            ));
        }
    }

    // Show edit buffer if adding
    if is_editing {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("  New: ", Style::default().fg(theme.yellow)),
            Span::styled(
                format!("{}|", state.edit_buffer),
                Style::default()
                    .fg(theme.text)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]));
    }

    // Footer hints for list tabs
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "  [A] Add  [D] Delete  [Enter] Edit selected",
        Style::default().fg(theme.overlay),
    ));

    let paragraph = Paragraph::new(lines)
        .style(Style::default().bg(theme.mantle))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render the help/description bar
fn render_help_bar(frame: &mut Frame, area: Rect, state: &SettingsState, theme: &Theme) {
    let mut lines = Vec::new();

    // Description of selected item
    let items = items::items_for_tab(&state.active_tab);
    if let Some(item) = items.get(state.selected_item) {
        lines.push(Line::styled(
            format!("  {}", item.description),
            Style::default().fg(theme.subtext),
        ));
    } else {
        lines.push(Line::raw(""));
    }

    // Key hints
    let hints = if state.editing {
        vec![
            Span::styled("  Enter", Style::default().fg(theme.accent_dim).add_modifier(Modifier::BOLD)),
            Span::styled(" confirm  ", Style::default().fg(theme.overlay)),
            Span::styled("Esc", Style::default().fg(theme.accent_dim).add_modifier(Modifier::BOLD)),
            Span::styled(" cancel", Style::default().fg(theme.overlay)),
        ]
    } else {
        vec![
            Span::styled("  \u{2190}\u{2192}", Style::default().fg(theme.accent_dim).add_modifier(Modifier::BOLD)),
            Span::styled(" tab  ", Style::default().fg(theme.overlay)),
            Span::styled("\u{2191}\u{2193}", Style::default().fg(theme.accent_dim).add_modifier(Modifier::BOLD)),
            Span::styled(" select  ", Style::default().fg(theme.overlay)),
            Span::styled("+/-", Style::default().fg(theme.accent_dim).add_modifier(Modifier::BOLD)),
            Span::styled(" adjust  ", Style::default().fg(theme.overlay)),
            Span::styled("S", Style::default().fg(theme.green).add_modifier(Modifier::BOLD)),
            Span::styled(" save  ", Style::default().fg(theme.overlay)),
            Span::styled("R", Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD)),
            Span::styled(" reset  ", Style::default().fg(theme.overlay)),
            Span::styled("Esc", Style::default().fg(theme.accent_dim).add_modifier(Modifier::BOLD)),
            Span::styled(" close", Style::default().fg(theme.overlay)),
        ]
    };
    lines.push(Line::from(hints));

    let paragraph = Paragraph::new(lines).style(Style::default().bg(theme.mantle));
    frame.render_widget(paragraph, area);
}

/// Format a slider bar like `[====50====        ]`
fn format_slider(value: u32, max: u32, width: usize) -> String {
    let ratio = (value as f64) / (max as f64);
    let filled = (ratio * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);

    let val_str = value.to_string();
    let bar_filled: String = "\u{2588}".repeat(filled); // █
    let bar_empty: String = "\u{2591}".repeat(empty); // ░

    format!("{}{} {}", bar_filled, bar_empty, val_str)
}

/// Create a centered rectangle with the given percentage width/height
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
