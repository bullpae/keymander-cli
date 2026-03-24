//! Action execution — launch programs, open files, open URLs

use std::process::Command;

use crate::index::{system_commands, ItemKind};
use crate::search::SearchResult;

/// The result of executing an action
#[derive(Debug)]
pub enum ActionResult {
    /// Successfully launched
    Launched,
    /// Opened URL in browser
    OpenedUrl(String),
    /// Requires user confirmation before executing
    NeedsConfirmation(String),
    /// Error
    Error(String),
}

/// Execute the action for a search result
pub fn execute(result: &SearchResult) -> ActionResult {
    match result.item.kind {
        ItemKind::App | ItemKind::Executable | ItemKind::File | ItemKind::Directory => {
            open_with_system(&result.item.path)
        }
        ItemKind::SystemCommand => execute_system_command(&result.item.name),
        ItemKind::WebSearch => {
            let url_from_keywords = result.item.keywords.split_whitespace().find(|s| {
                s.starts_with("http://") || s.starts_with("https://")
            });
            if let Some(url) = url_from_keywords {
                return open_url(url);
            }
            let path = result.item.path.trim();
            if path.starts_with("http://") || path.starts_with("https://") {
                open_url(path)
            } else {
                ActionResult::Error(format!(
                    "웹 항목에 열 수 있는 URL이 없습니다: {}",
                    result.item.name
                ))
            }
        }
        ItemKind::Calculator => {
            // Calculator results are handled by the TUI (clipboard copy)
            ActionResult::Launched
        }
        ItemKind::Emoji => {
            // Emoji results are handled by the TUI (clipboard copy)
            ActionResult::Launched
        }
        ItemKind::Shell => {
            // Shell commands are handled by the TUI (execute + show output)
            ActionResult::Launched
        }
    }
}

/// Open a file/app using the system's default handler
pub fn open_with_system(path: &str) -> ActionResult {
    match spawn_open(path) {
        Ok(_) => ActionResult::Launched,
        Err(e) => ActionResult::Error(format!("Failed to open '{}': {}", path, e)),
    }
}

/// Open a URL in the default browser
pub fn open_url(url: &str) -> ActionResult {
    match spawn_open(url) {
        Ok(_) => ActionResult::OpenedUrl(url.to_string()),
        Err(e) => ActionResult::Error(format!("Failed to open URL '{}': {}", url, e)),
    }
}

/// Spawn the platform-specific "open" command for a path or URL
fn spawn_open(target: &str) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        // Use a single shell-open path to avoid extra cmd parsing.
        Command::new("explorer.exe")
            .arg(target)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(target).spawn()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Command::new("xdg-open").arg(target).spawn()
    }
}

/// Execute a system command
fn execute_system_command(display_name: &str) -> ActionResult {
    let Some(cmd) = system_commands::find_by_display_name(display_name) else {
        return ActionResult::Error(format!("Unknown system command: {}", display_name));
    };

    if cmd.confirm {
        return ActionResult::NeedsConfirmation(display_name.to_string());
    }

    do_execute_system_command(cmd)
}

/// Actually run a system command (after confirmation if needed)
pub fn do_execute_system_command(cmd: &system_commands::SystemCommand) -> ActionResult {
    let mut command = Command::new(cmd.command);
    command.args(cmd.args);

    // Hide the console window on Windows so it doesn't flash.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    match command.spawn() {
        Ok(_) => ActionResult::Launched,
        Err(e) => ActionResult::Error(format!("Failed to execute '{}': {}", cmd.display_name, e)),
    }
}
