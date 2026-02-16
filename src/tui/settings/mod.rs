//! Settings modal — TUI configuration panel (F2)
//!
//! Full-screen modal overlay with tabbed settings categories.

pub mod items;
pub mod render;

use crossterm::event::{KeyCode, KeyModifiers};

// ── State ────────────────────────────────────────────────────────────────────

/// Settings modal state
pub struct SettingsState {
    /// Currently selected tab
    pub active_tab: SettingsTab,
    /// Selected item index within current tab
    pub selected_item: usize,
    /// Whether currently editing a value
    pub editing: bool,
    /// Edit buffer for text/number input
    pub edit_buffer: String,
    /// The live config being edited (clone of original)
    pub config: kmd_core::Config,
    /// Whether changes were made
    pub dirty: bool,
}

impl SettingsState {
    /// Create a new settings state from the current config
    pub fn new(config: kmd_core::Config) -> Self {
        Self {
            active_tab: SettingsTab::Priority,
            selected_item: 0,
            editing: false,
            edit_buffer: String::new(),
            config,
            dirty: false,
        }
    }

    /// Get the number of items in the current tab
    pub fn tab_item_count(&self) -> usize {
        items::items_for_tab(&self.active_tab).len()
    }
}

/// Settings tab categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Priority,
    SearchTool,
    SearchPaths,
    IgnorePatterns,
    Display,
    Keybindings,
}

impl SettingsTab {
    /// All tabs in order
    pub const ALL: &'static [SettingsTab] = &[
        SettingsTab::Priority,
        SettingsTab::SearchTool,
        SettingsTab::SearchPaths,
        SettingsTab::IgnorePatterns,
        SettingsTab::Display,
        SettingsTab::Keybindings,
    ];

    pub fn label(&self) -> &str {
        match self {
            Self::Priority => "Priority",
            Self::SearchTool => "Search",
            Self::SearchPaths => "Paths",
            Self::IgnorePatterns => "Ignore",
            Self::Display => "Display",
            Self::Keybindings => "Keys",
        }
    }

    /// Move to the next tab
    pub fn next(&self) -> Self {
        let idx = Self::ALL.iter().position(|t| t == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    /// Move to the previous tab
    pub fn prev(&self) -> Self {
        let idx = Self::ALL.iter().position(|t| t == self).unwrap_or(0);
        if idx == 0 {
            Self::ALL[Self::ALL.len() - 1]
        } else {
            Self::ALL[idx - 1]
        }
    }
}

// ── Key Handling ─────────────────────────────────────────────────────────────

/// Result of handling a settings key event
pub enum SettingsAction {
    /// Nothing happened, stay in settings
    None,
    /// Close the settings modal
    Close,
    /// Save config and optionally rebuild index
    Save { needs_rebuild: bool },
    /// Reset to defaults
    Reset,
}

/// Handle a key event while settings modal is open
pub fn handle_settings_key(
    state: &mut SettingsState,
    key: crossterm::event::KeyEvent,
) -> SettingsAction {
    // If editing a text field, handle edit-mode keys
    if state.editing {
        return handle_edit_key(state, key);
    }

    match (key.code, key.modifiers) {
        // Close modal
        (KeyCode::Esc, _) | (KeyCode::F(2), _) => SettingsAction::Close,

        // Tab navigation
        (KeyCode::Right, _) | (KeyCode::Tab, _) => {
            state.active_tab = state.active_tab.next();
            state.selected_item = 0;
            SettingsAction::None
        }
        (KeyCode::Left, _) | (KeyCode::BackTab, _) => {
            state.active_tab = state.active_tab.prev();
            state.selected_item = 0;
            SettingsAction::None
        }

        // Item navigation
        (KeyCode::Up, _) => {
            if state.selected_item > 0 {
                state.selected_item -= 1;
            }
            SettingsAction::None
        }
        (KeyCode::Down, _) => {
            let max = state.tab_item_count().saturating_sub(1);
            if state.selected_item < max {
                state.selected_item += 1;
            }
            SettingsAction::None
        }

        // Enter editing mode or toggle bool
        (KeyCode::Enter, _) => {
            handle_enter(state);
            SettingsAction::None
        }

        // Slider: adjust numeric values with +/-
        (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
            adjust_current_value(state, 10);
            SettingsAction::None
        }
        (KeyCode::Char('-'), _) => {
            adjust_current_value(state, -10);
            SettingsAction::None
        }

        // Save
        (KeyCode::Char('s'), _) | (KeyCode::Char('S'), _) => {
            let needs_rebuild = state.dirty;
            SettingsAction::Save { needs_rebuild }
        }

        // Reset to defaults
        (KeyCode::Char('r'), _) | (KeyCode::Char('R'), _) => SettingsAction::Reset,

        // Add item (for list tabs)
        (KeyCode::Char('a'), _) | (KeyCode::Char('A'), _) => {
            handle_add_item(state);
            SettingsAction::None
        }

        // Delete item (for list tabs)
        (KeyCode::Char('d'), _) | (KeyCode::Char('D'), _) => {
            handle_delete_item(state);
            SettingsAction::None
        }

        _ => SettingsAction::None,
    }
}

/// Handle key events in edit mode
fn handle_edit_key(state: &mut SettingsState, key: crossterm::event::KeyEvent) -> SettingsAction {
    match key.code {
        KeyCode::Esc => {
            // Cancel edit
            state.editing = false;
            state.edit_buffer.clear();
            SettingsAction::None
        }
        KeyCode::Enter => {
            // Commit edit
            let items = items::items_for_tab(&state.active_tab);
            if let Some(item) = items.get(state.selected_item) {
                let value = state.edit_buffer.clone();
                match item.widget {
                    items::WidgetKind::Text | items::WidgetKind::Number => {
                        let _ = state.config.set_value(item.key, &value);
                        state.dirty = true;
                    }
                    _ => {}
                }
            }
            state.editing = false;
            state.edit_buffer.clear();
            SettingsAction::None
        }
        KeyCode::Backspace => {
            state.edit_buffer.pop();
            SettingsAction::None
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT)
            {
                return SettingsAction::None;
            }
            state.edit_buffer.push(c);
            SettingsAction::None
        }
        _ => SettingsAction::None,
    }
}

