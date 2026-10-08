//! Escalation Service (Event-Sourced).
//! Source: Team Prompt 3 §3.13, rebuilt per Architectural Ruling Q3 (Team 3
//! Initiation — Contract Reconciliation).
//!
//! ## Q3 ruling and how it's actually implemented here
//!
//! Q3's ruling (Option A, recommended) says escalation should "trigger a
//! command" processed through the normal command pipeline
//! (`CommandEnvelope` → handler → `UnitOfWork::register_outbox()`), rather
//! than a direct `OutboxStore::enqueue()` call that doesn't exist.
//!
//! Two things were discovered while implementing this that the ruling's
//! sketch didn't anticipate, and are recorded here rather than silently
//! worked around:
//!
//! 1. **The real command pipeline (`command_handler::handle_command`) is
//!    generic over a concrete `AggregateRoot` with a `decide()`/`apply()`
//!    pair.** Escalation is not "a command against an existing aggregate's
//!    state machine" — there is no `Mission`/`Task`-like aggregate whose
//!    `decide()` should produce an `EscalateConflict` event. Routing
//!    escalation through `handle_command<A, C, E, Err>` would require
//!    fabricating a fake aggregate type with no real state, purely to
//!    satisfy a generic signature that assumes one exists. That is exactly
//!    the kind of invented contract-shape this project's rules prohibit.
//!
//! 2. **`Repository::commit()`'s real, delivered implementation
//!    (`persistence-sqlite`/`persistence-postgres`) only calls
//!    `unit.register_event(...)` — it never calls
//!    `unit.register_outbox(...)`.** `UnitOfWork::register_outbox()` exists
//!    on the port trait, but nothing in the delivered Increment 2 code path
//!    calls it. So even the literal command pipeline does not currently
//!    deliver outbox messages for *any* event, escalation or otherwise —
//!    this is a pre-existing gap in the delivered system, not something
//!    Team 3 introduced or can fix from here (fixing it would mean editing
//!    Team 1/2's persistence adapters, out of this increment's scope).
//!
//! **What this implementation actually does**, staying as close to Option
//! A's intent (durable, transactional, authority-respecting write; no
//! direct network call) as the real delivered ports allow: `escalate_conflict`
//! constructs a `DomainEventEnvelope<ConflictEscalationRequested>`
//! (mirroring exactly what `command_handler::handle_command` step 8 does
//! for a normal domain event) and commits it via
//! `UnitOfWorkFactory::create()` + `Repository::commit()` — the same
//! durable, transactional path every other domain event in this system
//! goes through. This guarantees the escalation event is durably recorded
//! exactly once per call (no direct network I/O, no partition-sensitive
//! delivery). Outbox *publication* of that event remains contingent on
//! Increment 2's `register_outbox()` gap being closed — flagged in
//! `DECISIONS.md` as a cross-increment follow-up, not silently assumed to
//! work.

use std::sync::Arc;

use platform_contracts::{AuditMetadata, DataClassification, DomainEventEnvelope};
use platform_kernel::{
    ActorContext, AuthorityEpoch, CorrelationId, DomainObjectRef, EventId, LifecycleEpoch,
    ObjectVersion, OperationId, OrganizationId, ReplicaId, SchemaVersion, Timestamp,
};
use query_application::{Repository, UnitOfWorkFactory};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::conflict::ConflictRecord;

/// Escalates open conflicts by durably recording an escalation event
/// through the same transactional write path every other domain event
/// uses. See module doc comment for exactly what guarantee this provides
/// and what it does not (yet) provide.
pub struct EscalationService {
    repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    policy_config: PolicyConfig,
}

/// Per-organization escalation policy.
#[derive(Clone)]
pub struct PolicyConfig {
    /// How long a conflict may remain open before it's eligible for
    /// escalation. Per-org configurable. Nanoseconds, matching
    /// `platform_kernel::Timestamp`'s unit.
    pub escalation_window_nanos: u64,
}

impl EscalationService {
    /// Build a service that durably records escalation events via `repo`/
    /// `unit_factory`, gated by `config`.
    pub fn new(
        repo: Arc<dyn Repository>,
        unit_factory: Arc<dyn UnitOfWorkFactory>,
        config: PolicyConfig,
    ) -> Self {
        Self {
            repo,
            unit_factory,
            policy_config: config,
        }
    }

