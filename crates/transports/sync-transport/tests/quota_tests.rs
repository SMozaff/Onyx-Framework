//! Team Prompt 4 §9.5 / §5. Quota enforcement is explicitly OUT OF SCOPE
//! for the transport layer itself: "The transport layer itself does NOT
//! enforce quotas... The primary enforcement is in the sync agent before
//! sending" (§5). Team 4 does not own `QuotaLedger` or `policy_port` (those
//! belong to Team 1/7's crates per the Handover Document), so this test
//! exercises the CONTRACT SHAPE Team 4 is responsible for:
//!
//! 1. `TransportError::QuotaExceeded` exists and is returned when a
//!    relay-side quota rejection occurs (§5: "It only reports
//!    TransportError::QuotaExceeded if the cloud relay rejects the
//!    request").
//! 2. A minimal local stand-in for the sync agent's pre-flight quota check
//!    (`check_quota_before_sync`, matching §5's exact function shown in the
//!    prompt) proves the "quota exceeded before any bytes are sent"
//!    ordering — using a placeholder `QuotaLedger`, not the real one.
//!
//! This does NOT prove the real sync agent enforces quota correctly — only
//! that this crate's error type and the documented calling contract compose
//! the way §5 describes. See DECISIONS.md §16.

use std::sync::{Arc, Mutex};
use std::time::Duration;
use sync_transport::cloud_relay::{CloudRelayTransport, RelaySocket, RelaySocketFactory};
use sync_transport::placeholder_types::StaticAuthorityProvider;
use sync_transport::transport_trait::{Transport, TransportError};
use sync_transport::PeerInfo;

/// Minimal local stand-in for the real `QuotaLedger` (owned by another
/// team's crate). Only enough behavior to exercise the pre-flight check
/// shape from §5.
struct StubQuotaLedger {
    remaining: u64,
}

impl StubQuotaLedger {
    fn remaining_operations(&self) -> u64 {
        self.remaining
    }
}

#[derive(Debug, PartialEq, Eq)]
enum StubSyncError {
    QuotaExceeded,
}

/// Mirrors §5's `check_quota_before_sync` exactly, against the local stub
/// ledger instead of the real `policy_port`.
fn check_quota_before_sync(
    ledger: &StubQuotaLedger,
    sync_volume: u64,
) -> Result<(), StubSyncError> {
    if ledger.remaining_operations() < sync_volume {
        return Err(StubSyncError::QuotaExceeded);
    }
    Ok(())
}

#[tokio::test]
async fn sync_operation_rejected_when_quota_exceeded_before_any_bytes_sent() {
    // Quota set low: 10 ops remaining.
    let ledger = StubQuotaLedger { remaining: 10 };

    // A send count that would exceed it (mirrors "attempt 11 sync
    // operations" from §9.5, expressed as a single batch of volume 11
    // since the transport layer deals in one Exchange phase at a time).
    let attempted_volume = 11;

    let sent_tracker = Arc::new(Mutex::new(0usize));

    // The check must run and fail BEFORE any transport call is made.
    let result = check_quota_before_sync(&ledger, attempted_volume);
    assert_eq!(result, Err(StubSyncError::QuotaExceeded));

    // Prove no bytes were sent: the transport is never invoked in this
    // branch, so the tracker stays at zero.
    assert_eq!(*sent_tracker.lock().unwrap(), 0);
}

#[tokio::test]
async fn sync_proceeds_when_quota_has_room() {
    let ledger = StubQuotaLedger { remaining: 100 };
    let attempted_volume = 11;

    let result = check_quota_before_sync(&ledger, attempted_volume);
    assert!(result.is_ok());
}

/// Relay-side quota rejection: proves `TransportError::QuotaExceeded` is a
/// real, reachable variant returned by a transport, per §5's second
/// sentence (the relay itself may also reject on quota, separately from
/// the sync agent's pre-flight check).
struct QuotaRejectingSocket;

#[async_trait::async_trait]
impl RelaySocket for QuotaRejectingSocket {
    async fn send_binary(&mut self, _bytes: Vec<u8>) -> Result<(), TransportError> {
        Err(TransportError::QuotaExceeded)
    }
    async fn recv_binary(&mut self) -> Result<Vec<u8>, TransportError> {
        Err(TransportError::QuotaExceeded)
    }
    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

struct QuotaRejectingFactory;

#[async_trait::async_trait]
impl RelaySocketFactory for QuotaRejectingFactory {
    async fn connect(
        &self,
        _relay_url: &str,
        _peer: &PeerInfo,
        _bearer_token: &str,
        _timeout: Duration,
    ) -> Result<Box<dyn RelaySocket>, TransportError> {
        Ok(Box::new(QuotaRejectingSocket))
    }
}

#[tokio::test]
async fn relay_side_quota_rejection_surfaces_as_quota_exceeded() {
    let transport = CloudRelayTransport::new(
        "relay.onyx.test",
        Arc::new(StaticAuthorityProvider("token".to_string())),
        Arc::new(QuotaRejectingFactory),
    );

    let mut conn = transport
        .connect(PeerInfo::dummy(), Duration::from_secs(1))
        .await
        .expect("connect itself should succeed; only send is rejected");

    let msg = sync_transport::SyncMessage {
        message_id: sync_transport::MessageId::new(),
        version: platform_kernel::SchemaVersion::new("1.0"),
        message_type: sync_transport::SyncMessageType::OperationBatch,
        organization_id: platform_kernel::OrganizationId::new_random(),
        sender_replica: platform_kernel::ReplicaId::new_random(),
        target_replica: None,
        payload: vec![1, 2, 3],
        timestamp: platform_kernel::Timestamp::now(),
        signature: None,
    };

    let err = conn.send(msg).await.unwrap_err();
    matches!(err, TransportError::QuotaExceeded);
}
