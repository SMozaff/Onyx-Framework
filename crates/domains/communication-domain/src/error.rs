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
