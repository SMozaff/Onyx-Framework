//! Domain events emitted by the Conversation and Message aggregates.
//! Source: Part I §4.7.5.

use crate::value::{ConversationId, ConversationType, MessageId, ReactionCode, RedactionReason};
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
