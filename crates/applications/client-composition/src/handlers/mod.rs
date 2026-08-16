//! Concrete [`CreationHandler`](crate::CreationHandler)/
//! [`DecisionHandler`](crate::DecisionHandler) implementations for the
//! two real aggregate types Increment 1 delivered: `Mission`
//! (`mission-domain`) and `Task` (`work-domain`).
//!
//! # Provenance
//! Not specified by any Team Prompt — these are the glue between the
//! generic `CommandRegistry` (`command_registry.rs`) and the real,
//! concrete `AggregateRoot` implementations, grounded in Team 1's actual
//! delivered `Mission::create`/`Task::create` and Team 2's actual
//! `api_server::handle_command` signature (see R3/S1 in `DECISIONS.md`).
//!
//! One `{Mission,Task}CreationHandler`/`{Mission,Task}DecisionHandler`
//! pair per aggregate type, each owning its own aggregate-type-scoped
//! `Repository`/`UnitOfWorkFactory` (see the repository-scoping
//! correction documented on `CreationHandler`'s doc comment) plus a
//! shared `IdempotencyStore` (cross-aggregate by design, per
//! `worker-application`'s own `IdempotencyStore` doc comment: keyed only
//! by `OperationId`).
//!
//! Split into `creation_handler.rs`/`decision_handler.rs` (this module
//! holding only what both files share) — a structural refactor of what
//! was originally one flat `handlers.rs`; no behavior changed, verified
//! by an unmodified full test suite pass after the split (see
//! `DECISIONS.md`).

mod creation_handler;
mod decision_handler;

pub use creation_handler::{
    ConnectionRequestCreationHandler, ConversationCreationHandler, FileAssetCreationHandler,
    LegalHoldCreationHandler, MessageCreationHandler, MissionCreationHandler,
    PolicyCreationHandler, TaskCreationHandler, UploadSessionCreationHandler,
};
pub use decision_handler::{
    ConnectionRequestDecisionHandler, ConversationDecisionHandler, FileAssetDecisionHandler,
    LegalHoldDecisionHandler, MessageDecisionHandler, MissionDecisionHandler,
    PolicyDecisionHandler, TaskDecisionHandler, UploadSessionDecisionHandler,
};

use platform_contracts::{DecisionContext, IdGenerator};
use platform_kernel::{
    ActorContext, EventId, ObjectId, OperationId, PolicyDecisionSet, Timestamp, VerifiedAuthority,
};

/// Generates real random identifiers. Mirrors `api_server::command_handler`'s
/// private `DefaultIdGenerator` (not exported from that crate — see
/// `DECISIONS.md` S1's doc comment on why `command_handler`/`query_handler`
/// were the only modules re-exported), so this is a deliberate, minimal
/// duplication of an 8-line struct rather than requesting a further
/// `api-server` visibility change for something this small.
struct RandomIdGenerator;

impl IdGenerator for RandomIdGenerator {
    fn generate_object_id(&self) -> ObjectId {
        ObjectId::new_random()
    }
    fn generate_operation_id(&self) -> OperationId {
        OperationId::new_random()
    }
    fn generate_event_id(&self) -> EventId {
        EventId::new_random()
    }
}

/// Builds a `DecisionContext` for a creation call, mirroring the one
/// `api_server::handle_command`'s Step 7 constructs (see that file) —
/// `authority`/`policy_outcomes` are the same Increment-1-stubbed values
/// (`VerifiedAuthority`, `PolicyDecisionSet`, both unconditionally
/// permissive per Team 1's ruling, real policy evaluation deferred to
/// Increment 7). Shared by both `creation_handler.rs` implementations,
/// which is why it lives here rather than in either of them.
pub(crate) fn creation_decision_context(actor: ActorContext) -> DecisionContext {
    DecisionContext {
        actor,
        authority: VerifiedAuthority,
        trusted_now: Timestamp::now(),
        policy_outcomes: PolicyDecisionSet,
        generated_id_generator: Box::new(RandomIdGenerator),
    }
}
