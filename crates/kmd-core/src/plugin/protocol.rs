//! Plugin communication protocol — JSON messages over stdin/stdout

use serde::{Deserialize, Serialize};

/// Request sent from kmd to plugin
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginRequest {
    /// Search query
    #[serde(rename = "search")]
    Search { query: String },
    /// Execute action on selected item
    #[serde(rename = "execute")]
    Execute { item_id: String },
}

/// Response from plugin to kmd
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginResponse {
    /// Search results
    #[serde(rename = "results")]
    Results { items: Vec<PluginItem> },
    /// Action result
    #[serde(rename = "action")]
    Action { action: PluginAction },
    /// Error
    #[serde(rename = "error")]
    Error { message: String },
}

/// An item returned by a plugin
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
}

/// An action returned by a plugin after execution
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum PluginAction {
    #[serde(rename = "display")]
    Display { text: String },
    #[serde(rename = "copy")]
    Copy { text: String },
    #[serde(rename = "open_url")]
    OpenUrl { url: String },
    #[serde(rename = "noop")]
    Noop,
}
