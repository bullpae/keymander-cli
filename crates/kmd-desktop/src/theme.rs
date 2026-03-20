//! Desktop theme system — 5 built-in presets.
//!
//! Colors are kept in sync with the CLI TUI theme for brand consistency.

use iced::Color;

// ─── DesktopTheme ─────────────────────────────────────────────────────────────

/// Complete theme definition for the desktop launcher UI.
#[derive(Debug, Clone)]
pub struct DesktopTheme {
    pub name: &'static str,

    // Surfaces
    pub background: Color,
    pub surface: Color,
    pub surface2: Color,
    pub border: Color,

    // Text hierarchy
    pub text: Color,
    pub subtext: Color,
    pub overlay: Color,

    // Accent color
    pub accent: Color,

    // Semantic colors
    pub green: Color,
    pub peach: Color,
    pub yellow: Color,
    pub red: Color,
    pub teal: Color,

    // Effects
    pub opacity: f32,
    pub corner_radius: f32,
    pub shadow_intensity: f32,
}

impl DesktopTheme {
    /// Get the background color with opacity applied.
    pub fn background_with_opacity(&self) -> Color {
        Color {
            a: self.opacity,
            ..self.background
        }
    }

    /// Color for a specific ItemKind badge.
    pub fn kind_color(&self, kind: kmd_core::ItemKind) -> Color {
        match kind {
            kmd_core::ItemKind::App => self.peach,
            kmd_core::ItemKind::Directory => self.green,
            kmd_core::ItemKind::File => self.accent,
            kmd_core::ItemKind::Executable => self.yellow,
            kmd_core::ItemKind::SystemCommand => self.red,
            kmd_core::ItemKind::WebSearch => self.teal,
            kmd_core::ItemKind::Calculator => self.yellow,
            kmd_core::ItemKind::Emoji => self.yellow,
            kmd_core::ItemKind::Shell => self.red,
        }
    }
}

// ─── Built-in Presets ─────────────────────────────────────────────────────────

/// 1. Midnight (Default) — CLI theme lineage, keymander signature
pub fn midnight() -> DesktopTheme {
    DesktopTheme {
        name: "Midnight",
        background: rgb(0x18, 0x18, 0x28),
        surface: rgb(0x2A, 0x2A, 0x40),
        surface2: rgb(0x37, 0x3A, 0x50),
        border: rgb(0x44, 0x48, 0x62),
        text: rgb(0xDC, 0xE4, 0xFF),
        subtext: rgb(0xB4, 0xBE, 0xDC),
        overlay: rgb(0x82, 0x8C, 0xAA),
        accent: rgb(0x56, 0xD2, 0xFF),
        green: rgb(0x50, 0xFA, 0x7B),
        peach: rgb(0xFF, 0xA5, 0x60),
        yellow: rgb(0xFF, 0xE6, 0x78),
        red: rgb(0xFF, 0x6E, 0x8C),
        teal: rgb(0x50, 0xF0, 0xD2),
        opacity: 0.95,
        corner_radius: 12.0,
        shadow_intensity: 1.0,
    }
}

/// 2. Obsidian — OLED black, ultra-minimal
pub fn obsidian() -> DesktopTheme {
    DesktopTheme {
        name: "Obsidian",
        background: rgb(0x00, 0x00, 0x00),
        surface: rgb(0x0A, 0x0A, 0x0A),
        surface2: rgb(0x1A, 0x1A, 0x1A),
        border: rgb(0x2A, 0x2A, 0x2A),
        text: rgb(0xE0, 0xE0, 0xE0),
        subtext: rgb(0x88, 0x88, 0x88),
        overlay: rgb(0x55, 0x55, 0x55),
        accent: rgb(0x7C, 0x5C, 0xFF),
        green: rgb(0x50, 0xFA, 0x7B),
        peach: rgb(0xFF, 0xA5, 0x60),
        yellow: rgb(0xFF, 0xE6, 0x78),
        red: rgb(0xFF, 0x6E, 0x8C),
        teal: rgb(0x50, 0xF0, 0xD2),
        opacity: 0.98,
        corner_radius: 12.0,
        shadow_intensity: 0.8,
    }
}

