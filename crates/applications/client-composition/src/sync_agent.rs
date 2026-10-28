//! `SyncAgent` — the client-side orchestrator that (1) pumps the local
//! outbox into the `EventBus` for UI push, and (2) periodically runs a
//! `SynchronizationSession` against a discovered peer over the
//! `TransportSelector`'s fallback chain.
//!
//! # Provenance
//! Team Prompt 5 §3.2 (`get_sync_status`), §4.1 (`SyncAgent::new(...).run()`)
//! reference `SyncAgent` by name and illustrative shape only — it does not
//! exist in Increments 1–4 (R1, `DECISIONS.md`). Built against the real
//! `synchronization_domain::SynchronizationSession` (Increment 3) and
//! `sync_transport::{TransportSelector, CompositeDiscovery}` (Increment 4),
//! whose actual signatures differ from Team Prompt 5's §4.3/§4.4
//! snippets — see R2 in `DECISIONS.md` for the specific corrections this
//! implementation makes.

use std::sync::Arc;
use std::time::Duration as StdDuration;

use platform_kernel::{OrganizationId, ReplicaId};
use sync_transport::message::SyncMessageType;
use sync_transport::{CompositeDiscovery, PeerInfo, TransportError, TransportSelector};
use synchronization_domain::{
    ConflictRecord, ConflictResolution, SyncCursor, SyncError, SynchronizationSession,
};
use worker_application::{ClaimedMessage, OutboxStore};

use crate::cloud_outbox::{CloudOutboxUploader, UploadAck, UploadItem};

/// A row is retried this many times with a rejected/failed ack before it is
/// moved to the dead-letter table. Offline attempts do not count.
const MAX_CLOUD_ATTEMPTS: u32 = 10;
/// Backoff is 2^attempt seconds, capped at this value.
const MAX_BACKOFF_SECS: u64 = 300;

use crate::event_bus::EventBus;

/// Exponential backoff for cloud retries: 2^attempt seconds, capped.
fn retry_backoff(attempt: u32) -> platform_kernel::Duration {
    let secs = 2u64.saturating_pow(attempt.min(16)).min(MAX_BACKOFF_SECS);
    platform_kernel::Duration(secs.saturating_mul(1_000_000_000))
}

/// Configuration for a `SyncAgent`. No prior increment specifies these
/// values; chosen as reasonable defaults, not frozen contract.
#[derive(Debug, Clone)]
pub struct SyncAgentConfig {
    /// How often to poll the local outbox and forward new events to the
    /// `EventBus`.
    pub outbox_poll_interval: StdDuration,
    /// How often to attempt a full synchronization session with a
    /// discovered peer.
    pub sync_interval: StdDuration,
    /// Per-transport connection attempt timeout, passed to
    /// `TransportSelector::connect_best`.
    pub transport_timeout: StdDuration,
    /// How many outbox messages to claim per poll.
    pub outbox_batch_size: usize,
}

impl Default for SyncAgentConfig {
    fn default() -> Self {
        Self {
            outbox_poll_interval: StdDuration::from_secs(2),
            sync_interval: StdDuration::from_secs(60),
            transport_timeout: StdDuration::from_secs(10),
            outbox_batch_size: 50,
        }
    }
}

/// Current sync state, returned by `get_sync_status` (Team Prompt 5 §3.2).
/// Shape is new (not defined by any prior increment) — covers the
/// offline/queued/synced/conflicted states Team Prompt 5 §7 "Sync Status
/// Display" acceptance criterion requires, sourced from
/// `SynchronizationSession::state()`/`pending_conflicts()` and the local
/// outbox's `pending_count()`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncStatus {
    pub online: bool,
    pub pending_outbox_count: u64,
    pub open_conflict_count: usize,
    pub last_sync_attempt: Option<platform_kernel::Timestamp>,
    pub last_sync_result: Option<String>,
}

pub struct SyncAgent {
    local_replica: ReplicaId,
    organization_id: OrganizationId,
    outbox: Arc<dyn OutboxStore>,
    discovery: Arc<CompositeDiscovery>,
    transport_selector: Arc<TransportSelector>,
    event_bus: Arc<EventBus>,
    config: SyncAgentConfig,
    status: Arc<tokio::sync::RwLock<SyncStatus>>,
    open_conflicts: Arc<tokio::sync::RwLock<Vec<ConflictRecord>>>,
    /// Cloud uploader for outbox rows. `None` leaves rows pending (they are
    /// still delivered to the local UI bus once).
    cloud_uploader: Option<Arc<dyn CloudOutboxUploader>>,
}

