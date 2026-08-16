//! The `AggregateRoot` trait and its supporting `DecisionContext`.
//!
//! # Import Path Ruling
//! The frozen Team Prompt (§3.9) writes `use crate::versioning::...`,
//! `crate::authority::...`, `crate::time::...`, and `crate::id_generation::...`
//! — but those modules live in `platform-kernel`, not in this crate
//! (`platform-contracts`). Per ruling, this is a typo in the frozen prompt;
//! corrected to `platform_kernel::...` below.

use platform_kernel::{
    ActorContext, AuthorityEpoch, ConflictId, LifecycleEpoch, ObjectId, ObjectVersion, OperationId,
    PolicyDecisionSet, Timestamp, VerifiedAuthority,
};

/// Generates new identifiers on demand during `decide()`. Implementations
/// are infrastructure-free (no network, no database) — typically backed by
/// `uuid::Uuid::new_v4()` in production and a deterministic counter in
/// tests.
pub trait IdGenerator: Send + Sync {
    /// Generates a new object identifier.
    fn generate_object_id(&self) -> ObjectId;
    /// Generates a new operation identifier.
    fn generate_operation_id(&self) -> OperationId;
    /// Generates a new event identifier.
    fn generate_event_id(&self) -> platform_kernel::EventId;
}

/// Context passed to `decide()`. Contains resolved authority and trusted
/// time so that `decide()` itself never needs to perform I/O.
pub struct DecisionContext {
    /// The actor issuing the command.
    pub actor: ActorContext,
    /// The verified authority resolved for this actor/command. Stubbed in
    /// Increment 1 (see [`VerifiedAuthority`] docs); full policy evaluation
    /// lands in Increment 7.
    pub authority: VerifiedAuthority,
    /// The trusted current time, resolved once per command by the caller
    /// so that `decide()` remains deterministic and testable.
    pub trusted_now: Timestamp,
    /// The outcome of any external policy evaluation (rate limits, quotas).
    /// Stubbed in Increment 1; full implementation lands in Increment 7.
    pub policy_outcomes: PolicyDecisionSet,
    /// Generates new identifiers needed while deciding (e.g. a new
    /// blueprint revision id).
    pub generated_id_generator: Box<dyn IdGenerator>,
}

/// Marks that an aggregate has an unresolved merge conflict and cannot
/// accept further mutating commands until it's resolved.
///
/// Added per Architectural Ruling Q2 (Team 3 Initiation — Contract
/// Reconciliation): not present anywhere in Team 1's delivered
/// `platform-contracts`, nor referenced by `DomainError::ConflictPending`
/// (which only carries a bare `ConflictId`). Full semantics belong to
/// `synchronization-domain::ConflictRecord` (Increment 3); this marker is
/// the minimal read-only projection of that record's existence that the
/// kernel-level trait needs to expose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConflictPendingMarker {
    /// The unresolved conflict blocking this aggregate.
    pub conflict_id: ConflictId,
    /// When the conflict was detected.
    pub since: Timestamp,
}

/// The core trait that every aggregate implements. All business rules,
/// invariants, and state transitions live here.
pub trait AggregateRoot: Send + Sync {
    /// The aggregate's identity type.
    type Id: Clone + PartialEq + Eq + std::fmt::Debug + Send + Sync;
    /// The aggregate's command type.
    type Command;
    /// The aggregate's event type.
    type Event;
    /// The aggregate's error type.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Returns the aggregate's identity.
    fn id(&self) -> &Self::Id;
    /// Returns the aggregate's current version.
    fn version(&self) -> ObjectVersion;
    /// Returns the aggregate's current lifecycle epoch.
    fn lifecycle_epoch(&self) -> LifecycleEpoch;
    /// Returns the aggregate's current authority epoch.
    fn authority_epoch(&self) -> AuthorityEpoch;

    /// Returns `Some(ConflictPendingMarker)` if this aggregate has an
    /// unresolved conflict. Otherwise returns `None`.
    ///
    /// Added per Architectural Ruling Q2 (Team 3 Initiation): a
    /// non-breaking additive default. Existing Increment 1 aggregates
    /// (`Mission`, `Task`) inherit this default unchanged and continue to
    /// compile without modification. Aggregates that support conflict
    /// resolution (wired up in Increment 3+) override it.
    fn conflict_pending(&self) -> Option<ConflictPendingMarker> {
        None
    }

    /// Pure decision function.
    ///
    /// Given a command and context, returns zero or more domain events, or
    /// an error. No I/O, no side effects. Must be deterministic. Called by
    /// the command handler before any persistence.
    ///
    /// # Increment 1 Ruling (Aggregate Construction, ruling C)
    /// `decide()` assumes an existing aggregate instance. Creation commands
    /// (e.g. `CreateMission`, `CreateTask`) are **not** routed through this
    /// method — they use a dedicated associated constructor function
    /// (e.g. `Mission::create(cmd, &ctx)`) instead, since there is no
    /// aggregate instance to call `decide()` on yet.
    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error>;

    /// Apply a domain event to mutate the aggregate state.
    ///
    /// Called during rehydration and after `decide()` to produce new state.
    /// Must be pure: no validation, no I/O.
    fn apply(&mut self, event: &Self::Event);
}
