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
    color_eyre::install()?;

    // Initialize logging
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
        // No subcommand → launch TUI
        None => {
            tui::run()?;
        }
    }

    Ok(())
}
