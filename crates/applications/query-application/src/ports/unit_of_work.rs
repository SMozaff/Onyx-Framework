//! UnitOfWork port.
//! Source: Team Prompt 2 §3.2, corrected by Architectural Ruling (Team 2
//! Kickoff, item 3a), refined by "Clarification — UnitOfWork Port Trait"
//! and "Ruling: Connection Trait Shape", then FINALIZED by "Ruling — Team
//! 1/Team 2 Interface Reconciliation" once Team 1's real
//! `platform-contracts::DomainEventEnvelope<E>` (genuinely generic, per
//! the frozen Handover Document) was available.
//!
//! Final resolution: `register_event` and `register_idempotency_result`
//! accept `serde_json::Value` — a serialized, type-erased envelope/result —
//! rather than a concrete non-generic struct. The caller (command handler,
//! §6.1) is responsible for constructing the typed
//! `platform_contracts::DomainEventEnvelope<E>` / `CommandResult`-shaped
//! value and calling `serde_json::to_value(...)` on it before registering.
//! This is what preserves `UnitOfWork`'s object-safety (`&mut dyn
//! UnitOfWork`, used in §3.1 and §6.1) without constraining `E` to a single
//! concrete type across all aggregates.

use crate::ports::connection::Connection;
use async_trait::async_trait;
use platform_kernel::{EventId, OperationId, OrganizationId, Timestamp, VectorClock};

#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// The tenant boundary this UnitOfWork's writes belong to. Per
    /// Architectural Ruling ("Organization ID in Persistence" — DECISIONS.md
    /// O1): `Mission`/`Task` aggregates carry no `organization_id` field of
    /// their own (organization is a tenant boundary at the command-envelope
    /// level, not a domain property of the aggregate), so the UnitOfWork —
    /// constructed with the organization_id from `CommandEnvelope.actor.
    /// organization_id` — is the source of truth for populating the
    /// `aggregates.organization_id` column at commit time.
    fn organization_id(&self) -> OrganizationId;

    /// Register a domain event to be committed. Returns the event's
    /// assigned ID.
    ///
    /// `event` is a serialized `platform_contracts::DomainEventEnvelope<E>`
    /// (via `serde_json::to_value`), for whatever concrete event type `E`
    /// the calling aggregate produced. The caller constructs the full
    /// envelope (event_id, recorded_at, etc. already set) before calling.
    fn register_event(&mut self, event: serde_json::Value) -> EventId;

    /// Register an outbox message (integration event) to be published after commit.
    fn register_outbox(&mut self, message: OutboxMessage) -> OutboxId;

    /// Register the idempotency result (for duplicate command detection).
    ///
    /// `result` is a serialized `CommandResult`-shaped value (via
    /// `serde_json::to_value`) — see module doc for why this is `Value`
    /// rather than a concrete struct.
    fn register_idempotency_result(&mut self, operation_id: OperationId, result: serde_json::Value);

    /// Commit the transaction atomically.
    /// Writes: aggregate state, domain events, outbox rows, idempotency record.
    async fn commit(&mut self) -> Result<(), UnitOfWorkError>;

    /// Rollback the transaction (if not yet committed).
    async fn rollback(&mut self) -> Result<(), UnitOfWorkError>;

    /// Returns a type-erased connection handle that the repository can downcast.
    fn connection(&self) -> &dyn Connection;
}

/// Creates `UnitOfWork` instances. Per Architectural Ruling (Team 2 Kickoff,
/// item 4): referenced in Team Prompt 2 §6.1 but never defined anywhere;
/// this is our own port, defined here.
///
/// Takes `organization_id` per Ruling O1: the caller (command handler)
/// extracts it from `CommandEnvelope.actor.organization_id` and passes it
/// through so the created `UnitOfWork` can populate `aggregates.
/// organization_id` at commit time without the aggregate itself needing to
/// carry that field.
#[async_trait]
pub trait UnitOfWorkFactory: Send + Sync {
    async fn create(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Box<dyn UnitOfWork>, UnitOfWorkError>;
}

#[derive(Debug, Clone)]
pub struct OutboxMessage {
    pub event_id: EventId,
    pub event_type: String,
    pub aggregate_id: String,
    pub organization_id: OrganizationId,
    /// Serialized event (JSON or binary).
    pub payload: Vec<u8>,
    pub vector_clock: VectorClock,
    pub occurred_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutboxId(pub u64);

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UnitOfWorkError {
    #[error("transaction commit failed: {0}")]
    CommitFailed(String),
    #[error("transaction rollback failed")]
    RollbackFailed,
    #[error("event registration failed: {0}")]
    EventRegistration(String),
}
