//! Theme system — color definitions for the TUI

use ratatui::style::{Color, Modifier, Style};

/// Theme colors and styles
pub struct Theme {
    pub input_fg: Color,
    pub input_border: Color,
    pub list_selected_bg: Color,
    pub list_selected_fg: Color,
    pub list_normal_fg: Color,
    pub kind_tag_fg: Color,
    pub path_fg: Color,
    pub status_fg: Color,
    pub status_bg: Color,
    pub header_fg: Color,
}

impl Theme {
    /// Default theme
    pub fn default_theme() -> Self {
        Self {
            input_fg: Color::White,
            input_border: Color::Cyan,
            list_selected_bg: Color::DarkGray,
            list_selected_fg: Color::White,
            list_normal_fg: Color::Gray,
            kind_tag_fg: Color::DarkGray,
            path_fg: Color::DarkGray,
            status_fg: Color::DarkGray,
            status_bg: Color::Reset,
            header_fg: Color::Cyan,
        }
    }

    /// Style for the input bar
    pub fn input_style(&self) -> Style {
        Style::default().fg(self.input_fg)
    }

    /// Style for the input border
    pub fn input_border_style(&self) -> Style {
        Style::default().fg(self.input_border)
    }

    /// Style for selected list item
    pub fn list_selected_style(&self) -> Style {
        Style::default()
            .fg(self.list_selected_fg)
            .bg(self.list_selected_bg)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for normal list item
    pub fn list_normal_style(&self) -> Style {
        Style::default().fg(self.list_normal_fg)
    }

    /// Style for item kind tag [App], [File], etc.
    pub fn kind_tag_style(&self) -> Style {
        Style::default().fg(self.kind_tag_fg)
    }

    /// Style for file paths
    pub fn path_style(&self) -> Style {
        Style::default().fg(self.path_fg)
    }

    /// Style for status bar
    pub fn status_style(&self) -> Style {
        Style::default().fg(self.status_fg).bg(self.status_bg)
    }

    /// Style for header
    pub fn header_style(&self) -> Style {
        Style::default()
            .fg(self.header_fg)
            .add_modifier(Modifier::BOLD)
    }
}
