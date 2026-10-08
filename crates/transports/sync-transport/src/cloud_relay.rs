//! Cloud Relay: always-available fallback. Frozen contract per Team Prompt 4
//! §4.1. Uses TLS 1.3 over WebSocket to a central relay server.
//!
//! Constraint: production code must not depend on `tokio` directly (the
//! runtime is injected by the composing binary). This module depends only
//! on `async_trait` and `std::future`; the actual WebSocket I/O is behind
//! the `RelaySocket` trait so a concrete `tokio-tungstenite` adapter can be
//! supplied by the composition root (e.g. `sync-agent`) without this crate
//! naming `tokio` in `[dependencies]`. See DECISIONS.md §10.

use crate::discovery::PeerInfo;
use crate::message::SyncMessage;
use crate::placeholder_types::AuthorityProvider;
use crate::transport_trait::{Connection, Listener, Transport, TransportError, TransportType};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

/// Abstraction over a bidirectional binary WebSocket-like channel, so this
/// crate can avoid a hard dependency on a specific async runtime / socket
/// library in its public API. A `tokio-tungstenite`-backed implementation
/// is expected to be supplied by the composition root.
#[async_trait]
pub trait RelaySocket: Send + Sync {
    async fn send_binary(&mut self, bytes: Vec<u8>) -> Result<(), TransportError>;
    async fn recv_binary(&mut self) -> Result<Vec<u8>, TransportError>;
    async fn close(&mut self) -> Result<(), TransportError>;
}

/// Opens a `RelaySocket` to the given relay URL for the given peer, using
/// the supplied bearer token for auth. Implemented by the composition root
/// (binds to `tokio-tungstenite` + `reqwest` there); `sync-transport` only
/// consumes the trait.
#[async_trait]
pub trait RelaySocketFactory: Send + Sync {
    async fn connect(
        &self,
        relay_url: &str,
        peer: &PeerInfo,
        bearer_token: &str,
        timeout: Duration,
    ) -> Result<Box<dyn RelaySocket>, TransportError>;
}

/// Cloud Relay: always-available fallback.
/// Uses TLS 1.3 over WebSocket (or QUIC) to a central relay server.
pub struct CloudRelayTransport {
    endpoint: String,
    auth_provider: Arc<dyn AuthorityProvider>,
    socket_factory: Arc<dyn RelaySocketFactory>,
}

impl CloudRelayTransport {
    pub fn new(
        endpoint: impl Into<String>,
        auth_provider: Arc<dyn AuthorityProvider>,
        socket_factory: Arc<dyn RelaySocketFactory>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            auth_provider,
            socket_factory,
        }
    }
}

#[async_trait]
impl Transport for CloudRelayTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::CloudRelay
    }

    fn is_available(&self) -> bool {
        true // Always available if internet is reachable.
    }

    async fn connect(
        &self,
        peer: PeerInfo,
        timeout: Duration,
    ) -> Result<Box<dyn Connection>, TransportError> {
        let token = self
            .auth_provider
            .bearer_token()
            .await
            .map_err(|e| TransportError::Platform(e.to_string()))?;

        // `platform_kernel::ReplicaId` implements `Debug` (formatted as
        // "ReplicaId(<uuid>)") but not `Display`, so embedding it directly
        // in a URL path via `{}` doesn't compile — caught by `cargo check`,
        // not assumed. Converting explicitly to the bare UUID string
        // instead, since that's what belongs in a URL path segment.
        let peer_uuid = uuid::Uuid::from_bytes(peer.id.0);
        let ws_url = format!("wss://{}/relay/{}", self.endpoint, peer_uuid);
        let socket = self
            .socket_factory
            .connect(&ws_url, &peer, &token, timeout)
            .await?;

        Ok(Box::new(CloudRelayConnection { socket, peer }))
    }

    async fn listen(
        &self,
        _listen_addr: Option<std::net::SocketAddr>,
    ) -> Result<Box<dyn Listener>, TransportError> {
        // Cloud relay does not listen locally; it connects outbound.
        // The relay handles inbound routing.
        Err(TransportError::Unreachable)
    }

    fn max_message_size(&self) -> usize {
        10 * 1024 * 1024 // 10 MB
    }
}

struct CloudRelayConnection {
    socket: Box<dyn RelaySocket>,
    #[allow(dead_code)]
    peer: PeerInfo,
}

