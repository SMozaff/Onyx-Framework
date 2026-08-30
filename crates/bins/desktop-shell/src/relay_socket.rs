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

/// Mints a short-lived, single-use relay ticket over a normal authenticated
/// HTTPS call (audit finding H4(b) item 2), rather than putting the real
/// bearer access token in the WebSocket URL.
///
/// Derives the ticket-minting endpoint from `relay_url` (swapping
/// `wss`/`ws` for `https`/`http`, keeping the same host) rather than taking
/// a second configured URL: the relay ticket endpoint always lives on the
/// same host as the relay itself, and a second, independently-configurable
/// URL would be one more place for the two to drift out of sync with no
/// benefit.
async fn mint_relay_ticket(
    relay_url: &str,
    target_id: uuid::Uuid,
    bearer_token: &str,
) -> Result<String, TransportError> {
    let mut ticket_url = reqwest::Url::parse(relay_url)
        .map_err(|e| TransportError::Platform(format!("invalid relay URL: {e}")))?;
    let https_scheme = match ticket_url.scheme() {
        "wss" => "https",
        "ws" => "http",
        other => other,
    }
    .to_string();
    ticket_url
        .set_scheme(&https_scheme)
        .map_err(|_| TransportError::Platform("relay URL has an unexpected scheme".to_string()))?;
    // Deliberately NOT under /api/relay -- see routes::relay's own module
    // doc comment: that path prefix is reserved for the dedicated,
    // single-replica relay Deployment's Ingress rule, while minting a
    // ticket is a cheap, stateless, horizontally-scalable operation that
    // belongs on the ordinary, autoscaled API fleet instead.
    ticket_url.set_path("/api/relay-ticket");
    ticket_url.set_query(None);

    let response = reqwest::Client::new()
        .post(ticket_url)
        .bearer_auth(bearer_token)
        .json(&serde_json::json!({ "target_id": target_id.to_string() }))
        .send()
        .await
        .map_err(|e| TransportError::Platform(format!("relay ticket request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(TransportError::Platform(format!(
            "relay ticket request returned {}",
            response.status()
        )));
    }
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| TransportError::Platform(format!("invalid relay ticket response: {e}")))?;
    body["ticket"].as_str().map(str::to_string).ok_or_else(|| {
        TransportError::Platform("relay ticket response missing 'ticket'".to_string())
    })
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
        peer: &PeerInfo,
        bearer_token: &str,
        timeout: Duration,
    ) -> Result<Box<dyn RelaySocket>, TransportError> {
        // H4(b): mint a narrow, short-lived, single-use ticket scoped to
        // this specific target replica, rather than sending the real
        // bearer access token as a URL query parameter -- a credential
        // valid for the rest of the API for a full hour, sitting in a URL
        // that can propagate into reverse-proxy/access/diagnostic logs
        // even over WSS. The ticket is still passed as a query parameter
        // (browser WebSocket clients cannot set request headers at all,
        // so a header-only scheme would make the relay unreachable from
        // one of the platforms that has to reach it — see the module's
        // prior note on this), but a leaked ticket is worth dramatically
        // less: it authorizes exactly one connection attempt, to exactly
        // this target, for a handful of seconds.
        let target_id = uuid::Uuid::from_bytes(peer.id.0);
        let ticket = tokio::time::timeout(
            timeout,
            mint_relay_ticket(relay_url, target_id, bearer_token),
        )
        .await
        .map_err(|_| TransportError::Timeout)??;

        let separator = if relay_url.contains('?') { '&' } else { '?' };
        let url = format!(
            "{relay_url}{separator}ticket={}&self={}",
            encode_query_value(&ticket),
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
                Some(Ok(Message::Close(_))) | None => return Err(TransportError::ConnectionLost),
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
