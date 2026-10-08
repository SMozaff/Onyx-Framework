//! `sync-transport`: the secure, reliable, platform-aware communication
//! substrate for ONYX replica synchronization. See Team Prompt 4.
//!
//! Production code in this crate depends on `async-trait` + `std::future`
//! only — no direct `tokio` dependency (the runtime is injected by the
//! composing binary, per the ruling in DECISIONS.md §10).

pub mod bluetooth_le;
pub mod cloud_relay;
pub mod discovery;
pub mod message;
pub mod quic_cross_network;
pub mod selector;
pub mod transport_trait;
pub mod wi_fi_direct;

/// Team-4-owned support types not covered by `platform-kernel`
/// (AuthorityProvider, DiscoveryError, SerializationError, CloudDiscovery,
/// LocalDiscovery, PlatformHandle, PlatformBLEHandle). Real cross-team
/// identifiers/timestamps (`ReplicaId`, `OrganizationId`, `Timestamp`,
/// `SchemaVersion`) now come directly from `platform_kernel` — see module
/// docs for what changed when the real workspace was integrated.
pub mod placeholder_types;

/// Test doubles (`MockTransport`, etc.), gated behind the `test-support`
/// feature so both this crate's `#[cfg(test)]` unit tests and the separate
/// `tests/integration/*.rs` binaries can use the same mocks. See
/// DECISIONS.md §9.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use discovery::{CompositeDiscovery, Discovery, PeerInfo};
pub use message::{MessageId, SyncMessage, SyncMessageType};
pub use selector::TransportSelector;
pub use transport_trait::{Connection, Listener, Transport, TransportError, TransportType};
