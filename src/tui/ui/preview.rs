//! Preview panel widget — shows details of the selected item

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

use crate::tui::app::AppState;
use crate::tui::theme::Theme;

/// Render the preview panel for the selected item
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let title = Line::from(vec![
        Span::styled(" \u{1F50E} ", theme.preview_title_style()),  // 🔎
        Span::styled("Preview ", theme.preview_title_style()),
    ]);

    if let Some(result) = state.results.get(state.selected_index) {
        let item = &result.item;

        let mut lines = vec![
            // Name — large and prominent
            Line::from(vec![
                Span::styled(format!("{} ", item.icon), theme.preview_value_style()),
                Span::styled(
                    item.name.clone(),
                    ratatui::style::Style::default()
                        .fg(theme.text)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            // Type
            Line::from(vec![
                Span::styled("  Type     ", theme.preview_label_style()),
                Span::styled(format!("{}", item.kind), theme.preview_value_style()),
            ]),
            // Source
            Line::from(vec![
                Span::styled("  Source   ", theme.preview_label_style()),
                Span::styled(format!("{:?}", item.source), theme.preview_dim_style()),
            ]),
            Line::from(""),
            // Path
            Line::from(vec![
                Span::styled("  Path", theme.preview_label_style()),
            ]),
            Line::from(vec![
                Span::styled(format!("  {}", item.path), theme.preview_dim_style()),
            ]),
        ];

        // Keywords (if present)
        if !item.keywords.is_empty() && item.keywords != item.path {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Keywords", theme.preview_label_style()),
            ]));
            lines.push(Line::from(vec![
                Span::styled(format!("  {}", item.keywords), theme.preview_dim_style()),
            ]));
        }

        // Score (if nonzero)
        if result.score > 0 {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Score    ", theme.preview_label_style()),
                Span::styled(format!("{}", result.score), theme.preview_value_style()),
            ]));
        }

        let preview = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.preview_border_style())
                    .title(title),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(preview, area);
    } else {
        // Empty state
        let empty_lines = vec![
            Line::from(""),
            Line::from(""),
            Line::from(vec![Span::styled(
                "      No item selected",
                theme.preview_empty_style(),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "    Type to search or use",
                theme.preview_empty_style(),
            )]),
            Line::from(vec![Span::styled(
                "    \u{2191}\u{2193} to navigate results",  // ↑↓
                theme.preview_empty_style(),
            )]),
        ];

        let preview = Paragraph::new(empty_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.preview_border_style())
                    .title(title),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(preview, area);
    }
}
