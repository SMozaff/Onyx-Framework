//! Real `RelaySocketFactory` backed by `tokio-tungstenite`.
//!
//! `sync-transport` deliberately keeps its public API free of any specific
//! socket library — its own module note says a "`tokio-tungstenite`-backed
//! implementation is expected to be supplied by the composition root". This
//! is that implementation. It replaces `NotYetImplementedSocketFactory`,
//! which returned `TransportError::Unreachable` unconditionally and was the
//! single reason a fully-implemented `CloudRelayTransport` still reported the
//! client as offline.
//!
//! The frames on this socket are `SyncMessage`s in the binary wire format of
//! `sync_transport::message`; this layer neither parses nor interprets them,
//! matching §8.1's boundary ("Chapter 8 only gets them from one replica to
//! another").

use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use sync_transport::{
    cloud_relay::{RelaySocket, RelaySocketFactory},
    PeerInfo, TransportError,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};

/// Percent-encodes a query-string value.
///
/// Hand-rolled rather than pulling in a URL-encoding crate for one call site.
/// The token in practice is a JWT, whose base64url alphabet plus `.` is
/// already URL-safe, so this encodes nothing in the normal case — it exists so
/// that a future non-JWT credential (an opaque token containing `+` or `=`,
/// say) does not silently corrupt itself in transit and produce an
/// authentication failure with no obvious cause.
fn encode_query_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Carries this replica's own id because the relay needs it in the handshake:
/// a connection is registered — and therefore addressable — from the moment it
/// opens, not lazily from whatever it happens to send first.
pub struct TungsteniteRelaySocketFactory {
    local_replica: platform_kernel::ReplicaId,
}

impl TungsteniteRelaySocketFactory {
    pub fn new(local_replica: platform_kernel::ReplicaId) -> Self {
        Self { local_replica }
    }
}

#[async_trait]
impl RelaySocketFactory for TungsteniteRelaySocketFactory {
    async fn connect(
        &self,
        relay_url: &str,
        _peer: &PeerInfo,
        bearer_token: &str,
        timeout: Duration,
    ) -> Result<Box<dyn RelaySocket>, TransportError> {
        // The relay authenticates with the same access token the rest of the
        // API uses, passed as a query parameter rather than an Authorization
        // header. That is not a stylistic choice: the server reads it via
        // axum's `Query` extractor exactly as `/api/events` does, and browser
        // WebSocket clients cannot set request headers at all, so a
        // header-only scheme would make the relay unreachable from one of the
        // platforms that has to reach it.
        let separator = if relay_url.contains('?') { '&' } else { '?' };
        let url = format!(
            "{relay_url}{separator}token={}&self={}",
            encode_query_value(bearer_token),
            uuid::Uuid::from_bytes(self.local_replica.0)
        );

        let request = url
            .as_str()
            .into_client_request()
            .map_err(|e| TransportError::Platform(format!("invalid relay URL: {e}")))?;

        // §8.4 gives every transport a bounded connection-attempt window
        // before the selector falls through to the next one. Without this the
        // fallback chain would stall on a relay that accepts the TCP
        // connection and then never completes the handshake.
        let (stream, _response) = tokio::time::timeout(timeout, connect_async(request))
            .await
            .map_err(|_| TransportError::Timeout)?
            .map_err(|e| TransportError::Platform(format!("relay handshake failed: {e}")))?;

        Ok(Box::new(TungsteniteRelaySocket { stream }))
    }
}

struct TungsteniteRelaySocket {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

#[async_trait]
impl RelaySocket for TungsteniteRelaySocket {
    async fn send_binary(&mut self, bytes: Vec<u8>) -> Result<(), TransportError> {
        self.stream
            .send(Message::Binary(bytes))
            .await
            .map_err(|_| TransportError::ConnectionLost)
    }

    async fn recv_binary(&mut self) -> Result<Vec<u8>, TransportError> {
        // Loops rather than returning on the first frame because a WebSocket
        // peer may interleave Ping/Pong and Text frames with the binary ones
        // this protocol actually uses. Returning an error on those would tear
        // down a healthy connection over ordinary keepalive traffic.
        loop {
            match self.stream.next().await {
                Some(Ok(Message::Binary(bytes))) => return Ok(bytes),
                Some(Ok(Message::Close(_))) | None => {
                    return Err(TransportError::ConnectionLost)
                }
                Some(Ok(_)) => continue,
                Some(Err(_)) => return Err(TransportError::ConnectionLost),
            }
        }
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.stream
            .close(None)
            .await
            .map_err(|_| TransportError::ConnectionLost)
    }
}
