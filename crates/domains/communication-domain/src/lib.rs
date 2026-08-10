//! # communication-domain
//!
//! ONYX Conversation/Message aggregates: pure domain logic for team
//! messaging (Part I §4.7). No I/O, no persistence, no async.
//!
//! # Status
//! Value types, errors, state machines, commands, and events are complete.
//! `Conversation`/`Message` aggregate implementations (`decide`/`apply`,
//! per `platform_contracts::AggregateRoot`) are not yet written — this
//! crate does not yet export a working aggregate. Tracked as in-progress,
//! not silently assumed done: nothing in the workspace depends on this
//! crate yet, so its incompleteness is contained.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod command;
pub mod error;
pub mod event;
pub mod state_machine;
pub mod value;

pub use command::{ConversationCommand, MessageCommand};
pub use error::{ConversationError, MessageError};
pub use event::{ConversationEvent, MessageEvent};
pub use state_machine::{ConversationStatus, MessageStatus};
pub use value::{
    ConversationId, ConversationType, MessageBody, MessageId, Reaction, ReactionCode,
    RedactionReason,
};