impl SyncAgent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_replica: ReplicaId,
        organization_id: OrganizationId,
        outbox: Arc<dyn OutboxStore>,
        discovery: Arc<CompositeDiscovery>,
        transport_selector: Arc<TransportSelector>,
        event_bus: Arc<EventBus>,
        config: SyncAgentConfig,
    ) -> Self {
        Self {
            local_replica,
            organization_id,
            outbox,
            discovery,
            transport_selector,
            event_bus,
            config,
            status: Arc::new(tokio::sync::RwLock::new(SyncStatus {
                online: false,
                pending_outbox_count: 0,
                open_conflict_count: 0,
                last_sync_attempt: None,
                last_sync_result: None,
            })),
            open_conflicts: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            cloud_uploader: None,
        }
    }

    /// Attach the cloud uploader used by the outbox pump.
    pub fn with_cloud_uploader(mut self, uploader: Arc<dyn CloudOutboxUploader>) -> Self {
        self.cloud_uploader = Some(uploader);
        self
    }

    /// Current sync status snapshot. Cheap; safe to call frequently from a
    /// Tauri command / FFI query.
    pub async fn status(&self) -> SyncStatus {
        self.status.read().await.clone()
    }

    /// Returns the currently open conflict records for client conflict UI.
    /// Conflicts are retained for the lifetime of this client process; the
    /// authoritative synchronization-domain resolution remains represented
    /// by the `ConflictRecord` itself.
    pub async fn open_conflicts(&self) -> Vec<ConflictRecord> {
        self.open_conflicts
            .read()
            .await
            .iter()
            .filter(|record| record.status != synchronization_domain::ConflictStatus::Resolved)
            .cloned()
            .collect()
    }

    /// Applies a human resolution to an open conflict and updates the sync
    /// status presented to native clients. Returns `false` when the conflict
    /// is no longer open or was not found.
    pub async fn resolve_conflict(
        &self,
        conflict_id: platform_kernel::ConflictId,
        resolution: ConflictResolution,
    ) -> bool {
        let mut records = self.open_conflicts.write().await;
        let Some(record) = records.iter_mut().find(|record| {
            record.conflict_id == conflict_id
                && record.status != synchronization_domain::ConflictStatus::Resolved
        }) else {
            return false;
        };
        record.resolve(resolution);
        let open_count = records
            .iter()
            .filter(|record| record.status != synchronization_domain::ConflictStatus::Resolved)
            .count();
        drop(records);
        self.status.write().await.open_conflict_count = open_count;
        true
    }

    /// Runs both the outbox pump and the periodic sync loop concurrently.
    /// Intended to be spawned once as a background tokio task at client
    /// startup (`tokio::spawn(sync_agent.run())` — matches Team Prompt 5
    /// §4.1 step 6's intent, though not its literal `SyncAgent::new(...)`
    /// call shape; see R2).
    pub async fn run(self: Arc<Self>) {
        let outbox_pump = {
            let agent = Arc::clone(&self);
            tokio::spawn(async move { agent.run_outbox_pump().await })
        };
        let sync_loop = {
            let agent = Arc::clone(&self);
            tokio::spawn(async move { agent.run_sync_loop().await })
        };

        // Both loops run for the lifetime of the process; if either exits
        // (which should only happen on an unrecoverable error, since both
        // are internally `loop`s that log-and-continue on transient
        // failures), log it — the other loop keeps running rather than
        // taking the whole agent down over one failure mode.
        let (outbox_res, sync_res) = tokio::join!(outbox_pump, sync_loop);
        if let Err(e) = outbox_res {
            tracing::error!(error = ?e, "outbox pump task panicked");
        }
        if let Err(e) = sync_res {
            tracing::error!(error = ?e, "sync loop task panicked");
        }
    }

    /// Polls the local `OutboxStore` and republishes each claimed message
    /// to the `EventBus`, so the UI observes locally-committed events even
    /// before they've been exchanged with any peer. See `event_bus.rs`'s
    /// `publish()` doc comment for why this indirection is necessary
    /// (`handle_command` doesn't return event envelopes directly).
    async fn run_outbox_pump(&self) {
        let consumer_id = "client-ui-event-pump";
        loop {
            match self
                .outbox
                .claim_unpublished(consumer_id, self.config.outbox_batch_size)
                .await
            {
                Ok(claimed) => {
                    self.process_claimed(claimed).await;
                    if let Ok(pending) = self.outbox.pending_count().await {
                        self.status.write().await.pending_outbox_count = pending;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "outbox claim failed; will retry next poll");
                }
            }

            tokio::time::sleep(self.config.outbox_poll_interval).await;
        }
    }

    /// One pump pass over claimed rows.
    ///
    /// 1. Malformed payloads go to the dead-letter table at once.
    /// 2. Each valid row is delivered to the local UI bus once
    ///    (`bus_delivered`), so a retry never repeats a UI event.
    /// 3. Rows are uploaded to the cloud. Only a `stored` ack marks a row
    ///    done. Rejections are retried with backoff and dead-lettered after
    ///    `MAX_CLOUD_ATTEMPTS`. Whole-batch failures (offline, auth, server)
    ///    do not count as attempts: the lease expires and the rows are
    ///    claimed again later.
    async fn process_claimed(&self, claimed: Vec<ClaimedMessage>) {
        let mut uploadable: Vec<(ClaimedMessage, UploadItem)> = Vec::with_capacity(claimed.len());

        for msg in claimed {
            let payload = match serde_json::from_slice::<serde_json::Value>(&msg.message.payload) {
                Ok(value) => value,
                Err(e) => {
                    self.dead_letter_row(&msg, &format!("malformed payload: {e}"))
                        .await;
                    continue;
                }
            };

            if !msg.bus_delivered {
                self.event_bus.publish(payload.clone());
                if let Err(e) = self.outbox.mark_bus_delivered(msg.outbox_id).await {
                    tracing::warn!(outbox_id = ?msg.outbox_id, error = %e, "failed to record bus delivery");
                }
            }

            let item = UploadItem {
                outbox_id: msg.outbox_id.0,
                event_id: uuid::Uuid::from_bytes(msg.message.event_id.0).to_string(),
                event_type: msg.message.event_type.clone(),
                aggregate_id: msg.message.aggregate_id.clone(),
                organization_id: uuid::Uuid::from_bytes(msg.message.organization_id.0).to_string(),
                occurred_at_millis: (msg.message.occurred_at.0 / 1_000_000) as i64,
                payload,
            };
            uploadable.push((msg, item));
        }

        // Without an uploader the rows stay pending; UI delivery already happened.
        let Some(uploader) = self.cloud_uploader.as_ref() else {
            return;
        };
        if uploadable.is_empty() {
            return;
        }

        let items: Vec<UploadItem> = uploadable.iter().map(|(_, item)| item.clone()).collect();
        match uploader.upload(&items).await {
            Ok(acks) => self.apply_acks(&uploadable, &acks).await,
            Err(e) => {
                tracing::warn!(error = %e, "cloud outbox upload failed; rows stay pending");
            }
        }
    }

    async fn apply_acks(&self, uploaded: &[(ClaimedMessage, UploadItem)], acks: &[UploadAck]) {
        for (msg, _) in uploaded {
            let Some(ack) = acks.iter().find(|a| match a {
                UploadAck::Stored { outbox_id } | UploadAck::Rejected { outbox_id, .. } => {
                    *outbox_id == msg.outbox_id.0
                }
            }) else {
                // No answer for this row: leave it pending (lease expires).
                continue;
            };

            match ack {
                UploadAck::Stored { .. } => {
                    if let Err(e) = self.outbox.mark_published(msg.outbox_id).await {
                        tracing::warn!(outbox_id = ?msg.outbox_id, error = %e, "failed to mark outbox row acknowledged");
                    }
                }
                UploadAck::Rejected {
                    retryable: false,
                    reason,
                    ..
                } => {
                    self.dead_letter_row(msg, &format!("rejected by cloud: {reason}"))
                        .await;
                }
                UploadAck::Rejected {
                    retryable: true,
                    reason,
                    ..
                } => {
                    let attempt = msg.retry_count.saturating_add(1);
                    if attempt >= MAX_CLOUD_ATTEMPTS {
                        self.dead_letter_row(msg, &format!("cloud retries exhausted: {reason}"))
                            .await;
                    } else {
                        let backoff = retry_backoff(attempt);
                        if let Err(e) = self
                            .outbox
                            .mark_failed(msg.outbox_id, reason, backoff)
                            .await
                        {
                            tracing::warn!(outbox_id = ?msg.outbox_id, error = %e, "failed to record outbox retry");
                        }
                    }
                }
            }
        }
    }

    async fn dead_letter_row(&self, msg: &ClaimedMessage, reason: &str) {
        tracing::warn!(outbox_id = ?msg.outbox_id, reason, "outbox row moved to dead letter");
        if let Err(e) = self.outbox.dead_letter(msg.outbox_id, reason).await {
            tracing::warn!(outbox_id = ?msg.outbox_id, error = %e, "failed to dead-letter outbox row");
        }
    }

    /// Periodically discovers a peer and runs a full four-phase
    /// synchronization session against it.
    async fn run_sync_loop(&self) {
        loop {
            tokio::time::sleep(self.config.sync_interval).await;
            let _ = self.run_one_cycle().await;
        }
    }

    /// Runs a single discover-and-sync cycle immediately (discover a
    /// peer, run one session against it if found, update status) and
    /// returns whether a session actually ran. Shared by the periodic
    /// loop (`run_sync_loop`) and [`trigger_sync_now`](Self::trigger_sync_now) —
    /// factored out so both go through identical status-tracking rather
    /// than duplicating it.
    async fn run_one_cycle(&self) -> Result<(), SyncSessionError> {
        let peers = self.discovery.discover(self.organization_id).await;
        let Some(peer) = peers.into_iter().next() else {
            tracing::debug!("no peers discovered this cycle");
            self.status.write().await.online = false;
            return Ok(());
        };

        self.status.write().await.last_sync_attempt = Some(platform_kernel::Timestamp::now());

        match self.run_one_session(peer).await {
            Ok(()) => {
                let mut status = self.status.write().await;
                status.online = true;
                status.last_sync_result = Some("success".to_string());
                Ok(())
            }
            Err(e) => {
                tracing::warn!(error = %e, "sync session failed");
                let mut status = self.status.write().await;
                status.online = false;
                status.last_sync_result = Some(format!("error: {e}"));
                Err(e)
            }
        }
    }

    /// Forces an immediate sync cycle rather than waiting for the next
    /// periodic tick. Team Prompt 5 §3.3's `mobile_core_trigger_sync`
    /// ("trigger a sync session manually") needs this — no prior
    /// increment or ruling defined it, since `SyncAgent` itself is new
    /// infrastructure (R1) whose only previously-implemented entry point
    /// was the periodic loop inside `run()`. Added here (not just inside
    /// `mobile-core`) so `desktop-shell` could use the same manual-trigger
    /// capability if it ever needs one, keeping the capability on the
    /// shared composition type rather than duplicated per-client.
    pub async fn trigger_sync_now(&self) -> Result<(), SyncSessionError> {
        self.run_one_cycle().await
    }

    /// Runs one Discovery -> Exchange -> Reconciliation -> Completion
    /// cycle against `peer`, using the wire protocol defined by ruling W1
    /// (`DECISIONS.md`): each `SyncMessage.payload` is a JSON-encoded
    /// schema keyed by `SyncMessageType` (`DiscoveryRequest`/
    /// `DiscoveryResponse` carry `{replica_id, vector_clock}`;
    /// `OfferOperations` carries `{aggregate_id, from_version}`;
    /// `OperationBatch` carries `{events: [DomainEventEnvelope, ...]}`;
    /// `ConflictNotification` carries `{conflict_id, field}`).
    ///
    /// Corrections made against the real delivered types (documented per
    /// the same convention as every other ruling's illustrative snippet —
    /// see R2/W1 in `DECISIONS.md`):
    /// - `Connection::send`/`recv`/`request_response` take `&mut self`.
    /// - `AggregateDelta` (returned by `session.discover()`) has
    ///   `aggregate_id`, not `id`, and has **no** `version` field — a
    ///   per-aggregate "from version" isn't available from Discovery
    ///   alone in the current session API, so `from_version` is
    ///   conservatively sent as `0` (full resync for that aggregate)
    ///   rather than fabricating a field that doesn't exist. Flagged, not
    ///   silently assumed correct.
    /// - `ConflictRecord`'s field is `field_path`, not `field`.
    async fn run_one_session(&self, peer: PeerInfo) -> Result<(), SyncSessionError> {
        let mut connection = self
            .transport_selector
            .connect_best(peer.clone(), self.config.transport_timeout)
            .await
            .map_err(SyncSessionError::Transport)?;

        let result = self
            .run_session_over_connection(&peer, connection.as_mut())
            .await;
        let _ = connection.close().await;
        result
    }

    /// The actual protocol logic, factored out from `run_one_session` so it
    /// can be exercised in tests against a scripted `Connection` without a
    /// real `Transport`/`TransportSelector` (which requires OS-level radio
    /// access to construct meaningfully). `run_one_session` is the only
    /// production caller.
    async fn run_session_over_connection(
        &self,
        peer: &PeerInfo,
        connection: &mut dyn sync_transport::Connection,
    ) -> Result<(), SyncSessionError> {
        let mut session =
            SynchronizationSession::new(self.local_replica, peer.id, self.organization_id);

        // Phase 1: Discovery. Send our vector clock, receive the peer's.
        let discovery_payload = DiscoveryPayload {
            replica_id: self.local_replica,
            vector_clock: session.vector_clock().clone(),
        };
        let discovery_msg = new_sync_message(
            SyncMessageType::DiscoveryRequest,
            self.organization_id,
            self.local_replica,
            Some(peer.id),
            serde_json::to_vec(&discovery_payload).map_err(SyncSessionError::Serialization)?,
        );
        let discovery_response = connection
            .request_response(discovery_msg, self.config.transport_timeout)
            .await
            .map_err(SyncSessionError::Transport)?;
        if discovery_response.message_type != SyncMessageType::DiscoveryResponse {
            return Err(SyncSessionError::UnexpectedMessageType {
                expected: SyncMessageType::DiscoveryResponse,
                actual: discovery_response.message_type,
            });
        }
        let remote: DiscoveryPayload = serde_json::from_slice(&discovery_response.payload)
            .map_err(SyncSessionError::Serialization)?;

        let deltas = session
            .discover(remote.vector_clock)
            .map_err(SyncSessionError::Session)?;

        // Phase 2: Exchange. For each divergent aggregate, offer our
        // interest and receive the peer's operation batch.
        for delta in deltas {
            let offer_payload = OfferPayload {
                aggregate_id: delta.aggregate_id,
                from_version: 0, // see doc comment: AggregateDelta carries no version.
            };
            let offer_msg = new_sync_message(
                SyncMessageType::OfferOperations,
                self.organization_id,
                self.local_replica,
                Some(peer.id),
                serde_json::to_vec(&offer_payload).map_err(SyncSessionError::Serialization)?,
            );
            let batch_response = connection
                .request_response(offer_msg, self.config.transport_timeout)
                .await
                .map_err(SyncSessionError::Transport)?;
            if batch_response.message_type != SyncMessageType::OperationBatch {
                return Err(SyncSessionError::UnexpectedMessageType {
                    expected: SyncMessageType::OperationBatch,
                    actual: batch_response.message_type,
                });
            }
            let batch: OperationBatchPayload = serde_json::from_slice(&batch_response.payload)
                .map_err(SyncSessionError::Serialization)?;

            session
                .exchange_operations(batch.events)
                .map_err(SyncSessionError::Session)?;
        }

        // Phase 3: Reconciliation.
        let conflicts = session.reconcile().map_err(SyncSessionError::Session)?;

        if !conflicts.is_empty() {
            let mut records = self.open_conflicts.write().await;
            for conflict in &conflicts {
                if !records
                    .iter()
                    .any(|existing| existing.conflict_id == conflict.conflict_id)
                {
                    records.push(conflict.clone());
                }
            }
            let open_count = records
                .iter()
                .filter(|record| record.status != synchronization_domain::ConflictStatus::Resolved)
                .count();
            drop(records);
            self.status.write().await.open_conflict_count = open_count;
        }

        // Notify the peer of any conflicts this side detected.
        for conflict in &conflicts {
            let conflict_payload = ConflictPayload {
                conflict_id: conflict.conflict_id,
                field: conflict.field_path.clone(),
            };
            let conflict_msg = new_sync_message(
                SyncMessageType::ConflictNotification,
                self.organization_id,
                self.local_replica,
                Some(peer.id),
                serde_json::to_vec(&conflict_payload).map_err(SyncSessionError::Serialization)?,
            );
            connection
                .send(conflict_msg)
                .await
                .map_err(SyncSessionError::Transport)?;
        }

        // Phase 4: Completion. Persist a resumption cursor before the
        // session is consumed by `complete()`, per Team Prompt 5 §4.3's
        // session-resumption requirement.
        let _cursor: Result<SyncCursor, SyncError> = session.pause();
        // TODO(session resumption, tracked separately): persist `_cursor`
        // to local storage and feed it back into `session.resume()` on
        // the next cycle. `SynchronizationSession` has no cursor-argument
        // resume (see R2) — resumption is via the session's own internal
        // `last_cursor`, which requires keeping the *same* session
        // instance alive across a pause, not reconstructing one from a
        // saved value. This is a real gap in the session API for a
        // client that may be killed and restarted between sync cycles
        // (e.g. iOS background suspension) — flagged, not silently
        // resolved.

        Ok(())
    }
}

