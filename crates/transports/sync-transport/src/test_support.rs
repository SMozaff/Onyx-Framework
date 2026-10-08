//! Shared test doubles. Not part of the frozen contract; extends the
//! `MockTransport` sketch in Team Prompt 4 §8.2 with a working
//! `Connection` implementation and a constructor, since the prompt's
//! version was a partial sketch (`// ... other methods`) with no
//! `Connection` type and no `new`. See DECISIONS.md §9.
//!
//! Only compiled under `#[cfg(test)]` via the `test-support` feature so it
//! is also usable from the crate's `tests/integration/*` binaries.

use crate::discovery::PeerInfo;
use crate::message::SyncMessage;
use crate::transport_trait::{Connection, Listener, Transport, TransportError, TransportType};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct MockTransport {
    transport_type: TransportType,
    available: bool,
    should_fail: bool,
    /// Counts `connect` invocations, for concurrency tests.
    pub connect_count: Arc<AtomicUsize>,
}

impl MockTransport {
    pub fn new(transport_type: TransportType, available: bool, should_fail: bool) -> Self {
        Self {
            transport_type,
            available,
            should_fail,
            connect_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl Transport for MockTransport {
    fn transport_type(&self) -> TransportType {
        self.transport_type
    }

    fn is_available(&self) -> bool {
        self.available
    }

    async fn connect(
        &self,
        _peer: PeerInfo,
        _timeout: Duration,
    ) -> Result<Box<dyn Connection>, TransportError> {
        self.connect_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            Err(TransportError::Unreachable)
        } else {
            Ok(Box::new(MockConnection::new(self.transport_type)))
        }
    }

    async fn listen(
        &self,
        _listen_addr: Option<std::net::SocketAddr>,
    ) -> Result<Box<dyn Listener>, TransportError> {
        Ok(Box::new(MockListener {
            transport_type: self.transport_type,
        }))
    }

    fn max_message_size(&self) -> usize {
        1024 * 1024
    }
}

pub struct MockConnection {
    transport_type: TransportType,
    inbox: Vec<SyncMessage>,
}

impl MockConnection {
    pub fn new(transport_type: TransportType) -> Self {
        Self {
            transport_type,
            inbox: Vec::new(),
        }
    }
}

#[async_trait]
impl Connection for MockConnection {
    async fn request_response(
        &mut self,
        request: SyncMessage,
        _timeout: Duration,
    ) -> Result<SyncMessage, TransportError> {
        // Echo back an Ack referencing the same message_id.
        Ok(SyncMessage {
            message_type: crate::message::SyncMessageType::Ack,
            ..request
        })
    }

    async fn send(&mut self, message: SyncMessage) -> Result<(), TransportError> {
        self.inbox.push(message);
        Ok(())
    }

    async fn recv(&mut self) -> Result<SyncMessage, TransportError> {
        self.inbox.pop().ok_or(TransportError::ConnectionLost)
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        self.transport_type
    }
}

pub struct MockListener {
    transport_type: TransportType,
}

#[async_trait]
impl Listener for MockListener {
    async fn accept(&mut self) -> Result<Box<dyn Connection>, TransportError> {
        Ok(Box::new(MockConnection::new(self.transport_type)))
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}
