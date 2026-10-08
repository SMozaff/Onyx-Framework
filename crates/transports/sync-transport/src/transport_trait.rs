//! Frozen contract, copied verbatim from Team Prompt 4 §3.1.
//! Do not alter signatures without following the formal amendment process.

use crate::discovery::PeerInfo;
use crate::message::SyncMessage;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// The core transport trait. Every approved transport implements this.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Unique identifier for this transport type.
    fn transport_type(&self) -> TransportType;

    /// Whether this transport is currently available on the current platform/hardware.
    /// Must check OS capabilities (e.g., Wi-Fi enabled, Bluetooth enabled, permissions).
    fn is_available(&self) -> bool;

    /// Attempt to connect to a peer. Returns a connection handle.
    async fn connect(
        &self,
        peer: PeerInfo,
        timeout: Duration,
    ) -> Result<Box<dyn Connection>, TransportError>;

    /// Listen for incoming connections from peers.
    async fn listen(
        &self,
        listen_addr: Option<SocketAddr>,
    ) -> Result<Box<dyn Listener>, TransportError>;

    /// The maximum message size this transport can carry (in bytes).
    fn max_message_size(&self) -> usize;

    /// Does this transport provide encryption inherently?
    /// For all approved transports, this MUST return true.
    fn provides_encryption(&self) -> bool {
        true // All approved transports are encrypted.
    }
}

/// A connection to a peer. Provides request-response and fire-and-forget semantics.
#[async_trait]
pub trait Connection: Send + Sync {
    /// Send a synchronous message and wait for a response (request-response).
    async fn request_response(
        &mut self,
        request: SyncMessage,
        timeout: Duration,
    ) -> Result<SyncMessage, TransportError>;

    /// Send a message without waiting for a response (fire-and-forget).
    async fn send(&mut self, message: SyncMessage) -> Result<(), TransportError>;

    /// Receive the next message from the connection.
    async fn recv(&mut self) -> Result<SyncMessage, TransportError>;

    /// Close the connection gracefully.
    async fn close(&mut self) -> Result<(), TransportError>;

    /// Which transport type produced this connection.
    ///
    /// NOTE: Not present in the prompt's literal trait listing under §3.1,
    /// but required by the acceptance test in §9.1
    /// (`assert_eq!(conn.transport_type(), TransportType::CloudRelay)`),
    /// which calls `conn.transport_type()` on a `Box<dyn Connection>`. This
    /// is recorded as a discrepancy in DECISIONS.md (§2) rather than silently
    /// added.
    fn transport_type(&self) -> TransportType;
}

/// A listener that accepts incoming connections from peers.
#[async_trait]
pub trait Listener: Send + Sync {
    /// Accept an incoming connection from a peer.
    async fn accept(&mut self) -> Result<Box<dyn Connection>, TransportError>;

    /// Close the listener.
    async fn close(&mut self) -> Result<(), TransportError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportType {
    CloudRelay,
    WifiDirect,
    BluetoothLE,
    QuicCrossNetwork,
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TransportType::CloudRelay => "CloudRelay",
            TransportType::WifiDirect => "WifiDirect",
            TransportType::BluetoothLE => "BluetoothLE",
            TransportType::QuicCrossNetwork => "QuicCrossNetwork",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum TransportError {
    #[error("Peer unreachable")]
    Unreachable,
    #[error("Connection timed out")]
    Timeout,
    #[error("Connection lost")]
    ConnectionLost,
    #[error("Message too large (max: {max})")]
    MessageTooLarge { max: usize },
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Quota exceeded")]
    QuotaExceeded,
    #[error("Protocol violation")]
    ProtocolViolation,
    #[error("No transport available")]
    NoTransportAvailable,
    #[error("Platform error: {0}")]
    Platform(String),
}
