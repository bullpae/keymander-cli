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
            if let Some(url) = result.item.keywords.split_whitespace().find(|s| s.starts_with("http")) {
                open_url(url)
            } else {
                open_url(&result.item.path)
            }
        }
        ItemKind::Calculator => {
            // Calculator results are handled by the TUI (clipboard copy)
            // This branch should not normally be reached
            ActionResult::Launched
        }
    }
}

/// Open a file/app using the system's default handler
pub fn open_with_system(path: &str) -> ActionResult {
    let result = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/c", "start", "", path])
            .spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()
    } else {
        Command::new("xdg-open").arg(path).spawn()
    };

    match result {
        Ok(_) => ActionResult::Launched,
        Err(e) => ActionResult::Error(format!("Failed to open '{}': {}", path, e)),
    }
}

/// Open a URL in the default browser
pub fn open_url(url: &str) -> ActionResult {
    let result = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };

    match result {
        Ok(_) => ActionResult::OpenedUrl(url.to_string()),
        Err(e) => ActionResult::Error(format!("Failed to open URL '{}': {}", url, e)),
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
    let result = Command::new(cmd.command).args(cmd.args).spawn();

    match result {
        Ok(_) => ActionResult::Launched,
        Err(e) => ActionResult::Error(format!(
            "Failed to execute '{}': {}",
            cmd.display_name, e
        )),
    }
}
