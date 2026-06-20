use std::collections::VecDeque;
use std::io;

/// A transport abstraction for framed JSON-RPC messages.
pub trait JsonRpcTransport {
    /// Reads the next framed message, or `Ok(None)` on clean EOF.
    fn read_message(&mut self) -> io::Result<Option<Vec<u8>>>;

    /// Writes one framed message.
    fn write_message(&mut self, bytes: &[u8]) -> io::Result<()>;
}

/// A simple in-memory transport useful for tests.
#[derive(Debug, Default)]
pub struct MemoryTransport {
    incoming: VecDeque<Vec<u8>>,
    outgoing: Vec<Vec<u8>>,
}

impl MemoryTransport {
    /// Creates an empty in-memory transport.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueues an incoming framed message.
    pub fn push_incoming(&mut self, bytes: Vec<u8>) {
        self.incoming.push_back(bytes);
    }

    /// Returns the messages written by the transport.
    pub fn outgoing(&self) -> &[Vec<u8>] {
        &self.outgoing
    }
}

impl JsonRpcTransport for MemoryTransport {
    fn read_message(&mut self) -> io::Result<Option<Vec<u8>>> {
        Ok(self.incoming.pop_front())
    }

    fn write_message(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.outgoing.push(bytes.to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_transport_round_trips_messages() {
        let mut transport = MemoryTransport::new();
        transport.push_incoming(vec![1, 2, 3]);

        assert_eq!(transport.read_message().expect("read"), Some(vec![1, 2, 3]));
        assert_eq!(transport.read_message().expect("read"), None);

        transport.write_message(&[4, 5]).expect("write");
        assert_eq!(transport.outgoing(), &[vec![4, 5]]);
    }
}
