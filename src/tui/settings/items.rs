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
    /// Read-only display (not editable)
    ReadOnly,
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
    "auto",
    "builtin",
    "fd",
    "everything",
    "winfs",
    "mdfind",
    "locate",
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
            SettingItem {
                label: "Multi LLM prefixes",
                key: "launcher.multi_llm_prefixes",
                widget: WidgetKind::Text,
                description: "Comma-separated aliases (e.g. @ll,@llm,@cmp)",
            },
            SettingItem {
                label: "Multi Web prefixes",
                key: "launcher.multi_web_prefixes",
                widget: WidgetKind::Text,
                description: "Comma-separated aliases (e.g. @m,@mw,@msearch)",
            },
            SettingItem {
                label: "Spell providers",
                key: "launcher.spell_providers",
                widget: WidgetKind::Text,
                description: "Comma-separated IDs (naver_spell,pusan_spell)",
            },
            SettingItem {
                label: "Spell prefixes",
                key: "launcher.spell_prefixes",
                widget: WidgetKind::Text,
                description: "Comma-separated aliases (e.g. @sp,@spell)",
            },
            SettingItem {
                label: "Translate providers",
                key: "launcher.translate_providers",
                widget: WidgetKind::Text,
                description: "Comma-separated IDs (google_translate,papago,deepl)",
            },
            SettingItem {
                label: "Translate prefixes",
                key: "launcher.translate_prefixes",
                widget: WidgetKind::Text,
                description: "Comma-separated aliases (e.g. @tr,@trko,@tren)",
            },
            SettingItem {
                label: "Keymap backend",
                key: "launcher.keymap.backend",
                widget: WidgetKind::Text,
                description: "Prototype backend type (default: kanata)",
            },
            SettingItem {
                label: "Kanata path",
                key: "launcher.keymap.kanata_path",
                widget: WidgetKind::Text,
                description: "Absolute path to kanata binary (empty = PATH)",
            },
            SettingItem {
                label: "Keymap profile dir",
                key: "launcher.keymap.profile_dir",
                widget: WidgetKind::Text,
                description: "Directory containing .kbd profile files",
            },
            SettingItem {
                label: "Active keymap profile",
                key: "launcher.keymap.active_profile",
                widget: WidgetKind::Text,
                description: "Profile file name used by keymap command",
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
                label: "Mode",
                key: "_portable_mode",
                widget: WidgetKind::ReadOnly,
                description: "Portable: data next to exe. System: OS standard paths. Use 'kmd portable enable/disable' to switch.",
            },
            SettingItem {
                label: "Data path",
                key: "_data_path",
                widget: WidgetKind::ReadOnly,
                description: "Where config, database, and index are stored",
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 설정 화면의 모든 항목이 `Config`의 get/set과 실제로 연결돼 있는지.
    ///
    /// 설정 키는 네 곳(구조체 · `get_value` · `set_value` · 이 파일)에 손으로
    /// 열거되는데 이를 잇는 컴파일 타임 장치가 없다. 실제로
    /// `launcher.everything_path`가 이 목록에만 있고 `Config` 양쪽에 없어서,
    /// "Everything path" 필드가 **항상 빈칸으로 보이고 입력해도 조용히 버려지는**
    /// 버그가 있었다 (2026-08-27 발견). 호출부가 `let _ = set_value(..)`로
    /// 오류를 삼켜 UI상으로는 정상처럼 보였다.
    #[test]
    fn 모든_설정_항목_키가_config에_연결돼_있다() {
        let mut config = kmd_core::Config::default();

        for tab in SettingsTab::ALL {
            for item in items_for_tab(tab) {
                if matches!(item.widget, WidgetKind::ListAdd) {
                    assert!(
                        item.key.is_empty(),
                        "ListAdd는 목록 추가 마커라 config 키가 없어야 한다: {}",
                        item.label
                    );
                    continue;
                }

                let current = config.get_value(item.key).unwrap_or_else(|| {
                    panic!(
                        "'{}' 항목의 키 '{}'를 Config::get_value가 모른다 — \
                         화면에는 항상 빈 값이 표시된다",
                        item.label, item.key
                    )
                });

                // ReadOnly는 표시 전용 (_portable_mode / _data_path)
                if matches!(item.widget, WidgetKind::ReadOnly) {
                    continue;
                }

                // 읽은 값을 그대로 되쓰는 왕복 — 실패하면 편집이 조용히 버려진다
                config.set_value(item.key, &current).unwrap_or_else(|e| {
                    panic!(
                        "'{}' 항목의 키 '{}'를 Config::set_value가 거부한다 ({e}) — \
                         편집해도 저장되지 않는다",
                        item.label, item.key
                    )
                });
            }
        }
    }
}
