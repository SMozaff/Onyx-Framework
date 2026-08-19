//! Domain events emitted by the Conversation, Message, and
//! ConnectionRequest aggregates.
//! Source: Part I §4.7.5, plus Phase 1 additions.

use crate::value::{
    ConnectionRequestId, ConversationId, ConversationType, MessageId, ReactionCode,
    RedactionReason,
};
use platform_kernel::{Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// Events emitted by the Conversation aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConversationEvent {
    /// A new conversation was created.
    ConversationCreated {
        /// The new conversation's identity.
        conversation_id: ConversationId,
        /// What kind of conversation this is.
        conversation_type: ConversationType,
        /// Which Supergroup this sub-team is nested under, if
        /// `conversation_type` is `SubTeam`. Always `None` otherwise.
        parent_supergroup: Option<ConversationId>,
        /// The creating actor, who becomes the first member.
        created_by: UserId,
        /// When the conversation was created.
        created_at: Timestamp,
    },
    /// A member was added to the conversation.
    ConversationMemberAdded {
        /// The member added.
        user_id: UserId,
        /// When they were added.
        added_at: Timestamp,
    },
    /// The conversation was archived.
    ConversationArchived {
        /// When it was archived.
        archived_at: Timestamp,
        /// Why, if given.
        reason: Option<String>,
    },
}

/// Events emitted by the Message aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MessageEvent {
    /// A message was posted.
    MessagePosted {
        /// The new message's identity.
        message_id: MessageId,
        /// Which conversation it belongs to.
        conversation_id: ConversationId,
        /// The posting actor.
        author_id: UserId,
        /// The message text.
        body: String,
        /// When it was posted.
        posted_at: Timestamp,
    },
    /// A message was edited. Per §4.7.4, this is an append-only revision,
    /// not an overwrite — both `MessagePosted` and every `MessageEdited`
    /// remain in the event stream, preserving the full audit lineage.
    MessageEdited {
        /// The new body text.
        new_body: String,
        /// When the edit was made.
        edited_at: Timestamp,
    },
    /// A message was redacted.
    MessageRedacted {
        /// Why it was redacted.
        reason: RedactionReason,
        /// When it was redacted.
        redacted_at: Timestamp,
    },
    /// A reaction was added.
    ReactionAdded {
        /// Who reacted.
        user_id: UserId,
        /// The reaction applied.
        emoji_code: ReactionCode,
        /// When.
        added_at: Timestamp,
    },
    /// A reaction was removed.
    ReactionRemoved {
        /// Who removed their reaction.
        user_id: UserId,
        /// The reaction removed.
        emoji_code: ReactionCode,
        /// When.
        removed_at: Timestamp,
    },
}

/// Events emitted by the ConnectionRequest aggregate. New in Phase 1 —
/// see `value::ConnectionRequestId`'s doc comment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConnectionRequestEvent {
    /// A connection request was sent.
    ConnectionRequestSent {
        /// The new request's identity.
        connection_request_id: ConnectionRequestId,
        /// Who sent it.
        sender_id: UserId,
        /// Who it's to.
        recipient_id: UserId,
        /// When.
        sent_at: Timestamp,
    },
    /// The recipient accepted.
    ConnectionRequestAccepted {
        /// When.
        accepted_at: Timestamp,
    },
    /// The recipient declined.
    ConnectionRequestDeclined {
        /// When.
        declined_at: Timestamp,
    },
    /// The sender revoked the request.
    ConnectionRequestRevoked {
        /// When.
        revoked_at: Timestamp,
    },
}
