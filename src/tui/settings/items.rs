//! Setting item definitions per tab

use super::SettingsTab;

/// Widget kind for a setting item
pub enum WidgetKind {
    /// Boolean toggle (Enter to flip)
    Toggle,
    /// Numeric slider (0-100, +/- to adjust)
    Slider,
    /// Numeric input (Enter to edit)
    Number,
    /// Free text input (Enter to edit)
    Text,
    /// Cycle through fixed options (Enter to cycle)
    Select(&'static [&'static str]),
    /// Add new item to a list (used as marker)
    ListAdd,
}

/// A single setting item displayed in the modal
pub struct SettingItem {
    /// Display label
    pub label: &'static str,
    /// Config key path (dot-separated)
    pub key: &'static str,
    /// Widget type
    pub widget: WidgetKind,
    /// Description shown in help area
    pub description: &'static str,
}

/// File search provider options
const PROVIDER_OPTIONS: &[&str] = &[
    "auto", "builtin", "fd", "everything", "winfs", "mdfind", "locate",
];

/// Get the setting items for a given tab
pub fn items_for_tab(tab: &SettingsTab) -> Vec<SettingItem> {
    match tab {
        SettingsTab::Priority => vec![
            SettingItem {
                label: "Directory (folder)",
                key: "launcher.kind_weights.directory",
                widget: WidgetKind::Slider,
                description: "Priority weight for folders in search results (0-100)",
            },
            SettingItem {
                label: "Application",
                key: "launcher.kind_weights.app",
                widget: WidgetKind::Slider,
                description: "Priority weight for applications (0-100)",
            },
            SettingItem {
                label: "File",
                key: "launcher.kind_weights.file",
                widget: WidgetKind::Slider,
                description: "Priority weight for files (0-100)",
            },
            SettingItem {
                label: "Executable",
                key: "launcher.kind_weights.executable",
                widget: WidgetKind::Slider,
                description: "Priority weight for PATH executables (0-100)",
            },
            SettingItem {
                label: "System Command",
                key: "launcher.kind_weights.system_cmd",
                widget: WidgetKind::Slider,
                description: "Priority weight for system commands (0-100)",
            },
            SettingItem {
                label: "Web Search",
                key: "launcher.kind_weights.web_search",
                widget: WidgetKind::Slider,
                description: "Priority weight for web search results (0-100)",
            },
        ],
        SettingsTab::SearchTool => vec![
            SettingItem {
                label: "Search provider",
                key: "launcher.file_search_provider",
                widget: WidgetKind::Select(PROVIDER_OPTIONS),
                description: "File search backend (auto = best available)",
            },
            SettingItem {
                label: "Everything path",
                key: "launcher.everything_path",
                widget: WidgetKind::Text,
                description: "Path to Everything CLI (es.exe), empty = auto-detect",
            },
            SettingItem {
                label: "Max results",
                key: "launcher.max_results",
                widget: WidgetKind::Number,
                description: "Maximum indexed files (default: 10000)",
            },
            SettingItem {
                label: "Search depth",
                key: "launcher.search_depth",
                widget: WidgetKind::Number,
                description: "Max recursive directory depth (default: 6)",
            },
            SettingItem {
                label: "Index directories",
                key: "launcher.index_directories",
                widget: WidgetKind::Toggle,
                description: "Include folders in search index",
            },
            SettingItem {
                label: "Auto-scan drives",
                key: "launcher.scan_drives",
                widget: WidgetKind::Toggle,
                description: "Auto-discover and scan available drive roots (C:\\, D:\\, etc.)",
            },
            SettingItem {
                label: "Drive scan depth",
                key: "launcher.drive_scan_depth",
                widget: WidgetKind::Number,
                description: "Max depth when scanning drive roots (default: 3, shallow to skip system dirs)",
            },
        ],
        SettingsTab::SearchPaths => {
            // Dynamic list — items are the actual paths
            vec![SettingItem {
                label: "[A] Add path  [D] Delete  [Enter] Edit",
                key: "",
                widget: WidgetKind::ListAdd,
                description: "Directories to scan for files",
            }]
        }
        SettingsTab::IgnorePatterns => {
            // Dynamic list — items are the actual patterns
            vec![SettingItem {
                label: "[A] Add pattern  [D] Delete  [Enter] Edit",
                key: "",
                widget: WidgetKind::ListAdd,
                description: "Directory/file patterns to exclude from indexing",
            }]
        }
        SettingsTab::Display => vec![
            SettingItem {
                label: "Show preview",
                key: "general.show_preview",
                widget: WidgetKind::Toggle,
                description: "Show the preview panel on the right",
            },
            SettingItem {
                label: "Preview width %",
                key: "general.preview_width_percent",
                widget: WidgetKind::Number,
                description: "Preview panel width (10-80)",
            },
            SettingItem {
                label: "Render FPS",
                key: "general.render_fps",
                widget: WidgetKind::Number,
                description: "TUI refresh rate (default: 30)",
            },
            SettingItem {
                label: "Quit on launch",
                key: "launcher.quit_on_launch",
                widget: WidgetKind::Toggle,
                description: "Exit kmd after launching a program",
            },
        ],
        SettingsTab::Keybindings => vec![
            SettingItem {
                label: "Global hotkey",
                key: "keybindings.global_hotkey",
                widget: WidgetKind::Text,
                description: "Hotkey to summon kmd (daemon mode)",
            },
            SettingItem {
                label: "Quit",
                key: "keybindings.quit",
                widget: WidgetKind::Text,
                description: "Key to quit the launcher",
            },
            SettingItem {
                label: "Next",
                key: "keybindings.next",
                widget: WidgetKind::Text,
                description: "Key to move down in results",
            },
            SettingItem {
                label: "Previous",
                key: "keybindings.prev",
                widget: WidgetKind::Text,
                description: "Key to move up in results",
            },
            SettingItem {
                label: "Select",
                key: "keybindings.select",
                widget: WidgetKind::Text,
                description: "Key to execute selected item",
            },
            SettingItem {
                label: "Toggle preview",
                key: "keybindings.toggle_preview",
                widget: WidgetKind::Text,
                description: "Key to show/hide preview panel",
            },
        ],
    }
}
