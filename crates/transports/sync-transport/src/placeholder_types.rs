//! # Team-4-owned support types
//!
//! Per the Ruling on real-workspace integration (DECISIONS.md, "R1"), this
//! crate uses `platform_kernel`'s real types directly:
//! `platform_kernel::{ReplicaId, OrganizationId, Timestamp, SchemaVersion}`.
//! This file previously defined LOCAL PLACEHOLDER versions of those four
//! types; they have been deleted (not merely deprecated) now that the real
//! workspace is available, to avoid two parallel, confusable definitions.
//!
//! What remains here are types that are genuinely owned by Team 4, because
//! nothing equivalent exists in `platform-kernel` or `platform-contracts`:
//!
//! | Type                  | Why it lives here, not platform-kernel               |
//! |------------------------|--------------------------------------------------------|
//! | `AuthorityProvider`     | No async token-fetch trait exists upstream; `platform_kernel::AuthorityProof`/`ActorContext` are a *proof object* and an *actor identity*, not a "fetch me a bearer token" abstraction. Team-4-owned per explicit ruling. |
//! | `DiscoveryError`        | `Discovery` trait (§3.4) is Team 4's; its error type was never defined by the prompt. |
//! | `SerializationError`    | `SyncMessage::deserialize`'s error type; also never defined by the prompt. |
//! | `CloudDiscovery` / `LocalDiscovery` | Discovery backends the prompt names but stubs; real behavior (mDNS, relay presence API) is Team 5/Team 4 integration work, not yet built. |
//! | `PlatformHandle` / `PlatformBLEHandle` | Mobile radio bridge handles; real implementation is `sync-transport-mobile` FFI + Team 5's native call sites. |

use async_trait::async_trait;
use platform_kernel::OrganizationId;

/// Team-4-owned trait for authenticating Cloud Relay connections.
/// Not a platform-kernel type — see module docs. Implemented by the
/// application layer (desktop/mobile shell) that holds real credentials.
#[async_trait]
pub trait AuthorityProvider: Send + Sync {
    async fn bearer_token(&self) -> Result<String, AuthorityError>;
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthorityError {
    #[error("no credentials available")]
    NoCredentials,
    #[error("credential refresh failed: {0}")]
    RefreshFailed(String),
}

/// A trivial provider for tests: always returns a fixed token.
pub struct StaticAuthorityProvider(pub String);

#[async_trait]
impl AuthorityProvider for StaticAuthorityProvider {
    async fn bearer_token(&self) -> Result<String, AuthorityError> {
        Ok(self.0.clone())
    }
}

/// Errors from `Discovery::discover_peers`. Owned by this crate (the prompt
/// references `DiscoveryError` in the `Discovery` trait but never defines
/// it).
#[derive(Debug, Clone, thiserror::Error)]
pub enum DiscoveryError {
    #[error("local discovery unavailable: {0}")]
    LocalUnavailable(String),
    #[error("cloud discovery unavailable: {0}")]
    CloudUnavailable(String),
    #[error("permission denied")]
    PermissionDenied,
}

/// Errors from `SyncMessage::deserialize`. Owned by this crate for the same
/// reason as `DiscoveryError` above.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SerializationError {
    #[error("buffer too short: expected at least {expected} bytes, got {actual}")]
    BufferTooShort { expected: usize, actual: usize },
    #[error("invalid message_type byte: {0}")]
    InvalidMessageType(u8),
    #[error("invalid UTF-8 in version string")]
    InvalidVersionUtf8,
    #[error("declared payload length ({declared}) exceeds remaining buffer ({remaining})")]
    PayloadLengthMismatch { declared: u32, remaining: usize },
    #[error("declared signature length ({declared}) exceeds remaining buffer ({remaining})")]
    SignatureLengthMismatch { declared: u16, remaining: usize },
    #[error("trailing bytes after a fully-parsed message")]
    TrailingBytes,
}

/// Cloud-side peer presence lookup (via the Cloud Relay's presence
/// endpoint). Stubbed with an in-memory backing store for tests; the real
/// implementation calls out to the relay's HTTP/WebSocket presence API.
pub struct CloudDiscovery {
    pub known_peers: std::sync::Mutex<Vec<crate::discovery::PeerInfo>>,
}

