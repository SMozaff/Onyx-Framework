//! # communication-domain
//!
//! ONYX Conversation/Message aggregates: pure domain logic for team
//! messaging (Part I §4.7). No I/O, no persistence, no async.
//!
//! # Status
//! `Conversation` and `Message` are complete `AggregateRoot`
//! implementations with 100% state-transition test coverage (28 tests —
//! every valid transition, every rejected one, both terminal states,
//! idempotent reaction add/remove). Not yet wired into
//! `client-composition`'s command/query registries or any persistence
//! adapter — that integration work has not started.

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
/// `Message` fixtures without duplicating this crate's construction
/// logic.
pub mod test_support;

pub use aggregate::{Conversation, Message};
pub use command::{ConversationCommand, MessageCommand};
pub use error::{ConversationError, MessageError};
pub use event::{ConversationEvent, MessageEvent};
pub use state_machine::{ConversationStatus, MessageStatus};
pub use value::{
    ConversationId, ConversationType, MessageBody, MessageId, Reaction, ReactionCode,
    RedactionReason,
};
