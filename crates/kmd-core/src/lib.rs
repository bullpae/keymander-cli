//! kmd-core — keymander core library
//!
//! Provides indexing, search, configuration, database, and action execution
//! for the keymander keyboard launcher.

pub mod action;
pub mod config;
pub mod db;
pub mod hangul;
pub mod history;
pub mod index;
pub mod ipc;
pub mod keymap;
pub mod plugin;
pub mod portable;
pub mod prompt;
pub mod query_prefix;
pub mod search;
pub mod single_instance;
pub mod transform;
pub mod web;

/// Re-export commonly used types
pub use config::Config;
pub use db::Database;
pub use index::{Index, IndexItem, ItemKind, Source};
pub use search::{SearchEngine, SearchMode, SearchResult};

// ── Well-known file names ────────────────────────────────────────────────────
/// Configuration file name.
pub const CONFIG_FILENAME: &str = "config.toml";
/// SQLite database file name.
pub const DB_FILENAME: &str = "kmd.db";
/// Index cache file name (JSON, legacy fallback).
pub const INDEX_CACHE_FILENAME: &str = "index.json";
/// Index cache file name (bincode, fast binary format).
pub const INDEX_CACHE_BIN_FILENAME: &str = "index.bin";
