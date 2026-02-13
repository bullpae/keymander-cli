//! keymander (kmd) — CLI-first cross-platform keyboard launcher
//!
//! Usage:
//!   kmd              → Launch TUI interactive mode
//!   kmd search <q>   → Search from CLI
//!   kmd launch <item>→ Launch a program/file
//!   kmd index        → Manage search index
//!   kmd config       → Manage configuration
//!   kmd history      → View/clear launch history
//!   kmd emoji <q>    → Search and copy Unicode emoji
//!   kmd portable     → Manage portable mode

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
    /// Search and copy Unicode emoji
    Emoji {
        /// Search query (e.g. "fire", "heart", "smile")
        query: Option<String>,
        /// Only list results, don't copy to clipboard
        #[arg(short, long)]
        list: bool,
        /// Output results as JSON
        #[arg(short, long)]
        json: bool,
        /// Copy the Nth result to clipboard (default: 1)
        #[arg(short, long)]
        copy: Option<usize>,
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
    // ── 0. DPI awareness (Windows only) ──────────────────────────────────
    // Must be set before any GetSystemMetrics / GetWindowRect calls so
    // coordinates are in real (physical) pixels, not DPI-scaled.
    #[cfg(windows)]
    win_console::set_dpi_aware();

    // ── 1. Hide console for toggle-off path (Windows only) ───────────────
    // If launched from a shortcut/hotkey, we own the console window.
    // Hide it immediately so the toggle-off path is invisible.
    // If launched from cmd.exe/powershell, we share their console — don't hide.
    #[cfg(windows)]
    let owns_console = win_console::is_sole_console_owner();
    #[cfg(windows)]
    if owns_console {
        win_console::hide();
    }

    // ── 2. Single-instance check ─────────────────────────────────────────
    let data_dir = kmd_core::Config::default_data_dir();
    let instance_guard =
        match kmd_core::single_instance::acquire_or_toggle(&data_dir) {
            kmd_core::single_instance::InstanceAction::Acquired(guard) => Some(guard),
            kmd_core::single_instance::InstanceAction::SignalledExisting => {
                // Existing instance was told to quit — exit with hidden console.
                return Ok(());
            }
        };

    // ── 3. Show console centred + set up UTF-8 / VT processing ─────────
    // Use SetWindowPlacement to atomically show AND position the window.
    // This overrides conhost's async cascade placement reliably.
    #[cfg(windows)]
    if owns_console {
        win_console::show_centered();
    }
    #[cfg(windows)]
    win_console::setup();

    // ── 4. Normal startup ────────────────────────────────────────────────
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
        Some(Commands::Emoji { query, list, json, copy }) => {
            cmd::emoji::run(query.as_deref().unwrap_or(""), list, json, copy)?;
        }
        // No subcommand → launch TUI (pass instance guard so event loop can check it)
        None => {
            #[cfg(windows)]
            {
                tui::run(instance_guard, owns_console)?;
            }
            #[cfg(not(windows))]
            {
                tui::run(instance_guard, false)?;
            }
        }
    }

    Ok(())
}

// ── Windows console management ───────────────────────────────────────────────
//
// Strategy: normal CONSOLE subsystem (no windows_subsystem = "windows").
//
// When launched from a **shortcut / hotkey**, the OS creates a console window.
// We immediately hide it, do the single-instance check, and only show it if
// we actually need to render the TUI.  This keeps the toggle-off path nearly
// invisible (just a 1-2 frame flash at worst — set shortcut "Run: Minimized"
// to eliminate even that).
//
// When launched from **cmd.exe / PowerShell**, we share the parent terminal.
// No window is created or hidden.
//
// This avoids AllocConsole() which creates a bare-bones conhost with broken
// alternate-screen-buffer support.

#[cfg(windows)]
mod win_console {
    use std::ffi::c_void;

    // ── Win32 constants ──────────────────────────────────────────────────
    const SW_HIDE: i32 = 0;
    const SW_SHOWNORMAL: u32 = 1;
    const SW_SHOW: i32 = 5;
    const STD_OUTPUT_HANDLE: u32 = (-11i32) as u32;
    const CP_UTF8: u32 = 65001;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;
    const PROCESS_PER_MONITOR_DPI_AWARE: u32 = 2;

    // ── Win32 structs ────────────────────────────────────────────────────
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    /// WINDOWPLACEMENT — the authoritative way to control a window's
    /// normal/minimized/maximized position.  Unlike SetWindowPos, the
    /// window manager honours this even during async layout operations
    /// (like the conhost cascade placement).
    #[repr(C)]
    struct WindowPlacement {
        length: u32,
        flags: u32,
        show_cmd: u32,
        pt_min_position: Point,
        pt_max_position: Point,
        rc_normal_position: Rect,
    }

    // ── Public API ───────────────────────────────────────────────────────

    /// Declare DPI awareness so GetSystemMetrics / GetWindowRect use
    /// consistent (real-pixel) coordinates regardless of display scaling.
    /// Call this at the very start of the process.
    pub fn set_dpi_aware() {
        // SAFETY: both functions are safe to call once at startup.
        unsafe {
            let result = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
            if result != 0 {
                SetProcessDPIAware();
            }
        }
    }

    /// Check if we are the only process on this console.
    /// If true, the OS created the console for us (hotkey/shortcut launch).
    /// If false, we share a parent terminal (cmd.exe, powershell, etc.).
    pub fn is_sole_console_owner() -> bool {
        let mut pids = [0u32; 16];
        let count = unsafe { GetConsoleProcessList(pids.as_mut_ptr(), 16) };
        count <= 1
    }

