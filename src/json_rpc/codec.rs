use crate::json_rpc::frame::{ContentLengthFrame, FrameError};
use crate::json_rpc::types::{
    ErrorResponse, JSON_RPC_VERSION, Message, Notification, Request, Response, SuccessResponse,
};
use std::fmt;

/// Errors that can occur while encoding or decoding JSON-RPC messages.
#[derive(Debug)]
pub enum JsonRpcError {
    /// A transport I/O error occurred.
    Io(std::io::Error),
    /// A framing error occurred while extracting the payload.
    Frame(FrameError),
    /// The payload could not be parsed as JSON.
    Serde(serde_json::Error),
    /// The message violated a JSON-RPC protocol invariant.
    Protocol(String),
}

impl fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Frame(error) => write!(f, "{error}"),
            Self::Serde(error) => write!(f, "{error}"),
            Self::Protocol(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for JsonRpcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Frame(error) => Some(error),
            Self::Serde(error) => Some(error),
            Self::Protocol(_) => None,
        }
    }
}

impl From<std::io::Error> for JsonRpcError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<FrameError> for JsonRpcError {
    fn from(value: FrameError) -> Self {
        Self::Frame(value)
    }
}

impl From<serde_json::Error> for JsonRpcError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}

/// Encodes a JSON-RPC message into a framed payload.
pub fn encode_message(message: &Message) -> Result<Vec<u8>, JsonRpcError> {
    validate_message(message)?;
    let payload = serde_json::to_vec(message)?;
    Ok(ContentLengthFrame::encode(&payload))
}

/// Decodes a framed JSON-RPC message.
pub fn decode_message(bytes: &[u8]) -> Result<Message, JsonRpcError> {
    let frame = ContentLengthFrame::decode(bytes)?;
    let message = serde_json::from_slice::<Message>(frame.payload)?;
    validate_message(&message)?;
    Ok(message)
}

fn validate_message(message: &Message) -> Result<(), JsonRpcError> {
    if message.jsonrpc() != JSON_RPC_VERSION {
        return Err(JsonRpcError::Protocol(format!(
            "unsupported jsonrpc version: {}",
            message.jsonrpc()
        )));
    }

    match message {
        Message::Request(Request { method, .. })
        | Message::Notification(Notification { method, .. }) => {
            if method.trim().is_empty() {
                return Err(JsonRpcError::Protocol(
                    "method must not be empty".to_string(),
                ));
            }
        }
        Message::Response(Response::Success(SuccessResponse { .. })) => {}
        Message::Response(Response::Error(ErrorResponse { error, .. })) => {
            if error.message.trim().is_empty() {
                return Err(JsonRpcError::Protocol(
                    "response error message must not be empty".to_string(),
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_rpc::types::{Message, Notification, Request, RequestId, Response};
    use serde_json::json;

    #[test]
    fn round_trips_request() {
        let message = Message::Request(Request::new(
            RequestId::Number(1),
            "initialize",
            Some(json!({"rootUri": null})),
        ));

        let bytes = encode_message(&message).expect("encode");
        let decoded = decode_message(&bytes).expect("decode");
        assert_eq!(decoded, message);
    }

    #[test]
    fn round_trips_notification() {
        let message = Message::Notification(Notification::new(
            "textDocument/didOpen",
            Some(json!({"textDocument": {"uri": "file:///tmp/demo.rs"}})),
        ));

        let bytes = encode_message(&message).expect("encode");
        let decoded = decode_message(&bytes).expect("decode");
        assert_eq!(decoded, message);
    }

    #[test]
    fn round_trips_response() {
        let message = Message::Response(Response::success(
            RequestId::String("abc".to_string()),
            json!({"capabilities": {}}),
        ));

        let bytes = encode_message(&message).expect("encode");
        let decoded = decode_message(&bytes).expect("decode");
        assert_eq!(decoded, message);
    }

    #[test]
    fn rejects_empty_method() {
        let message = Message::Notification(Notification::new("", None));

        let error = encode_message(&message).expect_err("protocol");
        match error {
            JsonRpcError::Protocol(message) => {
                assert!(message.contains("method must not be empty"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
