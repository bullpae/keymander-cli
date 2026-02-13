//! keymander (kmd) — CLI-first cross-platform keyboard launcher
//!
//! Usage:
//!   kmd              → Launch TUI interactive mode
//!   kmd search <q>   → Search from CLI
//!   kmd launch <item>→ Launch a program/file
//!   kmd index        → Manage search index
//!   kmd config       → Manage configuration
//!   kmd history      → View/clear launch history
//!   kmd portable     → Manage portable mode

// ── Windows: no console window in release ────────────────────────────────────
// In release builds the exe starts as a "GUI" app so Windows does NOT allocate
// a console window.  We create one ourselves only when we actually need it
// (TUI mode or CLI subcommands).  This makes the toggle-off path completely
// invisible — no flash at all.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cmd;
mod tui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "kmd",
    version,
    about = "Keyboard-first cross-platform launcher",
    long_about = "키보드 하나로 모든 것을 지휘한다 — CLI-first cross-platform launcher"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for applications, files, and commands
    Search {
        /// Search query
        query: String,
        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },
    /// Launch a program, file, or URL
    Launch {
        /// Item to launch (name or path)
        target: String,
    },
    /// Manage the search index
    Index {
        /// Force rebuild the entire index
        #[arg(long)]
        rebuild: bool,
        /// Show index statistics
        #[arg(long)]
        stats: bool,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// View and manage launch history
    History {
        #[command(subcommand)]
        action: Option<HistoryAction>,
    },
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Global hotkey daemon (start/stop/status)
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Manage portable mode (store all data next to exe)
    Portable {
        #[command(subcommand)]
        action: Option<PortableAction>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get a config value
    Get { key: String },
    /// Set a config value
    Set { key: String, value: String },
    /// Open config file in editor
    Edit,
    /// Show config file path
    Path,
}

#[derive(Subcommand)]
enum HistoryAction {
    /// List recent launches
    List {
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },
    /// Clear all history
    Clear,
}

#[derive(Subcommand)]
enum PluginAction {
    /// List installed plugins
    List,
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon
    Start,
    /// Stop the daemon
    Stop,
    /// Show daemon status
    Status,
}

#[derive(Subcommand)]
enum PortableAction {
    /// Enable portable mode (create kmd-data/ next to exe, migrate data)
    Enable,
    /// Disable portable mode (migrate data to system paths, remove kmd-data/)
    Disable,
}

fn main() -> color_eyre::Result<()> {
    // ── 1. Single-instance check (BEFORE any console/window work) ────────
    // This runs before a console exists (on release Windows builds).
    // Toggle-off path: signal existing instance → exit.  Zero visual artefacts.
    let data_dir = kmd_core::Config::default_data_dir();
    let instance_guard =
        match kmd_core::single_instance::acquire_or_toggle(&data_dir) {
            kmd_core::single_instance::InstanceAction::Acquired(guard) => Some(guard),
            kmd_core::single_instance::InstanceAction::SignalledExisting => {
                // Existing instance was told to quit — we're done.
                return Ok(());
            }
        };

    // ── 2. Ensure we have a console (Windows release builds only) ────────
    #[cfg(all(windows, not(debug_assertions)))]
    ensure_console();

    // ── 3. Normal startup ────────────────────────────────────────────────
    color_eyre::install()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Search { query, limit, json }) => {
            cmd::search::run(&query, limit, json)?;
        }
        Some(Commands::Launch { target }) => {
            cmd::launch::run(&target)?;
        }
        Some(Commands::Index { rebuild, stats }) => {
            cmd::index::run(rebuild, stats)?;
        }
        Some(Commands::Config { action }) => {
            cmd::config::run(action.map(|a| match a {
                ConfigAction::Get { key } => cmd::config::Action::Get(key),
                ConfigAction::Set { key, value } => cmd::config::Action::Set(key, value),
                ConfigAction::Edit => cmd::config::Action::Edit,
                ConfigAction::Path => cmd::config::Action::Path,
            }))?;
        }
        Some(Commands::History { action }) => {
            cmd::history::run(action.map(|a| match a {
                HistoryAction::List { limit, json } => cmd::history::Action::List(limit, json),
                HistoryAction::Clear => cmd::history::Action::Clear,
            }))?;
        }
        Some(Commands::Plugin { action }) => {
            cmd::plugin::run(match action {
                PluginAction::List => cmd::plugin::Action::List,
            })?;
        }
        Some(Commands::Daemon { action }) => {
            cmd::daemon::run(match action {
                DaemonAction::Start => cmd::daemon::Action::Start,
                DaemonAction::Stop => cmd::daemon::Action::Stop,
                DaemonAction::Status => cmd::daemon::Action::Status,
            })?;
        }
        Some(Commands::Portable { action }) => {
            cmd::portable::run(action.map(|a| match a {
                PortableAction::Enable => cmd::portable::Action::Enable,
                PortableAction::Disable => cmd::portable::Action::Disable,
            }))?;
        }
        // No subcommand → launch TUI (pass instance guard so event loop can check it)
        None => {
            tui::run(instance_guard)?;
        }
    }

    Ok(())
}

