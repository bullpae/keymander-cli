//! Preview panel widget — shows details of the selected item

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use kmd_core::index::ItemKind;

use crate::tui::app::AppState;
use crate::tui::theme::Theme;

/// Render the preview panel for the selected item
pub fn render(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let title = Line::from(vec![
        Span::styled(" \u{1F50E} ", theme.preview_title_style()), // 🔎
        Span::styled("Preview ", theme.preview_title_style()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.preview_border_style())
        .title(title);

    let lines = if let Some(result) = state.results.get(state.selected_index) {
        let item = &result.item;
        if item.kind == ItemKind::Calculator {
            render_calc_preview(item, state.effective_query(), theme)
        } else if item.kind == ItemKind::Emoji {
            render_emoji_preview(item, theme)
        } else if item.kind == ItemKind::Shell {
            render_shell_preview(item, theme)
        } else {
            render_item_preview(item, result.score, theme)
        }
    } else {
        // Empty state
        vec![
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
                "    \u{2191}\u{2193} to navigate results", // ↑↓
                theme.preview_empty_style(),
            )]),
        ]
    };

    let preview = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(preview, area);
}

/// Render calculator-specific preview
fn render_calc_preview<'a>(
    item: &kmd_core::index::IndexItem,
    query: &str,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    let mut lines = vec![
        Line::from(""),
        // Expression
        Line::from(vec![
            Span::styled("  \u{1F5A9} ", theme.preview_value_style()), // 🖩
            Span::styled("Calculator", theme.kind_calc_style()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Expression",
            theme.preview_label_style(),
        )]),
        Line::from(vec![Span::styled(
            format!("  {}", query.strip_prefix(":calc").unwrap_or(query).trim()),
            theme.preview_value_style(),
        )]),
    ];

    if !item.path.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  Result",
            theme.preview_label_style(),
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", item.path),
            theme.header_accent_style(),
        )]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Press ", theme.preview_dim_style()),
            Span::styled("Enter", theme.kind_calc_style()),
            Span::styled(" to copy result", theme.preview_dim_style()),
        ]));
    }

    lines
}

/// Render emoji-specific preview — large emoji with name and category
fn render_emoji_preview<'a>(item: &kmd_core::index::IndexItem, theme: &'a Theme) -> Vec<Line<'a>> {
    // item.path = emoji char
    // item.name = "😀 grinning face (활짝 웃는 얼굴)" or "😀 grinning face"
    let emoji = &item.path;
    let full_name = item.name.strip_prefix(emoji).unwrap_or(&item.name).trim();

    // Split English name and Korean name if present: "grinning face (활짝 웃는 얼굴)"
    let (en_name, ko_name) = if let Some(paren_start) = full_name.rfind(" (") {
        let en = full_name[..paren_start].trim();
        let ko = full_name[paren_start + 2..].trim_end_matches(')').trim();
        (en, ko)
    } else {
        (full_name, "")
    };

    let mut lines = vec![
        Line::from(""),
        // Large emoji display
        Line::from(vec![Span::styled(
            format!("    {}", emoji),
            theme.header_accent_style(),
        )]),
        Line::from(""),
        // English Name
        Line::from(vec![Span::styled("  Name", theme.preview_label_style())]),
        Line::from(vec![Span::styled(
            format!("  {}", en_name),
            theme.preview_value_style(),
        )]),
    ];

    // Korean name (if available)
    if !ko_name.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  \u{D55C}\u{AD6D}\u{C5B4}", theme.preview_label_style()), // 한국어
        ]));
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", ko_name),
            theme.preview_value_style(),
        )]));
    }

    // Unicode codepoints
    let codepoints: String = emoji
        .chars()
        .map(|c| format!("U+{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ");
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  Codepoint",
        theme.preview_label_style(),
    )]));
    lines.push(Line::from(vec![Span::styled(
        format!("  {}", codepoints),
        theme.preview_dim_style(),
    )]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Press ", theme.preview_dim_style()),
        Span::styled("Enter", theme.kind_calc_style()),
        Span::styled(" to copy emoji", theme.preview_dim_style()),
    ]));

    lines
}

/// Render shell command preview
fn render_shell_preview<'a>(item: &kmd_core::index::IndexItem, theme: &'a Theme) -> Vec<Line<'a>> {
    let is_quick_action = !item.name.contains("Run:");

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  {} ", item.icon), theme.preview_value_style()),
            Span::styled(
                if is_quick_action {
                    "Quick Action"
                } else {
                    "Shell Command"
                },
                theme.kind_calc_style(),
            ),
        ]),
        Line::from(""),
    ];

    if is_quick_action {
        // Quick action: show name and description
        let name = item
            .name
            .strip_prefix(&item.icon)
            .unwrap_or(&item.name)
            .trim();
        lines.push(Line::from(vec![Span::styled(
            "  Action",
            theme.preview_label_style(),
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", name),
            theme.preview_value_style(),
        )]));
        if !item.keywords.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "  Description",
                theme.preview_label_style(),
            )]));
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", item.keywords),
                theme.preview_dim_style(),
            )]));
        }
    } else {
        // Raw command
        lines.push(Line::from(vec![Span::styled(
            "  Command",
            theme.preview_label_style(),
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", item.path),
            theme.preview_value_style(),
        )]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Press ", theme.preview_dim_style()),
        Span::styled("Enter", theme.kind_calc_style()),
        Span::styled(
            if is_quick_action {
                " to execute & copy result"
            } else {
                " to run command & copy output"
            },
            theme.preview_dim_style(),
        ),
    ]));

    lines
}

/// Render standard item preview
fn render_item_preview<'a>(
    item: &kmd_core::index::IndexItem,
    score: u32,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    let mut lines = vec![
        // Name — large and prominent
        Line::from(vec![
            Span::styled(format!("{} ", item.icon), theme.preview_value_style()),
            Span::styled(item.name.clone(), theme.list_title_style()),
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
        Line::from(vec![Span::styled("  Path", theme.preview_label_style())]),
        Line::from(vec![Span::styled(
            format!("  {}", item.path),
            theme.preview_dim_style(),
        )]),
    ];

    // Keywords (if present)
    if !item.keywords.is_empty() && item.keywords != item.path {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  Keywords",
            theme.preview_label_style(),
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", item.keywords),
            theme.preview_dim_style(),
        )]));
    }

    // Score (if nonzero)
    if score > 0 {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Score    ", theme.preview_label_style()),
            Span::styled(format!("{}", score), theme.preview_value_style()),
        ]));
    }

    lines
}