/// Handle Enter on the currently selected item
fn handle_enter(state: &mut SettingsState) {
    let items = items::items_for_tab(&state.active_tab);
    let Some(item) = items.get(state.selected_item) else {
        return;
    };

    match item.widget {
        items::WidgetKind::Toggle => {
            // Toggle boolean value
            if let Some(current) = state.config.get_value(item.key) {
                let new_val = if current == "true" { "false" } else { "true" };
                let _ = state.config.set_value(item.key, new_val);
                state.dirty = true;
            }
        }
        items::WidgetKind::Text | items::WidgetKind::Number => {
            // Enter edit mode
            state.edit_buffer = state.config.get_value(item.key).unwrap_or_default();
            state.editing = true;
        }
        items::WidgetKind::Slider => {
            // For sliders, Enter also enters edit mode for direct number input
            state.edit_buffer = state.config.get_value(item.key).unwrap_or_default();
            state.editing = true;
        }
        items::WidgetKind::Select(options) => {
            // Cycle to next option
            if let Some(current) = state.config.get_value(item.key) {
                let idx = options.iter().position(|o| *o == current).unwrap_or(0);
                let next_idx = (idx + 1) % options.len();
                let _ = state.config.set_value(item.key, options[next_idx]);
                state.dirty = true;
            }
        }
        items::WidgetKind::ListAdd => {
            // Enter edit mode for new item
            state.edit_buffer.clear();
            state.editing = true;
        }
        items::WidgetKind::ReadOnly => {
            // Not editable — do nothing
        }
    }
}

/// Adjust the current numeric/slider value by delta
fn adjust_current_value(state: &mut SettingsState, delta: i32) {
    let items = items::items_for_tab(&state.active_tab);
    let Some(item) = items.get(state.selected_item) else {
        return;
    };

    if !matches!(
        item.widget,
        items::WidgetKind::Slider | items::WidgetKind::Number
    ) {
        return;
    }

    if let Some(current) = state.config.get_value(item.key) {
        if let Ok(val) = current.parse::<i32>() {
            let new_val = (val + delta).clamp(0, 100);
            let _ = state.config.set_value(item.key, &new_val.to_string());
            state.dirty = true;
        }
    }
}

/// Handle adding an item to a list (search_paths or ignore_patterns)
fn handle_add_item(state: &mut SettingsState) {
    match state.active_tab {
        SettingsTab::SearchPaths | SettingsTab::IgnorePatterns => {
            state.edit_buffer.clear();
            state.editing = true;
        }
        _ => {}
    }
}

/// Remove an item at `idx` from a Vec, update dirty flag, and clamp selection.
fn remove_list_item<T>(vec: &mut Vec<T>, idx: usize, dirty: &mut bool, selected: &mut usize) {
    if idx < vec.len() {
        vec.remove(idx);
        *dirty = true;
        *selected = (*selected).min(vec.len().saturating_sub(1));
    }
}

/// Handle deleting the selected item from a list
fn handle_delete_item(state: &mut SettingsState) {
    let idx = state.selected_item;
    match state.active_tab {
        SettingsTab::SearchPaths => {
            remove_list_item(
                &mut state.config.launcher.search_paths,
                idx,
                &mut state.dirty,
                &mut state.selected_item,
            );
        }
        SettingsTab::IgnorePatterns => {
            remove_list_item(
                &mut state.config.launcher.ignore_patterns,
                idx,
                &mut state.dirty,
                &mut state.selected_item,
            );
        }
        _ => {}
    }
}
