//! Commands accepted by the Conversation and Message aggregates.
//! Source: Part I §4.7.5.

use crate::value::{ConversationId, ConversationType};
use platform_kernel::UserId;
use serde::{Deserialize, Serialize};

/// Commands accepted by the Conversation aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConversationCommand {
    /// Create a new conversation. Routed through `Conversation::create()`,
    /// not `decide()` — mirrors `Task::create()`/`Mission::create()`'s
    /// established pattern (Increment 1 ruling C), since `decide()`
    /// assumes an aggregate already exists.
    CreateConversation {
        /// What kind of conversation to create.
        conversation_type: ConversationType,
    },
    /// Add a member to the conversation.
    AddMember {
        /// The user to add.
        user_id: UserId,
    },
    /// Archive the conversation.
    ArchiveConversation {
        /// Optional reason.
        reason: Option<String>,
    },
}

/// Commands accepted by the Message aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MessageCommand {
    /// Post a new message. Routed through `Message::create()`, same
    /// rationale as `CreateConversation` above.
    PostMessage {
        /// Which conversation this message belongs to.
        conversation_id: ConversationId,
        /// The message text.
        body: String,
    },
    /// Edit an existing message's body.
    EditMessage {
        /// The replacement text.
        new_body: String,
    },
    /// Redact a message.
    RedactMessage {
        /// Why.
        reason: String,
    },
    /// Add a reaction.
    AddReaction {
        /// The reaction to add.
        emoji_code: String,
    },
    /// Remove a previously-added reaction.
    RemoveReaction {
        /// The reaction to remove.
        emoji_code: String,
    },
}
