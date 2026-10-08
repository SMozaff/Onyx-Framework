//! ConflictRecord & ConflictPending.
//! Source: Team Prompt 3 §3.12, rebuilt against Team 1's real
//! `platform-kernel`/`platform-contracts` (Architectural Ruling Q1, Team 3
//! Initiation).
//!
//! Deviation: the frozen contract's `local_operation`/`remote_operation`
//! were typed as a single concrete `DomainEventEnvelope`. The real
//! `platform_contracts::DomainEventEnvelope<E>` is generic over its payload
//! type `E`. `ConflictRecord` fixes `E = serde_json::Value` (rather than
//! being generic itself), matching how the real command/repository
//! pipeline already treats events as opaque `serde_json::Value` once they
//! cross the aggregate boundary (see `command_handler.rs` step 8: events
//! are serialized to `Value` before this point) — a `ConflictRecord` is
//! always constructed downstream of that serialization, so it never sees a
//! concretely-typed event.

use platform_contracts::DomainEventEnvelope;
use platform_kernel::{ConflictId, ObjectId, OrganizationId, Timestamp};
use serde::{Deserialize, Serialize};

/// A single field-level conflict between two concurrent operations on an
/// authority-controlled (or otherwise non-auto-mergeable) field.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictRecord {
    /// Unique identifier for this conflict.
    pub conflict_id: ConflictId,
    /// The aggregate whose field is in conflict.
    pub aggregate_id: ObjectId,
    /// Which field on the aggregate is in conflict.
    pub field_path: String,
    /// The operation accepted locally.
    pub local_operation: DomainEventEnvelope<serde_json::Value>,
    /// The concurrent operation received from the remote replica.
    pub remote_operation: DomainEventEnvelope<serde_json::Value>,
    /// Current lifecycle status of this conflict.
    pub status: ConflictStatus,
    /// When this conflict was first detected.
    pub created_at: Timestamp,
    /// When this conflict was resolved, if it has been.
    pub resolved_at: Option<Timestamp>,
    /// How this conflict was resolved, if it has been.
    pub resolution: Option<ConflictResolution>,
    /// Tenant/organization this conflict belongs to.
    pub organization_id: OrganizationId,
}

/// Lifecycle status of a `ConflictRecord`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictStatus {
    /// Newly detected, awaiting resolution.
    Open,
    /// A human or automated process is actively reviewing it.
    UnderReview,
    /// Resolved; see `ConflictRecord::resolution`.
    Resolved,
}

/// How a `ConflictRecord` was resolved.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// The local operation's value was kept.
    LocalWins,
    /// The remote operation's value was kept.
    RemoteWins,
    /// A custom merged value was produced (serialized bytes).
    CustomMerge {
        /// Serialized new value.
        value: Vec<u8>,
    },
    /// The conflict was escalated to a human for out-of-band resolution.
    Escalated,
}

/// Re-exported from `platform_contracts_ext`: see that crate's doc comment
/// (Architectural Ruling Q2, Team 3 Initiation) for why this lives there
/// rather than being independently defined here. A duplicate,
/// independently-defined `ConflictPendingMarker` in this crate would be
/// exactly the kind of drift-prone duplication flagged in Team 3 v1's own
/// defect report.
pub use platform_contracts_ext::ConflictPendingMarker;

impl ConflictRecord {
    /// Create a new, open conflict between `local_op` and `remote_op` on
    /// `field_path` of the given aggregate.
    pub fn new(
        aggregate_id: ObjectId,
        field_path: String,
        local_op: DomainEventEnvelope<serde_json::Value>,
        remote_op: DomainEventEnvelope<serde_json::Value>,
        org_id: OrganizationId,
    ) -> Self {
        Self {
            conflict_id: ConflictId(*uuid::Uuid::new_v4().as_bytes()),
            aggregate_id,
            field_path,
            local_operation: local_op,
            remote_operation: remote_op,
            status: ConflictStatus::Open,
            created_at: Timestamp::now(),
            resolved_at: None,
            resolution: None,
            organization_id: org_id,
        }
    }

    /// Mark this conflict resolved with the given resolution, stamping
    /// `resolved_at` to now.
    pub fn resolve(&mut self, resolution: ConflictResolution) {
        self.status = ConflictStatus::Resolved;
        self.resolution = Some(resolution);
        self.resolved_at = Some(Timestamp::now());
    }
}
