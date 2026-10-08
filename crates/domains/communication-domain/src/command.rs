//! Commands accepted by the Conversation, Message, and ConnectionRequest
//! aggregates.
//! Source: Part I §4.7.5, plus Phase 1 additions (supergroup/sub-team
//! structure and ConnectionRequest — see
//! `PLAN_Desktop_Web_Completion.md` §7.1).

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
        /// Required if and only if `conversation_type` is `SubTeam` —
        /// which Supergroup this sub-team is nested under. Rejected if
        /// present for any other `conversation_type`, and rejected if
        /// absent for `SubTeam` (`Conversation::create`'s own
        /// validation, not left to the caller to get right).
        parent_supergroup: Option<ConversationId>,
    },
    /// Add a member to the conversation. For a `SubTeam` conversation,
    /// the target user must already be a member of the referenced
    /// `parent_supergroup` — enforced invariant, per the explicit
    /// product decision "supergroup membership required first" (§7.1).
    AddMember {
        /// The user to add.
        user_id: UserId,
        /// Whether `user_id` is currently a member of this
        /// conversation's `parent_supergroup`. Ignored for every
        /// `conversation_type` other than `SubTeam`. Supplied by the
        /// caller (composition root) rather than looked up by this
        /// crate — see `Conversation::decide`'s `AddMember` arm doc
        /// comment for why: aggregates are independently loaded and
        /// this crate has no cross-aggregate query capability (§5.2).
        ///
        /// `#[serde(default)]`: defaults to `false` (fail-closed) if a
        /// caller's JSON payload omits it — verified this is required,
        /// not optional-by-default, for a non-`Option` field (a plain
        /// `bool` errors on a missing key rather than silently
        /// defaulting), so every existing/future `Channel`/`Direct`
        /// caller that predates this field is unaffected rather than
        /// broken by a newly-required key.
        #[serde(default)]
        is_parent_supergroup_member: bool,
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

/// Commands accepted by the ConnectionRequest aggregate. New in Phase 1
/// — see `value::ConnectionRequestId`'s doc comment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConnectionRequestCommand {
    /// Send a connection request to another user. Routed through
    /// `ConnectionRequest::create()`, same rationale as
    /// `CreateConversation` above. The sender is the requesting actor
    /// (`context.actor.user_id`); `recipient_id` must already be
    /// resolved to a concrete `UserId` by the caller (via username, ID,
    /// or email lookup — that resolution is an application/composition-
    /// root concern, not this pure-domain crate's, matching how every
    /// other aggregate in this workspace receives already-resolved
    /// kernel ids rather than doing its own identity lookups).
    SendConnectionRequest {
        /// Who the request is to.
        recipient_id: UserId,
    },
    /// The recipient accepts.
    AcceptConnectionRequest,
    /// The recipient declines.
    DeclineConnectionRequest,
    /// The sender withdraws the request before it's decided.
    RevokeConnectionRequest,
}
