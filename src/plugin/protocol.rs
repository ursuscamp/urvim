//! MessagePack protocol types for process-backed plugins.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A top-level plugin protocol message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginMessage {
    /// Request expecting a response.
    Request(PluginRequest),
    /// Response to a request.
    Response(PluginResponse),
    /// Fire-and-forget notification.
    Notification(PluginNotification),
}

/// A process plugin request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginRequest {
    /// Request id.
    pub id: u64,
    /// Method name.
    pub method: String,
    /// Optional structured parameters.
    #[serde(default)]
    pub params: Value,
}

/// A process plugin response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginResponse {
    /// Request id being answered.
    pub id: u64,
    /// Successful result payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error message payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A process plugin notification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginNotification {
    /// Method name.
    pub method: String,
    /// Optional structured parameters.
    #[serde(default)]
    pub params: Value,
}

impl PluginRequest {
    /// Creates a request with JSON-like params.
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }
}

impl PluginResponse {
    /// Creates a successful response.
    pub fn success(id: u64, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error response.
    pub fn error(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(error.into()),
        }
    }
}

impl PluginNotification {
    /// Creates a notification with JSON-like params.
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            method: method.into(),
            params,
        }
    }
}
