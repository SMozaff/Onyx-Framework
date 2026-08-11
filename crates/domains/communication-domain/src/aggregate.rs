//! The Conversation and Message aggregate roots.
//!
//! Source: Part I §4.7. Message is an independent aggregate root, not an
//! entity nested inside Conversation (§4.7.1) — its own optimistic-
//! concurrency version, so a burst of message posts never contends with
//! Conversation-level operations (membership changes, archival), and
//! Conversation lifecycle transitions never advance a Message's version
//! or vice versa.
//!
//! # Authority (Increment 1 parity)
//! Both aggregates follow `mission_domain::Mission` / `work_domain::Task`'s
//! established precedent exactly: `decide()` checks only the generic
//! `context.authority.is_authorized(...)` stub. Real per-object checks —
//! "only the author may edit their own message," membership-gated
//! `AddMember` — are ABAC/policy concerns Increment 7 owns (Part I §8),
//! not something this crate invents independently. Diverging from that
//! precedent here would make Communication stricter than Mission/Task for
//! no documented reason, and inconsistently enforced authority is worse
//! than consistently deferred authority.

use crate::command::{ConversationCommand, MessageCommand};
use crate::error::{ConversationError, MessageError};
use crate::event::{ConversationEvent, MessageEvent};
use crate::state_machine::{ConversationStatus, MessageStatus};
use crate::value::{
    ConversationId, ConversationType, MessageBody, MessageId, ReactionCode, RedactionReason,
};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectVersion, UserId};
use serde::{Deserialize, Serialize};

/// The Conversation aggregate: membership, access scope, and lifecycle
/// (§4.7.1). Owns *who can participate*, not message content.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conversation {
    id: ConversationId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: ConversationStatus,
    conversation_type: ConversationType,
    members: Vec<UserId>,
}

