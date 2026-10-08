//! Errors produced by the Conversation and Message aggregates.

use serde::{Deserialize, Serialize};

/// An error produced while deciding or constructing a Conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum ConversationError {
    /// The command is not valid from the conversation's current status.
    #[error("Invalid conversation status transition: {0}")]
    InvalidTransition(String),

    /// A required field was missing or empty.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The actor lacks the authority required for this command.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// The member being added is already a member.
    #[error("Already a member: {0:?}")]
    AlreadyMember(platform_kernel::UserId),

    /// `CreateConversation` was called with `SubTeam` but no
    /// `parent_supergroup`, or with any other `conversation_type` but a
    /// `parent_supergroup` present.
    #[error("Invalid parent supergroup reference: {0}")]
    InvalidParentSupergroup(String),

    /// `AddMember` was called on a `SubTeam` conversation for a user who
    /// is not yet a member of the referenced `parent_supergroup`.
    /// Enforces the explicit product decision "supergroup membership
    /// required first" (`PLAN_Desktop_Web_Completion.md` §7.1). This
    /// crate cannot check the parent's actual membership list itself —
    /// aggregates are independently loaded and this crate has no
    /// cross-aggregate query capability (§5.2 "strong consistency exists
    /// only within one aggregate") — so the caller (the composition
    /// root, which loads both aggregates) must supply the answer via
    /// `DecisionContext`-adjacent input; see `Conversation::decide`'s
    /// `AddMember` arm doc comment for exactly how.
    #[error("User is not a member of the parent supergroup")]
    NotSupergroupMember,
}

/// An error produced while deciding or constructing a Message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum MessageError {
    /// The command is not valid from the message's current status.
    #[error("Invalid message status transition: {0}")]
    InvalidTransition(String),

    /// The message body failed validation (§value::MessageBody::new).
    #[error("Invalid message body: {0}")]
    InvalidBody(String),

    /// The reaction code failed validation.
    #[error("Invalid reaction: {0}")]
    InvalidReaction(String),

    /// The actor lacks the authority required for this command — in
    /// particular, only the original author may edit or redact their own
    /// message (§4.7.4's "Message edits create revisions; audit lineage
    /// remains" implies authorship is checked, not merely membership).
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

/// An error produced while deciding or constructing a ConnectionRequest.
/// New in Phase 1 — see `value::ConnectionRequestId`'s doc comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum ConnectionRequestError {
    /// The command is not valid from the request's current status.
    #[error("Invalid connection request status transition: {0}")]
    InvalidTransition(String),

    /// A user attempted to send a connection request to themselves.
    #[error("Cannot send a connection request to yourself")]
    SelfRequest,

    /// Only the recipient may accept/decline; only the sender may
    /// revoke.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}
