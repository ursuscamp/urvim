use crate::codec::{JsonRpcError, decode_message, encode_message};
use crate::transport::JsonRpcTransport;
use crate::types::{Message, Notification, Request, RequestId};
use serde_json::Value;
use std::collections::BTreeMap;

/// A pending request tracked by the JSON-RPC client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRequest {
    /// The request id.
    pub id: RequestId,
    /// The method name.
    pub method: String,
    /// Whether the request was locally canceled.
    pub canceled: bool,
}

impl PendingRequest {
    /// Creates a new pending request record.
    pub fn new(id: RequestId, method: impl Into<String>) -> Self {
        Self {
            id,
            method: method.into(),
            canceled: false,
        }
    }
}

/// Tracks in-flight request state.
#[derive(Debug, Default)]
pub struct RequestTracker {
    next_numeric_id: u64,
    pending: BTreeMap<RequestId, PendingRequest>,
}

impl RequestTracker {
    /// Creates an empty request tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates the next numeric request id.
    pub fn next_request_id(&mut self) -> RequestId {
        self.next_numeric_id += 1;
        RequestId::Number(self.next_numeric_id)
    }

    /// Registers a request as in-flight.
    pub fn insert(&mut self, request: PendingRequest) {
        self.pending.insert(request.id.clone(), request);
    }

    /// Marks a request as canceled if it is still pending.
    pub fn cancel(&mut self, id: &RequestId) -> bool {
        if let Some(request) = self.pending.get_mut(id) {
            request.canceled = true;
            true
        } else {
            false
        }
    }

    /// Removes a completed request from the tracker.
    pub fn complete(&mut self, id: &RequestId) -> Option<PendingRequest> {
        self.pending.remove(id)
    }

    /// Returns the number of tracked in-flight requests.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Returns `true` when no requests are currently pending.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// A small JSON-RPC client wrapper around a transport.
pub struct JsonRpcClient<T> {
    transport: T,
    tracker: RequestTracker,
}

impl<T> JsonRpcClient<T> {
    /// Creates a client around the provided transport.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            tracker: RequestTracker::new(),
        }
    }

    /// Returns a shared reference to the request tracker.
    pub fn tracker(&self) -> &RequestTracker {
        &self.tracker
    }

    /// Returns a mutable reference to the request tracker.
    pub fn tracker_mut(&mut self) -> &mut RequestTracker {
        &mut self.tracker
    }

    /// Returns the underlying transport.
    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T: JsonRpcTransport> JsonRpcClient<T> {
    /// Sends a request and returns its allocated id.
    pub fn send_request(
        &mut self,
        method: impl Into<String>,
        params: Option<Value>,
    ) -> Result<RequestId, JsonRpcError> {
        let id = self.tracker.next_request_id();
        let method = method.into();
        let request = Request::new(id.clone(), method.clone(), params);
        let message = Message::Request(request);
        let bytes = encode_message(&message)?;
        self.transport.write_message(&bytes)?;
        self.tracker.insert(PendingRequest::new(id.clone(), method));
        Ok(id)
    }

    /// Sends a notification.
    pub fn send_notification(
        &mut self,
        method: impl Into<String>,
        params: Option<Value>,
    ) -> Result<(), JsonRpcError> {
        let notification = Notification::new(method, params);
        let message = Message::Notification(notification);
        let bytes = encode_message(&message)?;
        self.transport.write_message(&bytes)?;
        Ok(())
    }

    /// Reads the next framed message from the transport, if any.
    pub fn poll_message(&mut self) -> Result<Option<Message>, JsonRpcError> {
        let Some(bytes) = self.transport.read_message()? else {
            return Ok(None);
        };

        Ok(Some(decode_message(&bytes)?))
    }

    /// Cancels a tracked request locally.
    pub fn cancel_request(&mut self, id: &RequestId) -> bool {
        self.tracker.cancel(id)
    }

    /// Completes a tracked request.
    pub fn complete_request(&mut self, id: &RequestId) -> Option<PendingRequest> {
        self.tracker.complete(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MemoryTransport;
    use serde_json::json;

    #[test]
    fn client_sends_requests_and_tracks_them() {
        let transport = MemoryTransport::new();
        let mut client = JsonRpcClient::new(transport);

        let id = client
            .send_request("initialize", Some(json!({"rootUri": null})))
            .expect("request");

        assert_eq!(id, RequestId::Number(1));
        assert_eq!(client.tracker().len(), 1);
        assert_eq!(
            client.tracker().pending.get(&id).expect("pending").method,
            "initialize"
        );

        let transport = client.into_transport();
        assert_eq!(transport.outgoing().len(), 1);
    }

    #[test]
    fn tracker_cancellation_marks_pending_request() {
        let mut tracker = RequestTracker::new();
        let id = tracker.next_request_id();
        tracker.insert(PendingRequest::new(id.clone(), "textDocument/hover"));

        assert!(tracker.cancel(&id));
        assert!(tracker.pending.get(&id).expect("pending").canceled);
    }
}
