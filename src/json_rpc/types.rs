use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The JSON-RPC protocol version used by LSP.
pub const JSON_RPC_VERSION: &str = "2.0";

fn json_rpc_version() -> String {
    JSON_RPC_VERSION.to_string()
}

/// A JSON-RPC request identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// A numeric request id.
    Number(u64),
    /// A string request id.
    String(String),
}

/// A JSON-RPC request message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The protocol version.
    #[serde(default = "json_rpc_version")]
    pub jsonrpc: String,
    /// The request id.
    pub id: RequestId,
    /// The invoked method name.
    pub method: String,
    /// Optional request parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    /// Creates a new JSON-RPC request.
    pub fn new(id: RequestId, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC notification message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    /// The protocol version.
    #[serde(default = "json_rpc_version")]
    pub jsonrpc: String,
    /// The invoked method name.
    pub method: String,
    /// Optional notification parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Notification {
    /// Creates a new JSON-RPC notification.
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC response message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    /// A successful response.
    Success(SuccessResponse),
    /// A failed response.
    Error(ErrorResponse),
}

/// A successful JSON-RPC response payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuccessResponse {
    /// The protocol version.
    #[serde(default = "json_rpc_version")]
    pub jsonrpc: String,
    /// The request id this response answers.
    pub id: RequestId,
    /// The successful result payload.
    pub result: Value,
}

/// A failed JSON-RPC response payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorResponse {
    /// The protocol version.
    #[serde(default = "json_rpc_version")]
    pub jsonrpc: String,
    /// The request id this response answers.
    pub id: RequestId,
    /// The error payload when the request failed.
    pub error: ResponseError,
}

impl Response {
    /// Creates a successful response.
    pub fn success(id: RequestId, result: impl Into<Value>) -> Self {
        Self::Success(SuccessResponse {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: result.into(),
        })
    }

    /// Creates an error response.
    pub fn error(id: RequestId, error: ResponseError) -> Self {
        Self::Error(ErrorResponse {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            error,
        })
    }

    /// Returns the protocol version carried by the response.
    pub fn jsonrpc(&self) -> &str {
        match self {
            Self::Success(response) => &response.jsonrpc,
            Self::Error(response) => &response.jsonrpc,
        }
    }
}

/// A JSON-RPC error payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseError {
    /// The protocol error code.
    pub code: i64,
    /// The human-readable error message.
    pub message: String,
    /// Optional additional error data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ResponseError {
    /// Creates a new JSON-RPC error payload.
    pub fn new(code: i64, message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }
}

/// Any JSON-RPC message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    /// A request message.
    Request(Request),
    /// A response message.
    Response(Response),
    /// A notification message.
    Notification(Notification),
}

impl Message {
    /// Returns the protocol version string carried by this message.
    pub fn jsonrpc(&self) -> &str {
        match self {
            Self::Request(message) => &message.jsonrpc,
            Self::Response(message) => message.jsonrpc(),
            Self::Notification(message) => &message.jsonrpc,
        }
    }
}
