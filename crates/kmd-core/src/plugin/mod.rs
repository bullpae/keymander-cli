//! Plugin system — extension trait and script-based plugin loader
//!
//! Plugins are external scripts/binaries that communicate via JSON over stdin/stdout.
//! Each plugin has a manifest.toml defining its metadata and activation prefix.

pub mod builtin_calc;
pub mod loader;
pub mod protocol;

use crate::index::IndexItem;

/// Extension trait — the interface plugins must implement
pub trait Extension: Send + Sync {
    /// Plugin name
    fn name(&self) -> &str;

    /// Activation prefix (e.g. ":calc", ":todo")
    /// If None, the plugin participates in global search
    fn prefix(&self) -> Option<&str>;

    /// Search within this extension
    fn search(&self, query: &str) -> Vec<IndexItem>;

    /// Execute an action on a selected item
    fn execute(&self, item: &IndexItem) -> ExtensionAction;
}

/// Action returned by extension execution
#[derive(Debug, Clone)]
pub enum ExtensionAction {
    /// Display a result string (e.g. calculator output)
    Display(String),
    /// Copy text to clipboard
    CopyToClipboard(String),
    /// Open a URL
    OpenUrl(String),
    /// Do nothing
    Noop,
}

/// Plugin metadata from manifest.toml
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub prefix: Option<String>,
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    5
}