/// `{replica_id, vector_clock}` — `DiscoveryRequest`/`DiscoveryResponse`
/// payload shape per ruling W1.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct DiscoveryPayload {
    replica_id: ReplicaId,
    vector_clock: platform_kernel::VectorClock,
}

/// `{aggregate_id, from_version}` — `OfferOperations` payload shape per
/// ruling W1.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OfferPayload {
    aggregate_id: platform_kernel::ObjectId,
    from_version: u64,
}

/// `{events: [DomainEventEnvelope, ...]}` — `OperationBatch` payload shape
/// per ruling W1.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OperationBatchPayload {
    events: Vec<platform_contracts::DomainEventEnvelope<serde_json::Value>>,
}

/// `{conflict_id, field}` — `ConflictNotification` payload shape per
/// ruling W1.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ConflictPayload {
    conflict_id: platform_kernel::ConflictId,
    field: String,
}

fn new_sync_message(
    message_type: SyncMessageType,
    organization_id: OrganizationId,
    sender_replica: ReplicaId,
    target_replica: Option<ReplicaId>,
    payload: Vec<u8>,
) -> sync_transport::SyncMessage {
    sync_transport::SyncMessage {
        message_id: sync_transport::MessageId::new(),
        version: platform_kernel::SchemaVersion::new("1.0"),
        message_type,
        organization_id,
        sender_replica,
        target_replica,
        payload,
        timestamp: platform_kernel::Timestamp::now(),
        signature: None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyncSessionError {
    #[error("transport error: {0}")]
    Transport(TransportError),
    #[error("sync session error: {0}")]
    Session(SyncError),
    #[error("wire payload serialization error: {0}")]
    Serialization(serde_json::Error),
    #[error("unexpected message type: expected {expected:?}, got {actual:?}")]
    UnexpectedMessageType {
        expected: sync_transport::SyncMessageType,
        actual: sync_transport::SyncMessageType,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_contracts::{AuditMetadata, DataClassification, DomainEventEnvelope};
    use platform_kernel::{
        ActorContext, AuthorityEpoch, CorrelationId, DomainObjectRef, EventId, LifecycleEpoch,
        ObjectId, ObjectVersion, SchemaVersion,
    };
    use query_application::{OutboxId, OutboxMessage};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;
    use sync_transport::placeholder_types::{CloudDiscovery, LocalDiscovery};
    use sync_transport::{Connection, MessageId, SyncMessage};
    use worker_application::{ClaimedMessage, LeaseToken, OutboxError};

    /// A `Connection` double whose `request_response`/`send` behavior is
    /// scripted per-call, so `run_session_over_connection` can be tested
    /// against specific `SyncMessageType` sequences. The real
    /// `sync_transport::test_support::MockConnection` always echoes `Ack`
    /// regardless of the request (verified before writing this — see
    /// `DECISIONS.md`), which cannot exercise a protocol that expects
    /// `DiscoveryResponse`/`OperationBatch` replies, so this is a
    /// purpose-built double rather than a reuse of Team 4's.
    struct ScriptedConnection {
        /// Responses returned by `request_response`, in call order.
        responses: Mutex<Vec<SyncMessage>>,
        /// Every message passed to `send` or `request_response`, for
        /// assertions on what was actually sent.
        sent: Mutex<Vec<SyncMessage>>,
    }

    impl ScriptedConnection {
        fn new(responses: Vec<SyncMessage>) -> Self {
            Self {
                responses: Mutex::new(responses),
                sent: Mutex::new(Vec::new()),
            }
        }

        fn sent_messages(&self) -> Vec<SyncMessage> {
            self.sent.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl Connection for ScriptedConnection {
        async fn request_response(
            &mut self,
            request: SyncMessage,
            _timeout: std::time::Duration,
        ) -> Result<SyncMessage, TransportError> {
            self.sent.lock().unwrap().push(request);
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                return Err(TransportError::ConnectionLost);
            }
            Ok(responses.remove(0))
        }

        async fn send(&mut self, message: SyncMessage) -> Result<(), TransportError> {
            self.sent.lock().unwrap().push(message);
            Ok(())
        }

        async fn recv(&mut self) -> Result<SyncMessage, TransportError> {
            Err(TransportError::ConnectionLost) // Not exercised by these tests.
        }

        async fn close(&mut self) -> Result<(), TransportError> {
            Ok(())
        }

        fn transport_type(&self) -> sync_transport::TransportType {
            sync_transport::TransportType::CloudRelay
        }
    }

    fn scripted_message(message_type: SyncMessageType, payload: Vec<u8>) -> SyncMessage {
        SyncMessage {
            message_id: MessageId::new(),
            version: SchemaVersion::new("1.0"),
            message_type,
            organization_id: OrganizationId::new_random(),
            sender_replica: ReplicaId::new_random(),
            target_replica: None,
            payload,
            timestamp: platform_kernel::Timestamp::now(),
            signature: None,
        }
    }

    fn sample_agent(event_bus: Arc<EventBus>, outbox: Arc<dyn OutboxStore>) -> SyncAgent {
        SyncAgent::new(
            ReplicaId::new_random(),
            OrganizationId::new_random(),
            outbox,
            Arc::new(CompositeDiscovery::new(
                CloudDiscovery::new(),
                LocalDiscovery::new(),
            )),
            Arc::new(TransportSelector::new(
                None,
                None,
                None,
                Box::new(sync_transport::test_support::MockTransport::new(
                    sync_transport::TransportType::CloudRelay,
                    true,
                    false,
                )),
            )),
            event_bus,
            SyncAgentConfig::default(),
        )
    }

    /// An in-memory `OutboxStore` double: `claim_unpublished` hands back
    /// every seeded, not-yet-published message once (mirroring the real
    /// stores' lease semantics loosely — good enough for pump-loop tests,
    /// not a general-purpose fake).
    struct InMemoryOutbox {
        pending: Mutex<Vec<OutboxMessage>>,
        published: Mutex<Vec<OutboxId>>,
        next_id: AtomicU64,
    }

    impl InMemoryOutbox {
        fn new() -> Self {
            Self {
                pending: Mutex::new(Vec::new()),
                published: Mutex::new(Vec::new()),
                next_id: AtomicU64::new(1),
            }
        }

        fn seed_json(&self, payload: serde_json::Value) -> OutboxId {
            let id = OutboxId(self.next_id.fetch_add(1, Ordering::SeqCst));
            self.pending.lock().unwrap().push(OutboxMessage {
                event_id: EventId::new_random(),
                event_type: "TestEvent".to_string(),
                aggregate_id: "test-aggregate".to_string(),
                organization_id: OrganizationId::new_random(),
                payload: serde_json::to_vec(&payload).unwrap(),
                vector_clock: platform_kernel::VectorClock::new(),
                occurred_at: platform_kernel::Timestamp::now(),
            });
            id
        }
    }

    #[async_trait::async_trait]
    impl OutboxStore for InMemoryOutbox {
        async fn claim_unpublished(
            &self,
            _consumer_id: &str,
            limit: usize,
        ) -> Result<Vec<ClaimedMessage>, OutboxError> {
            let mut pending = self.pending.lock().unwrap();
            let n = pending.len().min(limit);
            let claimed: Vec<OutboxMessage> = pending.drain(0..n).collect();
            Ok(claimed
                .into_iter()
                .enumerate()
                .map(|(i, message)| ClaimedMessage {
                    outbox_id: OutboxId(1000 + i as u64), // distinct from seed ids; fine for these tests
                    message,
                    lease_token: LeaseToken("test-lease".to_string()),
                    retry_count: 0,
                    bus_delivered: false,
                })
                .collect())
        }

        async fn mark_published(&self, outbox_id: OutboxId) -> Result<(), OutboxError> {
            self.published.lock().unwrap().push(outbox_id);
            Ok(())
        }

        async fn mark_bus_delivered(&self, _outbox_id: OutboxId) -> Result<(), OutboxError> {
            Ok(())
        }

        async fn mark_failed(
            &self,
            _outbox_id: OutboxId,
            _error: &str,
            _retry_after: platform_kernel::Duration,
        ) -> Result<(), OutboxError> {
            Ok(())
        }

        async fn dead_letter(&self, _outbox_id: OutboxId, _error: &str) -> Result<(), OutboxError> {
            Ok(())
        }

        async fn pending_count(&self) -> Result<u64, OutboxError> {
            Ok(self.pending.lock().unwrap().len() as u64)
        }
    }

    fn sample_event_envelope() -> DomainEventEnvelope<serde_json::Value> {
        let org = OrganizationId::new_random();
        DomainEventEnvelope {
            event_id: EventId::new_random(),
            event_type: "TestHappened".to_string(),
            schema_version: SchemaVersion::new("1.0"),
            aggregate_ref: DomainObjectRef {
                id: ObjectId::new_random(),
                r#type: "test".to_string(),
                organization_id: org,
            },
            aggregate_version: ObjectVersion(1),
            lifecycle_epoch: LifecycleEpoch(0),
            authority_epoch: AuthorityEpoch(0),
            operation_id: platform_kernel::OperationId::new_random(),
            actor: ActorContext {
                user_id: ObjectId::new_random(),
                device_id: ObjectId::new_random(),
                organization_id: org,
            },
            occurred_at: platform_kernel::Timestamp::now(),
            recorded_at: platform_kernel::Timestamp::now(),
            vector_clock: platform_kernel::VectorClock::new(),
            correlation_id: CorrelationId::new_random(),
            causation_id: None,
            audit_metadata: AuditMetadata {
                source_service: "test".to_string(),
                source_replica: ReplicaId::new_random(),
                integrity_digest: None,
                classification: DataClassification::Internal,
                tenant_isolation_key: org,
            },
            payload: serde_json::json!({"key": "value"}),
        }
    }

    #[tokio::test]
    async fn full_session_with_no_divergence_and_no_conflicts_succeeds() {
        let event_bus = Arc::new(EventBus::new(16));
        let outbox = Arc::new(InMemoryOutbox::new());
        let agent = sample_agent(Arc::clone(&event_bus), outbox);
        let peer = PeerInfo::dummy();

        // Discovery response echoes back an empty vector clock, so
        // session.discover() finds no divergent aggregates and Exchange
        // is skipped entirely (no OperationBatch round-trip needed).
        let discovery_payload = DiscoveryPayload {
            replica_id: peer.id,
            vector_clock: platform_kernel::VectorClock::new(),
        };
        let mut connection = ScriptedConnection::new(vec![scripted_message(
            SyncMessageType::DiscoveryResponse,
            serde_json::to_vec(&discovery_payload).unwrap(),
        )]);

        let result = agent
            .run_session_over_connection(&peer, &mut connection)
            .await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let sent = connection.sent_messages();
        assert_eq!(
            sent.len(),
            1,
            "only the DiscoveryRequest should have been sent"
        );
        assert_eq!(sent[0].message_type, SyncMessageType::DiscoveryRequest);
    }

    #[tokio::test]
    async fn discovery_response_wrong_message_type_is_rejected() {
        let event_bus = Arc::new(EventBus::new(16));
        let outbox = Arc::new(InMemoryOutbox::new());
        let agent = sample_agent(event_bus, outbox);
        let peer = PeerInfo::dummy();

        // Peer replies with the wrong message type (Ack instead of
        // DiscoveryResponse) — must be rejected, not silently accepted.
        let mut connection =
            ScriptedConnection::new(vec![scripted_message(SyncMessageType::Ack, vec![])]);

        let result = agent
            .run_session_over_connection(&peer, &mut connection)
            .await;
        match result {
            Err(SyncSessionError::UnexpectedMessageType {
                expected: SyncMessageType::DiscoveryResponse,
                actual: SyncMessageType::Ack,
            }) => {}
            other => {
                panic!("expected UnexpectedMessageType(DiscoveryResponse, Ack), got {other:?}")
            }
        }
    }

    #[tokio::test]
    async fn exchange_offers_operations_for_each_divergent_aggregate_and_ingests_the_batch() {
        let event_bus = Arc::new(EventBus::new(16));
        let outbox = Arc::new(InMemoryOutbox::new());
        let agent = sample_agent(Arc::clone(&event_bus), outbox);
        let peer = PeerInfo::dummy();

        // Peer's vector clock has an entry our (empty) local clock lacks,
        // so session.discover() reports one divergent aggregate and
        // Exchange proceeds to offer/receive a batch for it.
        let mut remote_clock = platform_kernel::VectorClock::new();
        remote_clock.entries.insert(peer.id, 1);
        let discovery_payload = DiscoveryPayload {
            replica_id: peer.id,
            vector_clock: remote_clock,
        };

        let batch_payload = OperationBatchPayload {
            events: vec![sample_event_envelope()],
        };

        let mut connection = ScriptedConnection::new(vec![
            scripted_message(
                SyncMessageType::DiscoveryResponse,
                serde_json::to_vec(&discovery_payload).unwrap(),
            ),
            scripted_message(
                SyncMessageType::OperationBatch,
                serde_json::to_vec(&batch_payload).unwrap(),
            ),
        ]);

        let result = agent
            .run_session_over_connection(&peer, &mut connection)
            .await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let sent = connection.sent_messages();
        assert_eq!(
            sent.len(),
            2,
            "DiscoveryRequest + OfferOperations should have been sent"
        );
        assert_eq!(sent[0].message_type, SyncMessageType::DiscoveryRequest);
        assert_eq!(sent[1].message_type, SyncMessageType::OfferOperations);

        let offer: OfferPayload = serde_json::from_slice(&sent[1].payload).unwrap();
        assert_eq!(offer.from_version, 0); // documented: AggregateDelta carries no version.
    }

    #[tokio::test]
    async fn outbox_pump_publishes_claimed_messages_to_the_event_bus() {
        let event_bus = Arc::new(EventBus::new(16));
        let outbox = Arc::new(InMemoryOutbox::new());
        let org = OrganizationId::new_random();
        outbox.seed_json(serde_json::json!({
            "event_type": "MissionCreated",
            "audit_metadata": {"tenant_isolation_key": org},
        }));

        let agent = sample_agent(
            Arc::clone(&event_bus),
            Arc::clone(&outbox) as Arc<dyn OutboxStore>,
        );
        let mut sub = event_bus.subscribe(crate::event_bus::EventFilter {
            organization_id: org,
            event_types: None,
        });

        // Run one iteration of the pump loop's body directly (not the
        // full `loop { ... sleep ... }`, which never returns) by claiming
        // and publishing once, matching what `run_outbox_pump` does per
        // cycle.
        let claimed = outbox.claim_unpublished("test", 10).await.unwrap();
        for msg in &claimed {
            let value: serde_json::Value = serde_json::from_slice(&msg.message.payload).unwrap();
            event_bus.publish(value);
            outbox.mark_published(msg.outbox_id).await.unwrap();
        }

        let received = tokio::time::timeout(std::time::Duration::from_secs(1), sub.recv())
            .await
            .expect("recv timed out")
            .expect("subscription closed");
        assert_eq!(received["event_type"], serde_json::json!("MissionCreated"));
        assert_eq!(outbox.pending_count().await.unwrap(), 0);

        // agent constructed but its background loops never started in
        // this test (no `.run()` call) — referenced only to prove the
        // agent itself constructs successfully with these dependencies.
        let _ = agent.status().await;
    }
}

#[cfg(test)]
mod pump_tests {
    use super::*;
    use crate::cloud_outbox::{CloudOutboxUploader, UploadError, UploadItem};
    use platform_kernel::{EventId, ObjectId, Timestamp, VectorClock};
    use query_application::{OutboxId, OutboxMessage};
    use std::sync::Mutex;
    use sync_transport::placeholder_types::{CloudDiscovery, LocalDiscovery};
    use worker_application::{ClaimedMessage, LeaseToken, OutboxError};

    #[derive(Default)]
    struct RecordingOutbox {
        events: Mutex<Vec<String>>,
    }

    impl RecordingOutbox {
        fn log(&self, entry: String) {
            self.events.lock().unwrap().push(entry);
        }
        fn entries(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl OutboxStore for RecordingOutbox {
        async fn claim_unpublished(
            &self,
            _: &str,
            _: usize,
        ) -> Result<Vec<ClaimedMessage>, OutboxError> {
            Ok(Vec::new())
        }
        async fn mark_published(&self, id: OutboxId) -> Result<(), OutboxError> {
            self.log(format!("published:{}", id.0));
            Ok(())
        }
        async fn mark_bus_delivered(&self, id: OutboxId) -> Result<(), OutboxError> {
            self.log(format!("bus:{}", id.0));
            Ok(())
        }
        async fn mark_failed(
            &self,
            id: OutboxId,
            _: &str,
            retry_after: platform_kernel::Duration,
        ) -> Result<(), OutboxError> {
            self.log(format!("failed:{}:{}", id.0, retry_after.0));
            Ok(())
        }
        async fn dead_letter(&self, id: OutboxId, reason: &str) -> Result<(), OutboxError> {
            self.log(format!("dead:{}:{}", id.0, reason));
            Ok(())
        }
        async fn pending_count(&self) -> Result<u64, OutboxError> {
            Ok(0)
        }
    }

    struct FakeUploader {
        response: Mutex<Result<Vec<UploadAck>, UploadError>>,
        uploads: Mutex<usize>,
    }

    #[async_trait::async_trait]
    impl CloudOutboxUploader for FakeUploader {
        async fn upload(&self, _items: &[UploadItem]) -> Result<Vec<UploadAck>, UploadError> {
            *self.uploads.lock().unwrap() += 1;
            self.response.lock().unwrap().clone()
        }
    }

    fn agent(outbox: Arc<RecordingOutbox>, uploader: Option<Arc<FakeUploader>>) -> SyncAgent {
        let agent = SyncAgent::new(
            ReplicaId::new_random(),
            OrganizationId::new_random(),
            outbox,
            Arc::new(CompositeDiscovery::new(
                CloudDiscovery::new(),
                LocalDiscovery::new(),
            )),
            Arc::new(TransportSelector::new(
                None,
                None,
                None,
                Box::new(sync_transport::test_support::MockTransport::new(
                    sync_transport::TransportType::CloudRelay,
                    true,
                    false,
                )),
            )),
            Arc::new(EventBus::new(16)),
            SyncAgentConfig::default(),
        );
        match uploader {
            Some(u) => agent.with_cloud_uploader(u),
            None => agent,
        }
    }

    fn row(id: u64, payload: &[u8], bus_delivered: bool, retry_count: u32) -> ClaimedMessage {
        ClaimedMessage {
            outbox_id: OutboxId(id),
            message: OutboxMessage {
                event_id: EventId::new_random(),
                event_type: "TestEvent".to_string(),
                aggregate_id: "agg".to_string(),
                organization_id: ObjectId::new_random(),
                payload: payload.to_vec(),
                vector_clock: VectorClock::new(),
                occurred_at: Timestamp::now(),
            },
            lease_token: LeaseToken("l".to_string()),
            retry_count,
            bus_delivered,
        }
    }

    fn uploader(response: Result<Vec<UploadAck>, UploadError>) -> Arc<FakeUploader> {
        Arc::new(FakeUploader {
            response: Mutex::new(response),
            uploads: Mutex::new(0),
        })
    }

    #[tokio::test]
    async fn stored_ack_marks_row_done_and_delivers_to_bus_once() {
        let outbox = Arc::new(RecordingOutbox::default());
        let up = uploader(Ok(vec![UploadAck::Stored { outbox_id: 1 }]));
        let a = agent(Arc::clone(&outbox), Some(Arc::clone(&up)));
        a.process_claimed(vec![row(1, b"{\"k\":1}", false, 0)])
            .await;
        assert_eq!(outbox.entries(), vec!["bus:1", "published:1"]);
        assert_eq!(*up.uploads.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn offline_failure_keeps_row_pending_without_counting_attempts() {
        let outbox = Arc::new(RecordingOutbox::default());
        let up = uploader(Err(UploadError::Unreachable("offline".into())));
        let a = agent(Arc::clone(&outbox), Some(up));
        a.process_claimed(vec![row(2, b"{}", true, 3)]).await;
        // Already on the bus, so no second bus publish; no done, retry or dead-letter.
        assert!(outbox.entries().is_empty(), "got {:?}", outbox.entries());
    }

    #[tokio::test]
    async fn rejected_non_retryable_is_dead_lettered() {
        let outbox = Arc::new(RecordingOutbox::default());
        let up = uploader(Ok(vec![UploadAck::Rejected {
            outbox_id: 3,
            retryable: false,
            reason: "bad schema".into(),
        }]));
        let a = agent(Arc::clone(&outbox), Some(up));
        a.process_claimed(vec![row(3, b"{}", true, 0)]).await;
        assert_eq!(
            outbox.entries(),
            vec!["dead:3:rejected by cloud: bad schema"]
        );
    }

    #[tokio::test]
    async fn retryable_rejection_backs_off_then_dead_letters_at_limit() {
        let outbox = Arc::new(RecordingOutbox::default());
        let rejected = || {
            uploader(Ok(vec![UploadAck::Rejected {
                outbox_id: 4,
                retryable: true,
                reason: "busy".into(),
            }]))
        };

        // First failure: retry with 2^1 s backoff.
        let a = agent(Arc::clone(&outbox), Some(rejected()));
        a.process_claimed(vec![row(4, b"{}", true, 0)]).await;
        assert_eq!(
            outbox.entries(),
            vec![format!("failed:4:{}", 2_000_000_000u64)]
        );

        // Tenth attempt (retry_count 9) is the limit: dead-letter.
        let outbox2 = Arc::new(RecordingOutbox::default());
        let a2 = agent(Arc::clone(&outbox2), Some(rejected()));
        a2.process_claimed(vec![row(4, b"{}", true, 9)]).await;
        assert_eq!(
            outbox2.entries(),
            vec!["dead:4:cloud retries exhausted: busy"]
        );
    }

    #[tokio::test]
    async fn malformed_payload_is_dead_lettered_and_not_uploaded() {
        let outbox = Arc::new(RecordingOutbox::default());
        let up = uploader(Ok(Vec::new()));
        let a = agent(Arc::clone(&outbox), Some(Arc::clone(&up)));
        a.process_claimed(vec![row(5, b"not json", false, 0)]).await;
        let entries = outbox.entries();
        assert_eq!(entries.len(), 1);
        assert!(
            entries[0].starts_with("dead:5:malformed payload"),
            "{entries:?}"
        );
        assert_eq!(*up.uploads.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn without_uploader_rows_stay_pending_after_one_bus_delivery() {
        let outbox = Arc::new(RecordingOutbox::default());
        let a = agent(Arc::clone(&outbox), None);
        a.process_claimed(vec![row(6, b"{}", false, 0)]).await;
        assert_eq!(outbox.entries(), vec!["bus:6"]);
    }
}
