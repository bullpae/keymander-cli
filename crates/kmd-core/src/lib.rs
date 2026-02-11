//! kmd-core — keymander core library
//!
//! Provides indexing, search, configuration, database, and action execution
//! for the keymander keyboard launcher.

pub mod action;
pub mod config;
pub mod db;
pub mod history;
pub mod index;
pub mod plugin;
pub mod search;
pub mod web;

/// Re-export commonly used types
pub use config::Config;
pub use db::Database;
pub use index::{Index, IndexItem, ItemKind, Source};
pub use search::{SearchEngine, SearchMode, SearchResult};
