//! # communication-domain
//!
//! ONYX Conversation/Message/ConnectionRequest aggregates: pure domain
//! logic for team messaging (Part I §4.7), plus Phase 1 additions —
//! Supergroup/SubTeam conversation structure and a User-to-User
//! ConnectionRequest aggregate (see `PLAN_Desktop_Web_Completion.md`
//! §7.1 for the product decisions behind both).
//!
//! # Status
//! `Conversation`, `Message`, and `ConnectionRequest` are complete
//! `AggregateRoot` implementations with full state-transition test
//! coverage (43 tests — every valid transition, every rejected one, both
//! terminal states, idempotent reaction add/remove, the
//! supergroup-membership-required-first invariant, and the
//! sender/recipient authorization split on `ConnectionRequest`).
//!
//! `Conversation`/`Message` are already wired into
//! `client-composition`'s command/query registries and `AppState`
//! (desktop). The Phase 1 additions in this file (`Supergroup`,
//! `SubTeam`, `ConnectionRequest`) are **not yet** wired there or into
//! any persistence adapter — that integration work is tracked separately
//! (see `PLAN_Desktop_Web_Completion.md`), and existing `Conversation`/
//! `Message` command dispatch is unaffected: `CreateConversation` and
//! `AddMember` gained new fields (`parent_supergroup`,
//! `is_parent_supergroup_member`), but every existing call site
//! (`Direct`/`Channel` conversations) supplies `None`/`false` for them,
//! which is a no-op for every invariant this change added.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod aggregate;
pub mod command;
pub mod error;
pub mod event;
pub mod state_machine;
pub mod value;

/// Deterministic test fixtures, exposed unconditionally for the same
/// reason as `mission_domain::test_support` (see its docs) — other
/// crates' integration tests need to construct valid `Conversation`/
/// `Message`/`ConnectionRequest` fixtures without duplicating this
/// crate's construction logic.
pub mod test_support;

pub use aggregate::{ConnectionRequest, Conversation, Message};
pub use command::{ConnectionRequestCommand, ConversationCommand, MessageCommand};
pub use error::{ConnectionRequestError, ConversationError, MessageError};
pub use event::{ConnectionRequestEvent, ConversationEvent, MessageEvent};
pub use state_machine::{ConnectionRequestStatus, ConversationStatus, MessageStatus};
pub use value::{
    ConnectionRequestId, ConversationId, ConversationType, MessageBody, MessageId, Reaction,
    ReactionCode, RedactionReason,
};