// ── Windows console management ───────────────────────────────────────────────

/// Attach to the parent terminal (for CLI commands run from cmd/powershell)
/// or allocate a brand-new console (for TUI launched via hotkey).
///
/// Because `windows_subsystem = "windows"` starts with NO standard handles,
/// we must explicitly open CONIN$/CONOUT$ and wire them up via SetStdHandle.
/// Without this, crossterm cannot read keyboard input or write to the screen.
#[cfg(all(windows, not(debug_assertions)))]
fn ensure_console() {
    // Win32 constants
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    const STD_INPUT_HANDLE: u32 = (-10i32) as u32;
    const STD_OUTPUT_HANDLE: u32 = (-11i32) as u32;
    const STD_ERROR_HANDLE: u32 = (-12i32) as u32;
    const GENERIC_READ_WRITE: u32 = 0xC000_0000; // GENERIC_READ | GENERIC_WRITE
    const FILE_SHARE_RW: u32 = 3; // FILE_SHARE_READ | FILE_SHARE_WRITE
    const OPEN_EXISTING: u32 = 3;
    const CP_UTF8: u32 = 65001;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    const INVALID_HANDLE: isize = -1;

    // "CONIN$\0" and "CONOUT$\0" as UTF-16
    const CONIN: [u16; 7] = [b'C' as u16, b'O' as u16, b'N' as u16, b'I' as u16, b'N' as u16, b'$' as u16, 0];
    const CONOUT: [u16; 8] = [b'C' as u16, b'O' as u16, b'N' as u16, b'O' as u16, b'U' as u16, b'T' as u16, b'$' as u16, 0];

    unsafe {
        // 1. Attach to parent console or create a new one
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            AllocConsole();
        }

        // 2. Open console I/O handles directly
        //    (windows_subsystem = "windows" starts with null std handles)
        let conin = CreateFileW(
            CONIN.as_ptr(),
            GENERIC_READ_WRITE,
            FILE_SHARE_RW,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );
        let conout = CreateFileW(
            CONOUT.as_ptr(),
            GENERIC_READ_WRITE,
            FILE_SHARE_RW,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );

        // 3. Wire up standard handles so Rust's std::io and crossterm can use them
        if conin as isize != INVALID_HANDLE {
            SetStdHandle(STD_INPUT_HANDLE, conin);
        }
        if conout as isize != INVALID_HANDLE {
            SetStdHandle(STD_OUTPUT_HANDLE, conout);
            SetStdHandle(STD_ERROR_HANDLE, conout);
        }

        // 4. Set UTF-8 code page for proper Unicode rendering
        SetConsoleOutputCP(CP_UTF8);
        SetConsoleCP(CP_UTF8);

        // 5. Enable VT processing so ANSI escape sequences work
        //    (alternate screen, cursor positioning, colors, etc.)
        if conout as isize != INVALID_HANDLE {
            let mut mode: u32 = 0;
            GetConsoleMode(conout, &mut mode);
            SetConsoleMode(conout, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

#[cfg(all(windows, not(debug_assertions)))]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
    fn AllocConsole() -> i32;
    fn CreateFileW(
        file_name: *const u16,
        desired_access: u32,
        share_mode: u32,
        security_attributes: *const std::ffi::c_void,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn SetStdHandle(std_handle: u32, handle: *mut std::ffi::c_void) -> i32;
    fn SetConsoleOutputCP(code_page: u32) -> i32;
    fn SetConsoleCP(code_page: u32) -> i32;
    fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: *mut std::ffi::c_void, mode: u32) -> i32;
}
