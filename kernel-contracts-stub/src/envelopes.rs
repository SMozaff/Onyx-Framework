//! Command & event envelopes.
//! Source: Handover Document §2 "envelopes" (lines 209-285).
//!
//! IMPORTANT DEVIATION FROM THE RAW HANDOVER TEXT:
//! The Handover Document marks `CommandEnvelope` and `DomainEventEnvelope` as
//! `generic: "T"` (i.e. `CommandEnvelope<T>`, `DomainEventEnvelope<T>`).
//! Per Architectural Ruling 3a (Team 2 Kickoff) and reaffirmed in the Team 2
//! Initiation ruling, `UnitOfWork` must be object-safe as `&mut dyn UnitOfWork`,
//! which requires it to operate on a *concrete* event envelope type rather than
//! a generic one. The ruling resolves this by having `DomainEventEnvelope` carry
//! its payload as `serde_json::Value` (already implied by the envelope's own
//! "JSON with base64-encoded payload where binary" serialization rule) instead
//! of a generic `T`. See DECISIONS.md, item 1.
//!
//! `CommandEnvelope` is retained as generic because it is constructed and
//! consumed entirely within the command-handling call stack (never stored
//! behind `dyn`), so genericity does not break object-safety anywhere it is
//! used in Team Prompt 2.

use crate::authority::{ActorContext, AuthorityProof, DataClassification};
use crate::causality::VectorClock;
use crate::domain_references::DomainObjectRef;
use crate::identifiers::{CommandId, CorrelationId, EventId, OperationId, OrganizationId};
use crate::time::Timestamp;
use crate::versioning::{AuthorityEpoch, LifecycleEpoch, ObjectVersion, ReplicaId, SchemaVersion};
use serde::{Deserialize, Serialize};

/// A command request directed at a specific aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEnvelope<T> {
    pub command_id: CommandId,
    pub operation_id: OperationId,
    pub command_type: String,
    pub schema_version: SchemaVersion,
    pub target: DomainObjectRef,
    pub expected_version: ObjectVersion,
    pub expected_lifecycle_epoch: LifecycleEpoch,
    pub expected_authority_epoch: AuthorityEpoch,
    pub actor: ActorContext,
    pub authority_proof: AuthorityProof,
    pub issued_at: Timestamp,
    pub vector_clock: VectorClock,
    pub correlation_id: CorrelationId,
    pub causation_id: Option<EventId>,
    pub payload: T,
}

/// Provenance/attribution metadata attached to every domain event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditMetadata {
    pub source_service: String,
    pub source_replica: ReplicaId,
    pub integrity_digest: Option<Vec<u8>>,
    pub classification: DataClassification,
    pub tenant_isolation_key: OrganizationId,
}

/// A domain event, as committed and transported across the system.
///
/// NOTE: `payload` is `serde_json::Value` rather than a generic `T`.
/// See module-level doc comment and DECISIONS.md item 1 for rationale
/// (object-safety of `UnitOfWork`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEventEnvelope {
    pub event_id: EventId,
    pub event_type: String,
    pub schema_version: SchemaVersion,
    pub aggregate_ref: DomainObjectRef,
    pub aggregate_version: ObjectVersion,
    pub lifecycle_epoch: LifecycleEpoch,
    pub authority_epoch: AuthorityEpoch,
    pub operation_id: OperationId,
    pub actor: ActorContext,
    pub occurred_at: Timestamp,
    pub recorded_at: Timestamp,
    pub vector_clock: VectorClock,
    pub correlation_id: CorrelationId,
    pub causation_id: Option<EventId>,
    pub audit_metadata: AuditMetadata,
    pub payload: serde_json::Value,
}

/// Errors from the domain layer; infrastructure-free.
/// Source: Handover Document §10 "error_protocols" (lines 1188-1211).
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum DomainError {
    #[error("invalid transition from {from} via command {command}")]
    InvalidTransition { from: String, command: String },
    #[error("unauthorized")]
    Unauthorized,
    #[error("version conflict: expected {expected:?}, actual {actual:?}")]
    VersionConflict {
        expected: ObjectVersion,
        actual: ObjectVersion,
    },
    #[error("epoch conflict: expected {expected:?}, actual {actual:?}")]
    EpochConflict {
        expected: LifecycleEpoch,
        actual: LifecycleEpoch,
    },
    #[error("not found: {id:?}")]
    NotFound { id: crate::identifiers::ObjectId },
    #[error("already exists: {id:?}")]
    AlreadyExists { id: crate::identifiers::ObjectId },
    #[error("invalid argument: {field}: {reason}")]
    InvalidArgument { field: String, reason: String },
    #[error("conflict pending: {conflict_id:?}")]
    ConflictPending {
        conflict_id: crate::identifiers::ConflictId,
    },
}

/// Result of executing a command, returned to the API layer and cached
/// for idempotent replay by OperationId.
///
/// Defined per Architectural Ruling (Team 2 Initiation) — not present
/// verbatim in the Handover Document; only referenced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub operation_id: OperationId,
    pub new_version: Option<ObjectVersion>,
    pub new_lifecycle_epoch: Option<LifecycleEpoch>,
    pub new_authority_epoch: Option<AuthorityEpoch>,
    pub events: Vec<DomainEventEnvelope>,
    pub result: Option<serde_json::Value>,
    pub error: Option<DomainError>,
}