/// 3. Snow — clean light theme
pub fn snow() -> DesktopTheme {
    DesktopTheme {
        name: "Snow",
        background: rgb(0xFA, 0xFA, 0xFA),
        surface: rgb(0xFF, 0xFF, 0xFF),
        surface2: rgb(0xF0, 0xF0, 0xF8),
        border: rgb(0xE0, 0xE0, 0xE8),
        text: rgb(0x1A, 0x1A, 0x2E),
        subtext: rgb(0x66, 0x66, 0x80),
        overlay: rgb(0x99, 0x99, 0xAA),
        accent: rgb(0x00, 0x66, 0xFF),
        green: rgb(0x00, 0xA8, 0x40),
        peach: rgb(0xE8, 0x6D, 0x00),
        yellow: rgb(0xC4, 0x9A, 0x00),
        red: rgb(0xD0, 0x30, 0x50),
        teal: rgb(0x00, 0x88, 0x80),
        opacity: 0.95,
        corner_radius: 12.0,
        shadow_intensity: 0.5,
    }
}

/// 4. Rose Pine — warm, soft palette
pub fn rose_pine() -> DesktopTheme {
    DesktopTheme {
        name: "Rose Pine",
        background: rgb(0x19, 0x17, 0x24),
        surface: rgb(0x1F, 0x1D, 0x2E),
        surface2: rgb(0x26, 0x23, 0x3A),
        border: rgb(0x39, 0x35, 0x52),
        text: rgb(0xE0, 0xDE, 0xF4),
        subtext: rgb(0x90, 0x8C, 0xAA),
        overlay: rgb(0x6E, 0x6A, 0x86),
        accent: rgb(0xC4, 0xA7, 0xE7),
        green: rgb(0x31, 0x74, 0x8F),
        peach: rgb(0xF6, 0xC1, 0x77),
        yellow: rgb(0xF6, 0xC1, 0x77),
        red: rgb(0xEB, 0x6F, 0x92),
        teal: rgb(0x9C, 0xCF, 0xD8),
        opacity: 0.93,
        corner_radius: 12.0,
        shadow_intensity: 0.9,
    }
}

/// 5. Nord — calm Scandinavian palette
pub fn nord() -> DesktopTheme {
    DesktopTheme {
        name: "Nord",
        background: rgb(0x2E, 0x34, 0x40),
        surface: rgb(0x3B, 0x42, 0x52),
        surface2: rgb(0x43, 0x4C, 0x5E),
        border: rgb(0x4C, 0x56, 0x6A),
        text: rgb(0xEC, 0xEF, 0xF4),
        subtext: rgb(0xD8, 0xDE, 0xE9),
        overlay: rgb(0x7B, 0x88, 0xA1),
        accent: rgb(0x88, 0xC0, 0xD0),
        green: rgb(0xA3, 0xBE, 0x8C),
        peach: rgb(0xD0, 0x87, 0x70),
        yellow: rgb(0xEB, 0xCB, 0x8B),
        red: rgb(0xBF, 0x61, 0x6A),
        teal: rgb(0x8F, 0xBC, 0xBB),
        opacity: 0.95,
        corner_radius: 12.0,
        shadow_intensity: 0.7,
    }
}

/// Resolve a preset name to a theme instance.
pub fn from_name(name: &str) -> DesktopTheme {
    match name.to_lowercase().as_str() {
        "obsidian" => obsidian(),
        "snow" | "light" => snow(),
        "rose_pine" | "rose-pine" | "rosepine" => rose_pine(),
        "nord" => nord(),
        _ => midnight(), // default
    }
}

// ─── Color Helpers ────────────────────────────────────────────────────────────

/// Create an iced Color from RGB bytes.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}