#[async_trait]
impl Connection for CloudRelayConnection {
    async fn request_response(
        &mut self,
        request: SyncMessage,
        timeout: Duration,
    ) -> Result<SyncMessage, TransportError> {
        let request_id = request.message_id;
        self.send(request).await?;

        let deadline = std::time::Instant::now() + timeout;
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(TransportError::Timeout);
            }
            let msg = self.recv().await?;
            if msg.message_id == request_id {
                return Ok(msg);
            }
            // Otherwise, it's a notification for another in-flight request;
            // a full implementation would buffer it for that caller. Out of
            // scope for this transport-layer stub (single in-flight request
            // per connection is assumed here).
        }
    }

    async fn send(&mut self, message: SyncMessage) -> Result<(), TransportError> {
        let bytes = message.serialize();
        self.socket.send_binary(bytes).await
    }

    async fn recv(&mut self) -> Result<SyncMessage, TransportError> {
        let bytes = self.socket.recv_binary().await?;
        SyncMessage::deserialize(&bytes).map_err(|_| TransportError::ProtocolViolation)
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.socket.close().await
    }

    fn transport_type(&self) -> TransportType {
        TransportType::CloudRelay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placeholder_types::StaticAuthorityProvider;
    use std::sync::Mutex;

    /// An in-memory `RelaySocket` that records every frame sent, so tests
    /// can assert the wire bytes never contain a recognizable plaintext
    /// payload marker (see `encryption_tests.rs` at the crate root for the
    /// literal §9.2 acceptance test).
    struct RecordingSocket {
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
        reply_queue: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    #[async_trait]
    impl RelaySocket for RecordingSocket {
        async fn send_binary(&mut self, bytes: Vec<u8>) -> Result<(), TransportError> {
            self.sent.lock().unwrap().push(bytes);
            Ok(())
        }

        async fn recv_binary(&mut self) -> Result<Vec<u8>, TransportError> {
            self.reply_queue
                .lock()
                .unwrap()
                .pop()
                .ok_or(TransportError::ConnectionLost)
        }

        async fn close(&mut self) -> Result<(), TransportError> {
            Ok(())
        }
    }

    struct RecordingSocketFactory {
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
        reply_queue: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    #[async_trait]
    impl RelaySocketFactory for RecordingSocketFactory {
        async fn connect(
            &self,
            _relay_url: &str,
            _peer: &PeerInfo,
            _bearer_token: &str,
            _timeout: Duration,
        ) -> Result<Box<dyn RelaySocket>, TransportError> {
            Ok(Box::new(RecordingSocket {
                sent: self.sent.clone(),
                reply_queue: self.reply_queue.clone(),
            }))
        }
    }

    #[tokio::test]
    async fn cloud_relay_is_always_available() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let reply_queue = Arc::new(Mutex::new(Vec::new()));
        let transport = CloudRelayTransport::new(
            "relay.onyx.test",
            Arc::new(StaticAuthorityProvider("test-token".to_string())),
            Arc::new(RecordingSocketFactory { sent, reply_queue }),
        );
        assert!(transport.is_available());
    }

    #[tokio::test]
    async fn send_serializes_to_bytes_not_struct_debug() {
        use platform_kernel::{OrganizationId, ReplicaId, SchemaVersion, Timestamp};

        let sent = Arc::new(Mutex::new(Vec::new()));
        let reply_queue = Arc::new(Mutex::new(Vec::new()));
        let transport = CloudRelayTransport::new(
            "relay.onyx.test",
            Arc::new(StaticAuthorityProvider("test-token".to_string())),
            Arc::new(RecordingSocketFactory {
                sent: sent.clone(),
                reply_queue,
            }),
        );

        let mut conn = transport
            .connect(PeerInfo::dummy(), Duration::from_secs(1))
            .await
            .unwrap();

        let msg = SyncMessage {
            message_id: crate::message::MessageId::new(),
            version: SchemaVersion::new("1.0"),
            message_type: crate::message::SyncMessageType::Ack,
            organization_id: OrganizationId::new_random(),
            sender_replica: ReplicaId::new_random(),
            target_replica: None,
            payload: b"super-secret-plaintext-marker".to_vec(),
            timestamp: Timestamp::now(),
            signature: None,
        };
        conn.send(msg.clone()).await.unwrap();

        let recorded = sent.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        // The wire bytes must equal SyncMessage::serialize()'s output
        // exactly (i.e., go through the frozen binary format), not a
        // Debug/JSON dump of the struct.
        assert_eq!(recorded[0], msg.serialize());
    }
}