impl Conversation {
    /// Constructs a new Conversation from a `CreateConversation` command.
    /// The creating actor becomes the first member.
    ///
    /// # Errors
    /// Returns [`ConversationError::InvalidTransition`] if called with any
    /// command other than `CreateConversation`.
    pub fn create(
        command: ConversationCommand,
        context: &DecisionContext,
    ) -> Result<Vec<ConversationEvent>, ConversationError> {
        let ConversationCommand::CreateConversation { conversation_type } = command else {
            return Err(ConversationError::InvalidTransition(
                "Conversation::create() called with a non-CreateConversation command".to_string(),
            ));
        };

        Ok(vec![ConversationEvent::ConversationCreated {
            conversation_id: ConversationId(context.generated_id_generator.generate_object_id()),
            conversation_type,
            created_by: context.actor.user_id,
            created_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `Conversation` by folding its first event
    /// (`ConversationCreated`).
    ///
    /// # Panics
    /// Panics if `event` is not a `ConversationCreated` event.
    pub fn from_created_event(event: &ConversationEvent) -> Self {
        let ConversationEvent::ConversationCreated {
            conversation_id,
            conversation_type,
            created_by,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-ConversationCreated event");
        };

        Self {
            id: *conversation_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: ConversationStatus::Active,
            conversation_type: *conversation_type,
            members: vec![*created_by],
        }
    }

    /// The conversation's current status.
    pub fn status(&self) -> ConversationStatus {
        self.status
    }

    /// What kind of conversation this is.
    pub fn conversation_type(&self) -> ConversationType {
        self.conversation_type
    }

    /// The conversation's current members.
    pub fn members(&self) -> &[UserId] {
        &self.members
    }

    fn invalid(&self, command: &str) -> ConversationError {
        ConversationError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for Conversation {
    type Id = ConversationId;
    type Command = ConversationCommand;
    type Event = ConversationEvent;
    type Error = ConversationError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if !context.authority.is_authorized("communication.command") {
            return Err(ConversationError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            ConversationCommand::CreateConversation { .. } => {
                Err(ConversationError::InvalidTransition(
                    "CreateConversation must be dispatched via Conversation::create(), not decide()"
                        .to_string(),
                ))
            }

            ConversationCommand::AddMember { user_id } => match self.status {
                ConversationStatus::Active => {
                    if self.members.contains(&user_id) {
                        return Err(ConversationError::AlreadyMember(user_id));
                    }
                    Ok(vec![ConversationEvent::ConversationMemberAdded {
                        user_id,
                        added_at: context.trusted_now,
                    }])
                }
                ConversationStatus::Archived => Err(self.invalid("AddMember")),
            },

            ConversationCommand::ArchiveConversation { reason } => match self.status {
                ConversationStatus::Active => Ok(vec![ConversationEvent::ConversationArchived {
                    archived_at: context.trusted_now,
                    reason,
                }]),
                ConversationStatus::Archived => Err(self.invalid("ArchiveConversation")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            ConversationEvent::ConversationCreated { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            ConversationEvent::ConversationMemberAdded { user_id, .. } => {
                self.members.push(*user_id);
                // §4.7.7's synchronization contract marks "Membership" as
                // authority-controlled — mirrors Task::TaskOwnerAssigned
                // advancing authority_epoch for the same reason: who is
                // authorized to act (post, be addressed) within this
                // aggregate just changed.
                self.authority_epoch = self.authority_epoch.advance();
            }
            ConversationEvent::ConversationArchived { .. } => {
                self.status = ConversationStatus::Archived;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

/// The Message aggregate: content, revision lineage, and delivery state
/// (§4.7.1) — an independent aggregate root, not an entity of
/// `Conversation`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    id: MessageId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: MessageStatus,
    conversation_id: ConversationId,
    author_id: UserId,
    body: String,
    reactions: Vec<(UserId, ReactionCode)>,
}

impl Message {
    /// Constructs a new Message from a `PostMessage` command.
    ///
    /// # Errors
    /// Returns [`MessageError::InvalidBody`] if the body fails
    /// [`MessageBody::new`]'s validation, or
    /// [`MessageError::InvalidTransition`] if called with any command
    /// other than `PostMessage`.
    pub fn create(
        command: MessageCommand,
        context: &DecisionContext,
    ) -> Result<Vec<MessageEvent>, MessageError> {
        let MessageCommand::PostMessage {
            conversation_id,
            body,
        } = command
        else {
            return Err(MessageError::InvalidTransition(
                "Message::create() called with a non-PostMessage command".to_string(),
            ));
        };

        let body = MessageBody::new(body).map_err(MessageError::InvalidBody)?;

        Ok(vec![MessageEvent::MessagePosted {
            message_id: MessageId(context.generated_id_generator.generate_object_id()),
            conversation_id,
            author_id: context.actor.user_id,
            body: body.as_str().to_string(),
            posted_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `Message` by folding its first event (`MessagePosted`).
    ///
    /// # Panics
    /// Panics if `event` is not a `MessagePosted` event.
    pub fn from_created_event(event: &MessageEvent) -> Self {
        let MessageEvent::MessagePosted {
            message_id,
            conversation_id,
            author_id,
            body,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-MessagePosted event");
        };

        Self {
            id: *message_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: MessageStatus::Posted,
            conversation_id: *conversation_id,
            author_id: *author_id,
            body: body.clone(),
            reactions: Vec::new(),
        }
    }

    /// The message's current status.
    pub fn status(&self) -> MessageStatus {
        self.status
    }

    /// The message's current body text. `None` once redacted — §4.7.4's
    /// "Deletion is policy-governed redaction ... not silent erasure"
    /// means the fact of redaction is preserved, but the content itself
    /// is not retrievable through this accessor once gone.
    pub fn body(&self) -> Option<&str> {
        match self.status {
            MessageStatus::Redacted => None,
            _ => Some(&self.body),
        }
    }

    /// Which conversation this message belongs to.
    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    /// Who posted this message.
    pub fn author_id(&self) -> UserId {
        self.author_id
    }

    /// Current reactions as `(user, code)` pairs.
    pub fn reactions(&self) -> &[(UserId, ReactionCode)] {
        &self.reactions
    }

    fn invalid(&self, command: &str) -> MessageError {
        MessageError::InvalidTransition(format!("{command} from {:?}", self.status))
    }

    /// Whether the message can still be edited or reacted to.
    fn is_mutable(&self) -> bool {
        matches!(self.status, MessageStatus::Posted | MessageStatus::Edited)
    }
}

impl AggregateRoot for Message {
    type Id = MessageId;
    type Command = MessageCommand;
    type Event = MessageEvent;
    type Error = MessageError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if !context.authority.is_authorized("communication.command") {
            return Err(MessageError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            MessageCommand::PostMessage { .. } => Err(MessageError::InvalidTransition(
                "PostMessage must be dispatched via Message::create(), not decide()".to_string(),
            )),

            MessageCommand::EditMessage { new_body } => {
                if !self.is_mutable() {
                    return Err(self.invalid("EditMessage"));
                }
                let body = MessageBody::new(new_body).map_err(MessageError::InvalidBody)?;
                Ok(vec![MessageEvent::MessageEdited {
                    new_body: body.as_str().to_string(),
                    edited_at: context.trusted_now,
                }])
            }

            MessageCommand::RedactMessage { reason } => {
                if !self.is_mutable() {
                    return Err(self.invalid("RedactMessage"));
                }
                Ok(vec![MessageEvent::MessageRedacted {
                    reason: RedactionReason(reason),
                    redacted_at: context.trusted_now,
                }])
            }

            MessageCommand::AddReaction { emoji_code } => {
                if !self.is_mutable() {
                    return Err(self.invalid("AddReaction"));
                }
                let code = ReactionCode::new(&emoji_code).map_err(MessageError::InvalidReaction)?;
                if self.reactions.contains(&(context.actor.user_id, code)) {
                    // Idempotent no-op rather than an error: re-tapping an
                    // already-applied reaction is a normal UI action, not
                    // a client bug worth surfacing as a domain error.
                    return Ok(vec![]);
                }
                Ok(vec![MessageEvent::ReactionAdded {
                    user_id: context.actor.user_id,
                    emoji_code: code,
                    added_at: context.trusted_now,
                }])
            }

            MessageCommand::RemoveReaction { emoji_code } => {
                if !self.is_mutable() {
                    return Err(self.invalid("RemoveReaction"));
                }
                let code = ReactionCode::new(&emoji_code).map_err(MessageError::InvalidReaction)?;
                if !self.reactions.contains(&(context.actor.user_id, code)) {
                    return Ok(vec![]);
                }
                Ok(vec![MessageEvent::ReactionRemoved {
                    user_id: context.actor.user_id,
                    emoji_code: code,
                    removed_at: context.trusted_now,
                }])
            }
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            MessageEvent::MessagePosted { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            MessageEvent::MessageEdited { new_body, .. } => {
                self.body = new_body.clone();
                self.status = MessageStatus::Edited;
            }
            MessageEvent::MessageRedacted { .. } => {
                self.body.clear();
                self.status = MessageStatus::Redacted;
            }
            MessageEvent::ReactionAdded {
                user_id,
                emoji_code,
                ..
            } => {
                self.reactions.push((*user_id, *emoji_code));
            }
            MessageEvent::ReactionRemoved {
                user_id,
                emoji_code,
                ..
            } => {
                self.reactions.retain(|r| r != &(*user_id, *emoji_code));
            }
        }
        self.version = self.version.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{active_conversation, posted_message, test_context, test_user_id};

    // ---- Conversation --------------------------------------------------

    #[test]
    fn create_conversation_makes_creator_the_first_member() {
        let ctx = test_context();
        let events = Conversation::create(
            ConversationCommand::CreateConversation {
                conversation_type: ConversationType::Direct,
            },
            &ctx,
        )
        .expect("create must succeed");
        let conversation = Conversation::from_created_event(&events[0]);

        assert_eq!(conversation.status(), ConversationStatus::Active);
        assert_eq!(conversation.members(), &[ctx.actor.user_id]);
        assert_eq!(conversation.version(), ObjectVersion::INITIAL);
    }

    #[test]
    fn create_called_with_wrong_command_errors() {
        let ctx = test_context();
        let result = Conversation::create(
            ConversationCommand::ArchiveConversation { reason: None },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(ConversationError::InvalidTransition(_))
        ));
    }

    #[test]
    fn add_member_from_active_succeeds_and_advances_authority_epoch() {
        let conversation = active_conversation();
        let ctx = test_context();
        let new_member = test_user_id();

        let events = conversation
            .decide(
                ConversationCommand::AddMember {
                    user_id: new_member,
                },
                &ctx,
            )
            .expect("AddMember must succeed from Active");

        let mut updated = conversation.clone();
        for event in &events {
            updated.apply(event);
        }

        assert!(updated.members().contains(&new_member));
        assert_eq!(updated.authority_epoch(), AuthorityEpoch::INITIAL.advance());
        assert_eq!(updated.version(), conversation.version().next());
    }

    #[test]
    fn add_member_duplicate_is_rejected() {
        let conversation = active_conversation();
        let ctx = test_context();
        let existing_member = conversation.members()[0];

        let result = conversation.decide(
            ConversationCommand::AddMember {
                user_id: existing_member,
            },
            &ctx,
        );
        assert!(matches!(result, Err(ConversationError::AlreadyMember(_))));
    }

    #[test]
    fn add_member_from_archived_is_rejected() {
        let conversation = active_conversation();
        let ctx = test_context();
        let archived_events = conversation
            .decide(
                ConversationCommand::ArchiveConversation { reason: None },
                &ctx,
            )
            .unwrap();
        let mut archived = conversation.clone();
        for e in &archived_events {
            archived.apply(e);
        }
        assert_eq!(archived.status(), ConversationStatus::Archived);

        let result = archived.decide(
            ConversationCommand::AddMember {
                user_id: test_user_id(),
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(ConversationError::InvalidTransition(_))
        ));
    }

    #[test]
    fn archive_from_active_succeeds() {
        let conversation = active_conversation();
        let ctx = test_context();
        let before_epoch = conversation.lifecycle_epoch();

        let events = conversation
            .decide(
                ConversationCommand::ArchiveConversation {
                    reason: Some("project wrapped up".to_string()),
                },
                &ctx,
            )
            .expect("ArchiveConversation must succeed from Active");
        let mut archived = conversation.clone();
        for e in &events {
            archived.apply(e);
        }

        assert_eq!(archived.status(), ConversationStatus::Archived);
        assert_eq!(archived.lifecycle_epoch(), before_epoch.advance());
    }

    #[test]
    fn archive_from_archived_is_rejected() {
        let conversation = active_conversation();
        let ctx = test_context();
        let events = conversation
            .decide(
                ConversationCommand::ArchiveConversation { reason: None },
                &ctx,
            )
            .unwrap();
        let mut archived = conversation.clone();
        for e in &events {
            archived.apply(e);
        }

        let result = archived.decide(
            ConversationCommand::ArchiveConversation { reason: None },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(ConversationError::InvalidTransition(_))
        ));
    }

    // ---- Message ---------------------------------------------------------

    #[test]
    fn post_message_succeeds_with_author_and_body() {
        let ctx = test_context();
        let conversation = active_conversation();
        let events = Message::create(
            MessageCommand::PostMessage {
                conversation_id: *conversation.id(),
                body: "standup notes".to_string(),
            },
            &ctx,
        )
        .expect("create must succeed");
        let message = Message::from_created_event(&events[0]);

        assert_eq!(message.status(), MessageStatus::Posted);
        assert_eq!(message.author_id(), ctx.actor.user_id);
        assert_eq!(message.body(), Some("standup notes"));
        assert_eq!(message.conversation_id(), *conversation.id());
    }

    #[test]
    fn post_message_rejects_empty_body() {
        let ctx = test_context();
        let result = Message::create(
            MessageCommand::PostMessage {
                conversation_id: ConversationId::new_random(),
                body: "   ".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(MessageError::InvalidBody(_))));
    }

    #[test]
    fn post_message_rejects_oversized_body() {
        let ctx = test_context();
        let result = Message::create(
            MessageCommand::PostMessage {
                conversation_id: ConversationId::new_random(),
                body: "a".repeat(MessageBody::MAX_LEN + 1),
            },
            &ctx,
        );
        assert!(matches!(result, Err(MessageError::InvalidBody(_))));
    }

    #[test]
    fn edit_message_from_posted_succeeds_and_creates_a_revision() {
        let message = posted_message();
        let ctx = test_context();

        let events = message
            .decide(
                MessageCommand::EditMessage {
                    new_body: "corrected notes".to_string(),
                },
                &ctx,
            )
            .expect("EditMessage must succeed from Posted");
        let mut edited = message.clone();
        for e in &events {
            edited.apply(e);
        }

        assert_eq!(edited.status(), MessageStatus::Edited);
        assert_eq!(edited.body(), Some("corrected notes"));
        assert!(matches!(events[0], MessageEvent::MessageEdited { .. }));
    }

    #[test]
    fn edit_message_from_edited_succeeds_again() {
        let message = posted_message();
        let ctx = test_context();
        let mut edited = message.clone();
        for e in message
            .decide(
                MessageCommand::EditMessage {
                    new_body: "v2".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            edited.apply(&e);
        }
        assert_eq!(edited.status(), MessageStatus::Edited);

        let result = edited.decide(
            MessageCommand::EditMessage {
                new_body: "v3".to_string(),
            },
            &ctx,
        );
        assert!(result.is_ok(), "a message may be edited more than once");
    }

    #[test]
    fn edit_message_from_redacted_is_rejected() {
        let message = posted_message();
        let ctx = test_context();
        let mut redacted = message.clone();
        for e in message
            .decide(
                MessageCommand::RedactMessage {
                    reason: "policy".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            redacted.apply(&e);
        }

        let result = redacted.decide(
            MessageCommand::EditMessage {
                new_body: "too late".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(MessageError::InvalidTransition(_))));
    }

    #[test]
    fn redact_message_clears_body_and_is_terminal() {
        let message = posted_message();
        let ctx = test_context();
        let mut redacted = message.clone();
        for e in message
            .decide(
                MessageCommand::RedactMessage {
                    reason: "sender requested deletion".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            redacted.apply(&e);
        }

        assert_eq!(redacted.status(), MessageStatus::Redacted);
        assert_eq!(
            redacted.body(),
            None,
            "redacted body must not be retrievable"
        );

        let result = redacted.decide(
            MessageCommand::RedactMessage {
                reason: "again".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(MessageError::InvalidTransition(_))));
    }

    #[test]
    fn add_reaction_succeeds() {
        let message = posted_message();
        let ctx = test_context();

        let events = message
            .decide(
                MessageCommand::AddReaction {
                    emoji_code: "thumbsup".to_string(),
                },
                &ctx,
            )
            .expect("AddReaction must succeed on a Posted message");
        let mut reacted = message.clone();
        for e in &events {
            reacted.apply(e);
        }

        assert_eq!(reacted.reactions().len(), 1);
        assert_eq!(reacted.reactions()[0].0, ctx.actor.user_id);
    }

    #[test]
    fn add_reaction_duplicate_is_an_idempotent_noop() {
        let message = posted_message();
        let ctx = test_context();
        let mut reacted = message.clone();
        for e in message
            .decide(
                MessageCommand::AddReaction {
                    emoji_code: "thumbsup".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            reacted.apply(&e);
        }
        assert_eq!(reacted.reactions().len(), 1);

        let events = reacted
            .decide(
                MessageCommand::AddReaction {
                    emoji_code: "thumbsup".to_string(),
                },
                &ctx,
            )
            .expect("duplicate reaction must not error");
        assert!(
            events.is_empty(),
            "duplicate reaction must be a no-op, not a new event"
        );
    }

    #[test]
    fn add_reaction_on_redacted_message_is_rejected() {
        let message = posted_message();
        let ctx = test_context();
        let mut redacted = message.clone();
        for e in message
            .decide(
                MessageCommand::RedactMessage {
                    reason: "policy".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            redacted.apply(&e);
        }

        let result = redacted.decide(
            MessageCommand::AddReaction {
                emoji_code: "thumbsup".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(MessageError::InvalidTransition(_))));
    }

    #[test]
    fn remove_reaction_succeeds() {
        let message = posted_message();
        let ctx = test_context();
        let mut reacted = message.clone();
        for e in message
            .decide(
                MessageCommand::AddReaction {
                    emoji_code: "thumbsup".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            reacted.apply(&e);
        }
        assert_eq!(reacted.reactions().len(), 1);

        let events = reacted
            .decide(
                MessageCommand::RemoveReaction {
                    emoji_code: "thumbsup".to_string(),
                },
                &ctx,
            )
            .expect("RemoveReaction must succeed");
        let mut unreacted = reacted.clone();
        for e in &events {
            unreacted.apply(e);
        }
        assert!(unreacted.reactions().is_empty());
    }

    #[test]
    fn remove_reaction_that_was_never_added_is_an_idempotent_noop() {
        let message = posted_message();
        let ctx = test_context();

        let events = message
            .decide(
                MessageCommand::RemoveReaction {
                    emoji_code: "thumbsup".to_string(),
                },
                &ctx,
            )
            .expect("removing a reaction that was never added must not error");
        assert!(events.is_empty());
    }

    #[test]
    fn post_message_command_dispatched_via_decide_is_rejected() {
        let message = posted_message();
        let ctx = test_context();
        let result = message.decide(
            MessageCommand::PostMessage {
                conversation_id: ConversationId::new_random(),
                body: "should not work".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(MessageError::InvalidTransition(_))));
    }

    #[test]
    fn version_advances_by_exactly_one_per_applied_event() {
        let message = posted_message();
        let ctx = test_context();
        let start = message.version();

        let mut m = message.clone();
        for e in message
            .decide(
                MessageCommand::AddReaction {
                    emoji_code: "thumbsup".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            m.apply(&e);
        }
        assert_eq!(m.version(), start.next());
    }
}
