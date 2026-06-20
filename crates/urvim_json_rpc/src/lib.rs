//! Transport-agnostic JSON-RPC support for LSP and future ACP use.
//!
//! This module owns the wire-shaped JSON-RPC types, content-length framing,
//! and transport abstractions used by higher-level protocol clients.

mod client;
mod codec;
mod frame;
mod transport;
mod types;

pub use client::{JsonRpcClient, PendingRequest, RequestTracker};
pub use codec::{JsonRpcError, decode_message, encode_message};
pub use frame::{ContentLengthFrame, DecodedFrame, FrameError};
pub use transport::{JsonRpcTransport, MemoryTransport};
pub use types::{
    ErrorResponse, JSON_RPC_VERSION, Message, Notification, Request, RequestId, Response,
    ResponseError, SuccessResponse,
};