    /// Called when a ConflictRecord has been open for longer than the window.
    pub async fn escalate_conflict(
        &self,
        conflict: &ConflictRecord,
    ) -> Result<(), EscalationError> {
        let event = ConflictEscalationRequested {
            conflict_id: conflict.conflict_id,
            aggregate_id: conflict.aggregate_id,
            created_at: Timestamp::now(),
        };

        let organization_id: OrganizationId = conflict.organization_id;
        let now = Timestamp::now();

        // Mirrors command_handler::handle_command's step 8 event
        // construction exactly, for the reasons in the module doc comment:
        // this is a real domain event, going through the real event
        // envelope shape, not a bespoke escalation-only wire format.
        let envelope = DomainEventEnvelope {
            event_id: EventId::new_random(),
            event_type: "synchronization.ConflictEscalationRequested".to_string(),
            schema_version: SchemaVersion::new("1.0"),
            aggregate_ref: DomainObjectRef {
                id: conflict.aggregate_id,
                r#type: "synchronization_conflict".to_string(),
                organization_id,
            },
            aggregate_version: ObjectVersion(0),
            lifecycle_epoch: LifecycleEpoch(0),
            authority_epoch: AuthorityEpoch(0),
            operation_id: OperationId::new_random(),
            actor: system_actor(organization_id),
            occurred_at: now,
            recorded_at: now,
            vector_clock: conflict.local_operation.vector_clock.clone(),
            correlation_id: CorrelationId::new_random(),
            causation_id: None,
            audit_metadata: AuditMetadata {
                source_service: "synchronization-domain".to_string(),
                source_replica: ReplicaId::new_random(),
                integrity_digest: None,
                classification: DataClassification::Internal,
                tenant_isolation_key: organization_id,
            },
            payload: event,
        };

        let event_value =
            serde_json::to_value(&envelope).map_err(|_| EscalationError::Serialization)?;

        let mut unit = self
            .unit_factory
            .create(organization_id)
            .await
            .map_err(|e| EscalationError::Persistence(e.to_string()))?;

        // No prior aggregate state changes — this write only registers the
        // escalation event itself. `commit()` requires an aggregate_state
        // value; the conflict's own aggregate_id is echoed back as a
        // minimal, self-describing marker rather than left empty, since
        // `commit()` implementations key staged writes off it.
        let marker_state = serde_json::json!({ "id": conflict.aggregate_id });

        self.repo
            .commit(marker_state, &[event_value], &mut *unit)
            .await
            .map_err(|e| EscalationError::Persistence(e.to_string()))?;

        unit.commit()
            .await
            .map_err(|e| EscalationError::Persistence(e.to_string()))?;

        Ok(())
    }

    /// Periodic check: scan for open conflicts past their window.
    pub async fn scan_and_escalate(
        &self,
        conflict_repo: Arc<dyn ConflictRepository>,
    ) -> Result<usize, EscalationError> {
        let now = Timestamp::now();
        let since = Timestamp::from_nanos(
            now.0
                .saturating_sub(self.policy_config.escalation_window_nanos),
        );
        let expired = conflict_repo.find_open_older_than(since).await?;

        let mut count = 0;
        for conflict in expired {
            if let Err(e) = self.escalate_conflict(&conflict).await {
                tracing::error!(
                    "Failed to escalate conflict {:?}: {}",
                    conflict.conflict_id,
                    e
                );
            } else {
                count += 1;
            }
        }
        Ok(count)
    }
}

/// Escalation is raised by the synchronization engine itself, not by a
/// human actor — this constructs the system's own `ActorContext` rather
/// than requiring a caller to supply a fabricated user/device identity.
/// Not defined by Team Prompt 3; a minimal, explicit choice rather than a
/// silently invented one.
fn system_actor(organization_id: OrganizationId) -> ActorContext {
    ActorContext {
        user_id: OrganizationId::default(),
        device_id: OrganizationId::default(),
        organization_id,
    }
}

/// The event recorded when a conflict is escalated.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictEscalationRequested {
    /// The conflict being escalated.
    pub conflict_id: platform_kernel::ConflictId,
    /// The aggregate the conflict belongs to.
    pub aggregate_id: platform_kernel::ObjectId,
    /// When the escalation event was created.
    pub created_at: Timestamp,
}

/// Errors that can occur while escalating a conflict.
#[derive(Error, Debug, Clone)]
pub enum EscalationError {
    /// The escalation event failed to serialize.
    #[error("failed to serialize escalation event")]
    Serialization,
    /// The durable write path (UnitOfWork/Repository) failed.
    #[error("persistence error: {0}")]
    Persistence(String),
    /// The conflict repository failed while scanning for expired conflicts.
    #[error("conflict repository error: {0}")]
    Repository(String),
}

impl From<ConflictRepositoryError> for EscalationError {
    fn from(e: ConflictRepositoryError) -> Self {
        EscalationError::Repository(e.to_string())
    }
}

/// Not defined in Team Prompt 3 (only referenced via `Arc<dyn
/// ConflictRepository>` in §3.13/§5.2). Minimal port needed for
/// `scan_and_escalate` / `MockConflictRepository` (§5.2) to compile.
#[async_trait::async_trait]
pub trait ConflictRepository: Send + Sync {
    /// Return all open conflicts created before `since`.
    async fn find_open_older_than(
        &self,
        since: Timestamp,
    ) -> Result<Vec<ConflictRecord>, ConflictRepositoryError>;
}

/// Errors that can occur while querying the conflict repository.
#[derive(Error, Debug, Clone)]
pub enum ConflictRepositoryError {
    /// The underlying store failed.
    #[error("conflict repository failure: {0}")]
    Store(String),
}
