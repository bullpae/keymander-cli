//! Platform-specific system commands (shutdown, restart, lock, etc.)

use super::{IndexItem, ItemKind, Source};

/// System command definition
pub struct SystemCommand {
    pub display_name: &'static str,
    pub keywords: &'static [&'static str],
    /// ASCII icon (legacy terminals)
    pub icon: &'static str,
    /// Emoji icon (modern terminals)
    pub emoji_icon: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub confirm: bool,
}

impl SystemCommand {
    pub fn pick_icon(&self, use_emoji: bool) -> &str {
        if use_emoji {
            self.emoji_icon
        } else {
            self.icon
        }
    }
}

#[cfg(target_os = "windows")]
const SYSTEM_COMMANDS: &[SystemCommand] = &[
    SystemCommand {
        display_name: "Shutdown",
        keywords: &["shutdown", "poweroff", "종료", "시스템종료"],
        icon: "!!",
        emoji_icon: "\u{23FB}", // ⏻
        command: "shutdown",
        args: &["/s", "/t", "0"],
        confirm: true,
    },
    SystemCommand {
        display_name: "Restart",
        keywords: &["restart", "reboot", "재시작"],
        icon: "<>",
        emoji_icon: "\u{1F504}", // 🔄
        command: "shutdown",
        args: &["/r", "/t", "0"],
        confirm: true,
    },
    SystemCommand {
        display_name: "Sleep",
        keywords: &["sleep", "suspend", "절전"],
        icon: "Zz",
        emoji_icon: "\u{1F4A4}", // 💤
        command: "rundll32",
        args: &["powrprof.dll,SetSuspendState", "0,1,0"],
        confirm: false,
    },
    SystemCommand {
        display_name: "Lock Screen",
        keywords: &["lock", "잠금", "lockscreen"],
        icon: "Lk",
        emoji_icon: "\u{1F512}", // 🔒
        command: "rundll32",
        args: &["user32.dll,LockWorkStation"],
        confirm: false,
    },
    SystemCommand {
        display_name: "Logout",
        keywords: &["logout", "logoff", "로그아웃"],
        icon: "->",
        emoji_icon: "\u{1F6AA}", // 🚪
        command: "shutdown",
        args: &["/l"],
        confirm: true,
    },
    SystemCommand {
        display_name: "Settings",
        keywords: &["settings", "설정", "windowssettings"],
        icon: "**",
        emoji_icon: "\u{2699}\u{FE0F}", // ⚙️
        command: "cmd",
        args: &["/c", "start", "ms-settings:"],
        confirm: false,
    },
    SystemCommand {
        display_name: "Task Manager",
        keywords: &["taskmgr", "taskmanager", "작업관리자"],
        icon: "Tm",
        emoji_icon: "\u{1F4CA}", // 📊
        command: "taskmgr",
        args: &[],
        confirm: false,
    },
    SystemCommand {
        display_name: "Recycle Bin",
        keywords: &["trash", "recyclebin", "휴지통"],
        icon: "Rb",
        emoji_icon: "\u{1F5D1}", // 🗑
        command: "explorer",
        args: &["shell:RecycleBinFolder"],
        confirm: false,
    },
];

#[cfg(target_os = "macos")]
const SYSTEM_COMMANDS: &[SystemCommand] = &[
    SystemCommand {
        display_name: "Shutdown",
        keywords: &["shutdown", "poweroff", "종료"],
        icon: "!!",
        emoji_icon: "\u{23FB}",
        command: "osascript",
        args: &["-e", "tell app \"System Events\" to shut down"],
        confirm: true,
    },
    SystemCommand {
        display_name: "Restart",
        keywords: &["restart", "reboot", "재시작"],
        icon: "<>",
        emoji_icon: "\u{1F504}",
        command: "osascript",
        args: &["-e", "tell app \"System Events\" to restart"],
        confirm: true,
    },
    SystemCommand {
        display_name: "Sleep",
        keywords: &["sleep", "suspend", "절전"],
        icon: "Zz",
        emoji_icon: "\u{1F4A4}",
        command: "pmset",
        args: &["sleepnow"],
        confirm: false,
    },
    SystemCommand {
        display_name: "Lock Screen",
        keywords: &["lock", "잠금"],
        icon: "Lk",
        emoji_icon: "\u{1F512}",
        command: "pmset",
        args: &["displaysleepnow"],
        confirm: false,
    },
    SystemCommand {
        display_name: "Logout",
        keywords: &["logout", "로그아웃"],
        icon: "->",
        emoji_icon: "\u{1F6AA}",
        command: "osascript",
        args: &["-e", "tell app \"System Events\" to log out"],
        confirm: true,
    },
];

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const SYSTEM_COMMANDS: &[SystemCommand] = &[
    SystemCommand {
        display_name: "Shutdown",
        keywords: &["shutdown", "poweroff", "종료"],
        icon: "!!",
        emoji_icon: "\u{23FB}",
        command: "systemctl",
        args: &["poweroff"],
        confirm: true,
    },
    SystemCommand {
        display_name: "Restart",
        keywords: &["restart", "reboot", "재시작"],
        icon: "<>",
        emoji_icon: "\u{1F504}",
        command: "systemctl",
        args: &["reboot"],
        confirm: true,
    },
    SystemCommand {
        display_name: "Sleep",
        keywords: &["sleep", "suspend", "절전"],
        icon: "Zz",
        emoji_icon: "\u{1F4A4}",
        command: "systemctl",
        args: &["suspend"],
        confirm: false,
    },
    SystemCommand {
        display_name: "Lock Screen",
        keywords: &["lock", "잠금"],
        icon: "Lk",
        emoji_icon: "\u{1F512}",
        command: "loginctl",
        args: &["lock-session"],
        confirm: false,
    },
    SystemCommand {
        display_name: "Logout",
        keywords: &["logout", "로그아웃"],
        icon: "->",
        emoji_icon: "\u{1F6AA}",
        command: "loginctl",
        args: &["terminate-session", "self"],
        confirm: true,
    },
    SystemCommand {
        display_name: "File Manager",
        keywords: &["files", "filemanager", "파일관리자"],
        icon: ">>",
        emoji_icon: "\u{1F4C2}",
        command: "xdg-open",
        args: &["."],
        confirm: false,
    },
];

/// Collect system commands as IndexItems
pub fn collect_system_commands(use_emoji: bool) -> Vec<IndexItem> {
    SYSTEM_COMMANDS
        .iter()
        .map(|cmd| {
            let keywords_str = cmd.keywords.join(", ");
            IndexItem {
                name: cmd.display_name.to_string(),
                path: cmd.command.to_string(),
                kind: ItemKind::SystemCommand,
                source: Source::SystemCommand,
                icon: cmd.pick_icon(use_emoji).to_string(),
                keywords: keywords_str,
                icon_path: None,
            }
        })
        .collect()
}

/// Find a system command by its display name
pub fn find_by_display_name(name: &str) -> Option<&'static SystemCommand> {
    SYSTEM_COMMANDS.iter().find(|cmd| cmd.display_name == name)
}
