//! Aggregate root trait and decision context.
//! Source: Handover Document §3 "AGGREGATE ROOT TRAIT" (lines 291-358).
//!
//! Per Architectural Ruling (Team 2 Kickoff, item 1): implemented as a trait
//! with associated types (not a struct-level generic `A`), matching the
//! Handover's own method signatures (`Self::Id`, `Self::Command`, `Self::Event`,
//! `Self::Error`).

use crate::authority::{ActorContext, PolicyDecisionSet, VerifiedAuthority};
use crate::identifiers::{EventId, ObjectId, OperationId};
use crate::time::Timestamp;
use crate::versioning::{AuthorityEpoch, LifecycleEpoch, ObjectVersion};

/// Generates fresh identifiers deterministically-injectable at the decision
/// boundary (so `decide()` remains pure / reproducible in tests).
/// Per Architectural Ruling (Team 2 Initiation).
pub trait IdGenerator: Send + Sync {
    fn generate_object_id(&self) -> ObjectId;
    fn generate_operation_id(&self) -> OperationId;
    fn generate_event_id(&self) -> EventId;
}

/// Context passed to `AggregateRoot::decide()`.
pub struct DecisionContext<'a> {
    pub actor: ActorContext,
    pub authority: VerifiedAuthority,
    pub trusted_now: Timestamp,
    pub policy_outcomes: PolicyDecisionSet,
    pub generated_id_generator: &'a dyn IdGenerator,
}

/// Consistency boundary that accepts commands and protects invariants.
///
/// `decide()` MUST be pure: no I/O, no repository access, no side effects,
/// deterministic for identical inputs.
///
/// `apply()` MUST be effectively pure: no validation, no I/O, idempotent —
/// it is called both during rehydration (replay) and immediately after
/// `decide()` to produce the aggregate's new in-memory state.
pub trait AggregateRoot: Sized {
    type Id;
    type Command;
    type Event;
    type Error;

    fn id(&self) -> &Self::Id;
    fn version(&self) -> ObjectVersion;
    fn lifecycle_epoch(&self) -> LifecycleEpoch;
    fn authority_epoch(&self) -> AuthorityEpoch;

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext<'_>,
    ) -> Result<Vec<Self::Event>, Self::Error>;

    fn apply(&mut self, event: &Self::Event);
}