    /// Hide the console window immediately (minimize toggle-off flash).
    pub fn hide() {
        unsafe {
            let hwnd = GetConsoleWindow();
            if !hwnd.is_null() {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    /// Show the console window.
    #[allow(dead_code)]
    pub fn show() {
        unsafe {
            let hwnd = GetConsoleWindow();
            if !hwnd.is_null() {
                ShowWindow(hwnd, SW_SHOW);
            }
        }
    }

    /// Show the console window at the exact centre of the primary monitor
    /// using `SetWindowPlacement`.
    ///
    /// Unlike `SetWindowPos`, `SetWindowPlacement` sets the *intended*
    /// normal-position of the window.  The window manager respects this
    /// even when conhost.exe is still applying its cascade placement
    /// asynchronously.
    ///
    /// `rcNormalPosition` uses **workspace coordinates** (screen minus
    /// taskbar), so we use `SystemParametersInfoW(SPI_GETWORKAREA)` to
    /// get the usable area.
    pub fn show_centered() {
        unsafe {
            let hwnd = GetConsoleWindow();
            if hwnd.is_null() {
                return;
            }

            // Get usable work-area (excludes taskbar).
            let mut work = Rect { left: 0, top: 0, right: 0, bottom: 0 };
            SystemParametersInfoW(
                0x0030, // SPI_GETWORKAREA
                0,
                &mut work as *mut Rect as *mut c_void,
                0,
            );
            let area_w = work.right - work.left;
            let area_h = work.bottom - work.top;
            if area_w <= 0 || area_h <= 0 {
                // Fallback: just show normally.
                ShowWindow(hwnd, SW_SHOW);
                return;
            }

            // Get the current placement to learn the window size.
            let mut wp = std::mem::zeroed::<WindowPlacement>();
            wp.length = std::mem::size_of::<WindowPlacement>() as u32;
            if GetWindowPlacement(hwnd, &mut wp) == 0 {
                ShowWindow(hwnd, SW_SHOW);
                return;
            }

            let win_w = wp.rc_normal_position.right - wp.rc_normal_position.left;
            let win_h = wp.rc_normal_position.bottom - wp.rc_normal_position.top;
            if win_w <= 0 || win_h <= 0 {
                ShowWindow(hwnd, SW_SHOW);
                return;
            }

            // Compute centred position in workspace coordinates.
            let x = work.left + (area_w - win_w) / 2;
            let y = work.top + (area_h - win_h) / 2;

            wp.show_cmd = SW_SHOWNORMAL;
            wp.rc_normal_position = Rect {
                left: x,
                top: y,
                right: x + win_w,
                bottom: y + win_h,
            };

            SetWindowPlacement(hwnd, &wp);
        }
    }

    /// Centre the console window on the primary monitor.
    ///
    /// Uses `MoveWindow` which simultaneously sets position **and** size,
    /// sending `WM_WINDOWPOSCHANGED` synchronously.  This is more
    /// reliable than `SetWindowPos` for console windows because conhost
    /// cannot override a `MoveWindow` call the way it can override
    /// `SetWindowPos` with `SWP_NOSIZE`.
    pub fn center() {
        unsafe {
            let hwnd = GetConsoleWindow();
            if hwnd.is_null() {
                return;
            }

            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);
            if screen_w == 0 || screen_h == 0 {
                return;
            }

            let mut rc = Rect { left: 0, top: 0, right: 0, bottom: 0 };
            if GetWindowRect(hwnd, &mut rc) == 0 {
                return;
            }

            let win_w = rc.right - rc.left;
            let win_h = rc.bottom - rc.top;
            if win_w <= 0 || win_h <= 0 {
                return;
            }

            let x = (screen_w - win_w) / 2;
            let y = (screen_h - win_h) / 2;

            // MoveWindow sets position + size and repaints synchronously.
            MoveWindow(hwnd, x, y, win_w, win_h, 1);
        }
    }

    /// Set UTF-8 code page and enable VT processing for ANSI sequences.
    pub fn setup() {
        unsafe {
            SetConsoleOutputCP(CP_UTF8);
            SetConsoleCP(CP_UTF8);

            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if !handle.is_null() && handle as isize != -1 {
                let mut mode: u32 = 0;
                if GetConsoleMode(handle, &mut mode) != 0 {
                    SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                }
            }
        }
    }

    // ── FFI declarations ─────────────────────────────────────────────────
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut c_void;
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
        fn GetStdHandle(std_handle: u32) -> *mut c_void;
        fn GetConsoleMode(handle: *mut c_void, mode: *mut u32) -> i32;
        fn SetConsoleMode(handle: *mut c_void, mode: u32) -> i32;
        fn SetConsoleOutputCP(code_page: u32) -> i32;
        fn SetConsoleCP(code_page: u32) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn ShowWindow(hwnd: *mut c_void, cmd_show: i32) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
        fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
        fn MoveWindow(hwnd: *mut c_void, x: i32, y: i32, w: i32, h: i32, repaint: i32) -> i32;
        fn GetWindowPlacement(hwnd: *mut c_void, wp: *mut WindowPlacement) -> i32;
        fn SetWindowPlacement(hwnd: *mut c_void, wp: *const WindowPlacement) -> i32;
        fn SystemParametersInfoW(action: u32, param: u32, data: *mut c_void, ini: u32) -> i32;
        fn SetProcessDPIAware() -> i32;
    }

    #[link(name = "shcore")]
    unsafe extern "system" {
        fn SetProcessDpiAwareness(value: u32) -> i32;
    }
}