impl CloudDiscovery {
    pub fn new() -> Self {
        Self {
            known_peers: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn with_peers(peers: Vec<crate::discovery::PeerInfo>) -> Self {
        Self {
            known_peers: std::sync::Mutex::new(peers),
        }
    }
}

impl Default for CloudDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::discovery::Discovery for CloudDiscovery {
    async fn discover_peers(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<crate::discovery::PeerInfo>, DiscoveryError> {
        let peers = self.known_peers.lock().expect("lock poisoned");
        Ok(peers
            .iter()
            .filter(|p| p.organization_id == organization_id)
            .cloned()
            .collect())
    }
}

/// Local-network peer presence (Wi-Fi Direct mDNS / BLE advertisement).
/// Stubbed with an in-memory backing store for tests; the real
/// implementation on mobile calls through `sync-transport-mobile` FFI.
pub struct LocalDiscovery {
    pub known_peers: std::sync::Mutex<Vec<crate::discovery::PeerInfo>>,
    /// Simulates a permission-denied local radio scan.
    pub permission_denied: bool,
}

impl LocalDiscovery {
    pub fn new() -> Self {
        Self {
            known_peers: std::sync::Mutex::new(Vec::new()),
            permission_denied: false,
        }
    }

    pub fn with_peers(peers: Vec<crate::discovery::PeerInfo>) -> Self {
        Self {
            known_peers: std::sync::Mutex::new(peers),
            permission_denied: false,
        }
    }
}

impl Default for LocalDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::discovery::Discovery for LocalDiscovery {
    async fn discover_peers(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<crate::discovery::PeerInfo>, DiscoveryError> {
        if self.permission_denied {
            return Err(DiscoveryError::PermissionDenied);
        }
        let peers = self.known_peers.lock().expect("lock poisoned");
        Ok(peers
            .iter()
            .filter(|p| p.organization_id == organization_id)
            .cloned()
            .collect())
    }
}

/// Placeholder for the desktop/mobile Wi-Fi Direct platform handle.
/// The real handle is provided by Team 5 (Native Clients) via FFI; here it
/// is an in-memory stub sufficient for `is_available`/`connect` testing.
pub struct PlatformHandle {
    pub wifi_direct_available: bool,
    pub connect_should_fail: bool,
}

impl PlatformHandle {
    pub fn new(available: bool) -> Self {
        Self {
            wifi_direct_available: available,
            connect_should_fail: false,
        }
    }

    pub fn is_wifi_direct_available(&self) -> bool {
        self.wifi_direct_available
    }

    pub async fn connect_to_peer(
        &self,
        _peer: &crate::discovery::PeerInfo,
        _timeout: std::time::Duration,
    ) -> Result<PlatformStream, PlatformHandleError> {
        if self.connect_should_fail || !self.wifi_direct_available {
            return Err(PlatformHandleError("wifi direct connect failed".into()));
        }
        Ok(PlatformStream)
    }

    pub async fn start_advertising(&self) -> Result<PlatformListenerHandle, PlatformHandleError> {
        if !self.wifi_direct_available {
            return Err(PlatformHandleError(
                "wifi direct advertising unavailable".into(),
            ));
        }
        Ok(PlatformListenerHandle)
    }
}

/// Placeholder for the mobile Bluetooth LE platform handle.
pub struct PlatformBLEHandle {
    pub ble_available: bool,
    pub connect_should_fail: bool,
}

impl PlatformBLEHandle {
    pub fn new(available: bool) -> Self {
        Self {
            ble_available: available,
            connect_should_fail: false,
        }
    }

    pub fn is_ble_available(&self) -> bool {
        self.ble_available
    }

    pub async fn connect_to_peer(
        &self,
        _peer: &crate::discovery::PeerInfo,
        _timeout: std::time::Duration,
    ) -> Result<PlatformStream, PlatformHandleError> {
        if self.connect_should_fail || !self.ble_available {
            return Err(PlatformHandleError("ble connect failed".into()));
        }
        Ok(PlatformStream)
    }

    pub async fn start_advertising(&self) -> Result<PlatformListenerHandle, PlatformHandleError> {
        if !self.ble_available {
            return Err(PlatformHandleError("ble advertising unavailable".into()));
        }
        Ok(PlatformListenerHandle)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("platform handle error: {0}")]
pub struct PlatformHandleError(pub String);

/// Opaque placeholder for a platform-native stream (Wi-Fi Direct socket or
/// BLE GATT channel). Carries no data in this stub.
pub struct PlatformStream;

/// Opaque placeholder for a platform-native listener/advertiser handle.
pub struct PlatformListenerHandle;
