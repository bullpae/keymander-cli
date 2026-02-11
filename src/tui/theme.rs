//! Theme system — color definitions and style builders for the TUI
//!
//! Design language:
//!   - Muted backgrounds, vibrant accents
//!   - Consistent color hierarchy: primary > secondary > muted > dim
//!   - Rounded borders everywhere for a modern feel

use ratatui::style::{Color, Modifier, Style};

// ─── Color Palette ───────────────────────────────────────────────────────────
// Inspired by modern dark themes (One Dark, Catppuccin Mocha, Raycast)

/// Accent — vibrant blue-cyan for primary interactive elements
const ACCENT: Color = Color::Rgb(116, 199, 236);       // #74C7EC — sky blue
/// Accent dim — softer version for secondary accents
const ACCENT_DIM: Color = Color::Rgb(137, 180, 250);    // #89B4FA — lavender blue
/// Green — success, directories, active states
const GREEN: Color = Color::Rgb(166, 227, 161);         // #A6E3A1
/// Yellow — composing state, warnings, kind tags
const YELLOW: Color = Color::Rgb(249, 226, 175);        // #F9E2AF
/// Peach — warm accent for special items
const PEACH: Color = Color::Rgb(250, 179, 135);         // #FAB387
/// Mauve — subtle accent for tags and metadata
const MAUVE: Color = Color::Rgb(203, 166, 247);         // #CBA6F7
/// Red — errors, system commands
const RED: Color = Color::Rgb(243, 139, 168);           // #F38BA8
/// Teal — web/URL items
const TEAL: Color = Color::Rgb(148, 226, 213);          // #94E2D5

/// Text — primary readable text
const TEXT: Color = Color::Rgb(205, 214, 244);           // #CDD6F4
/// Subtext — secondary, less important text
const SUBTEXT: Color = Color::Rgb(166, 173, 200);       // #A6ADC8
/// Overlay — muted text, hints, placeholders
const OVERLAY: Color = Color::Rgb(108, 112, 134);       // #6C7086
/// Surface 2 — lighter surface for selected items
const SURFACE2: Color = Color::Rgb(88, 91, 112);        // #585B70
/// Surface 1 — medium surface for borders
const SURFACE1: Color = Color::Rgb(69, 71, 90);         // #45475A
/// Surface 0 — subtle surface for separators
const SURFACE0: Color = Color::Rgb(49, 50, 68);         // #313244
/// Base — main background (transparent/terminal default for compatibility)
const BASE: Color = Color::Reset;
/// Mantle — slightly darker than base for header/status
const MANTLE: Color = Color::Rgb(24, 24, 37);           // #181825
/// Crust — darkest, for emphasis backgrounds
const CRUST: Color = Color::Rgb(17, 17, 27);            // #11111B

// ─── Theme ───────────────────────────────────────────────────────────────────

/// Theme colors and styles for the entire TUI
#[allow(dead_code)]
pub struct Theme {
    // Semantic colors
    pub accent: Color,
    pub accent_dim: Color,
    pub green: Color,
    pub yellow: Color,
    pub peach: Color,
    pub mauve: Color,
    pub red: Color,
    pub teal: Color,

    // Text hierarchy
    pub text: Color,
    pub subtext: Color,
    pub overlay: Color,

    // Surfaces
    pub surface2: Color,
    pub surface1: Color,
    pub surface0: Color,
    pub base: Color,
    pub mantle: Color,
    pub crust: Color,
}

impl Theme {
    /// Default dark theme (Catppuccin Mocha-inspired)
    pub fn default_theme() -> Self {
        Self {
            accent: ACCENT,
            accent_dim: ACCENT_DIM,
            green: GREEN,
            yellow: YELLOW,
            peach: PEACH,
            mauve: MAUVE,
            red: RED,
            teal: TEAL,
            text: TEXT,
            subtext: SUBTEXT,
            overlay: OVERLAY,
            surface2: SURFACE2,
            surface1: SURFACE1,
            surface0: SURFACE0,
            base: BASE,
            mantle: MANTLE,
            crust: CRUST,
        }
    }

    // ── Header & Status ──────────────────────────────────────────────────

    /// Header background
    pub fn header_style(&self) -> Style {
        Style::default().fg(self.accent).bg(self.mantle)
    }

    /// Header version label (dimmer)
    pub fn header_dim_style(&self) -> Style {
        Style::default().fg(self.overlay).bg(self.mantle)
    }

    /// Header item count (accent)
    pub fn header_accent_style(&self) -> Style {
        Style::default()
            .fg(self.green)
            .bg(self.mantle)
            .add_modifier(Modifier::BOLD)
    }

    /// Status bar background
    pub fn status_style(&self) -> Style {
        Style::default().fg(self.overlay).bg(self.mantle)
    }

    /// Status bar — mode badge
    pub fn status_mode_style(&self) -> Style {
        Style::default()
            .fg(self.mantle)
            .bg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Status bar — result count
    pub fn status_count_style(&self) -> Style {
        Style::default().fg(self.text).bg(self.mantle)
    }

    /// Status bar — key hints
    pub fn status_hint_key_style(&self) -> Style {
        Style::default()
            .fg(self.accent_dim)
            .bg(self.mantle)
            .add_modifier(Modifier::BOLD)
    }

    /// Status bar — hint descriptions
    pub fn status_hint_desc_style(&self) -> Style {
        Style::default().fg(self.overlay).bg(self.mantle)
    }

    // ── Input Bar ────────────────────────────────────────────────────────

    /// Input text
    pub fn input_style(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Input border
    pub fn input_border_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Input title
    pub fn input_title_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Input mode indicator ("> " or "한> ")
    pub fn input_prompt_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Input placeholder (when empty)
    pub fn input_placeholder_style(&self) -> Style {
        Style::default().fg(self.overlay)
    }

    /// Composing character (Korean input in-progress)
    pub fn input_composing_style(&self) -> Style {
        Style::default()
            .fg(self.yellow)
            .add_modifier(Modifier::UNDERLINED)
    }

    // ── Results List ─────────────────────────────────────────────────────

    /// List border
    pub fn list_border_style(&self) -> Style {
        Style::default().fg(self.surface1)
    }

    /// List title
    pub fn list_title_style(&self) -> Style {
        Style::default()
            .fg(self.subtext)
            .add_modifier(Modifier::BOLD)
    }

    /// Normal item — name
    pub fn list_normal_style(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Selected item
    pub fn list_selected_style(&self) -> Style {
        Style::default()
            .fg(self.text)
            .bg(self.surface2)
            .add_modifier(Modifier::BOLD)
    }

    /// Kind tag — colored by kind
    pub fn kind_app_style(&self) -> Style {
        Style::default().fg(self.green)
    }

    pub fn kind_file_style(&self) -> Style {
        Style::default().fg(self.subtext)
    }

    pub fn kind_dir_style(&self) -> Style {
        Style::default().fg(self.yellow)
    }

    pub fn kind_exe_style(&self) -> Style {
        Style::default().fg(self.peach)
    }

    pub fn kind_system_style(&self) -> Style {
        Style::default().fg(self.red)
    }

    pub fn kind_web_style(&self) -> Style {
        Style::default().fg(self.teal)
    }

    pub fn kind_calc_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Fallback kind style
    pub fn kind_tag_style(&self) -> Style {
        Style::default().fg(self.overlay)
    }

    /// Path (secondary info)
    pub fn path_style(&self) -> Style {
        Style::default().fg(self.overlay)
    }

    /// Scrollbar
    pub fn scrollbar_style(&self) -> Style {
        Style::default().fg(self.surface1)
    }

    pub fn scrollbar_thumb_style(&self) -> Style {
        Style::default().fg(self.overlay)
    }

    // ── Preview Panel ────────────────────────────────────────────────────

    /// Preview border
    pub fn preview_border_style(&self) -> Style {
        Style::default().fg(self.surface1)
    }

    /// Preview title
    pub fn preview_title_style(&self) -> Style {
        Style::default()
            .fg(self.subtext)
            .add_modifier(Modifier::BOLD)
    }

    /// Preview labels ("Name:", "Type:", etc.)
    pub fn preview_label_style(&self) -> Style {
        Style::default()
            .fg(self.accent_dim)
            .add_modifier(Modifier::BOLD)
    }

    /// Preview values
    pub fn preview_value_style(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Preview secondary values
    pub fn preview_dim_style(&self) -> Style {
        Style::default().fg(self.overlay)
    }

    /// Preview empty state
    pub fn preview_empty_style(&self) -> Style {
        Style::default().fg(self.surface1)
    }
}
