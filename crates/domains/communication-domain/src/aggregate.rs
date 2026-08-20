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

use crate::command::{ConnectionRequestCommand, ConversationCommand, MessageCommand};
use crate::error::{ConnectionRequestError, ConversationError, MessageError};
use crate::event::{ConnectionRequestEvent, ConversationEvent, MessageEvent};
use crate::state_machine::{ConnectionRequestStatus, ConversationStatus, MessageStatus};
use crate::value::{
    ConnectionRequestId, ConversationId, ConversationType, MessageBody, MessageId, ReactionCode,
    RedactionReason,
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
    /// `Some` if and only if `conversation_type` is `SubTeam` — see
    /// `ConversationType::SubTeam`'s doc comment.
    parent_supergroup: Option<ConversationId>,
    members: Vec<UserId>,
}

impl Conversation {
    /// Constructs a new Conversation from a `CreateConversation` command.
    /// The creating actor becomes the first member.
    ///
    /// # Errors
    /// Returns [`ConversationError::InvalidTransition`] if called with any
    /// command other than `CreateConversation`, or
    /// [`ConversationError::InvalidParentSupergroup`] if
    /// `parent_supergroup` is present but `conversation_type` isn't
    /// `SubTeam`, or absent while it is.
    pub fn create(
        command: ConversationCommand,
        context: &DecisionContext,
    ) -> Result<Vec<ConversationEvent>, ConversationError> {
        let ConversationCommand::CreateConversation {
            conversation_type,
            parent_supergroup,
        } = command
        else {
            return Err(ConversationError::InvalidTransition(
                "Conversation::create() called with a non-CreateConversation command".to_string(),
            ));
        };

        match (conversation_type, parent_supergroup) {
            (ConversationType::SubTeam, None) => {
                return Err(ConversationError::InvalidParentSupergroup(
                    "SubTeam requires a parent_supergroup".to_string(),
                ));
            }
            (other, Some(_)) if other != ConversationType::SubTeam => {
                return Err(ConversationError::InvalidParentSupergroup(format!(
                    "parent_supergroup is only valid for SubTeam, not {other:?}"
                )));
            }
            _ => {}
        }

        Ok(vec![ConversationEvent::ConversationCreated {
            conversation_id: ConversationId(context.generated_id_generator.generate_object_id()),
            conversation_type,
            parent_supergroup,
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
            parent_supergroup,
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
            parent_supergroup: *parent_supergroup,
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

    /// Which Supergroup this sub-team is nested under, if any.
    pub fn parent_supergroup(&self) -> Option<ConversationId> {
        self.parent_supergroup
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

            ConversationCommand::AddMember {
                user_id,
                is_parent_supergroup_member,
            } => match self.status {
                ConversationStatus::Active => {
                    if self.members.contains(&user_id) {
                        return Err(ConversationError::AlreadyMember(user_id));
                    }
                    // "Supergroup membership required first" — enforced
                    // here, not merely documented. This aggregate cannot
                    // load the parent Supergroup itself (§5.2, no
                    // cross-aggregate reads from within a pure decide()),
                    // so the composition root is trusted to have loaded
                    // it and supplied the answer via
                    // `is_parent_supergroup_member`. This mirrors how
                    // `context.authority` itself is a pre-verified input
                    // rather than something this crate re-derives.
                    if self.conversation_type == ConversationType::SubTeam
                        && !is_parent_supergroup_member
                    {
                        return Err(ConversationError::NotSupergroupMember);
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

/// The ConnectionRequest aggregate: the lifecycle of one User-to-User
/// contact request (see `value::ConnectionRequestId`'s doc comment).
/// Independent of `Conversation` — a Direct conversation may be started
/// once accepted, but this aggregate itself owns none of that; it only
/// tracks the request's own send/accept/decline/revoke lifecycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionRequest {
    id: ConnectionRequestId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: ConnectionRequestStatus,
    sender_id: UserId,
    recipient_id: UserId,
}

impl ConnectionRequest {
    /// Constructs a new `ConnectionRequest` from a
    /// `SendConnectionRequest` command. The sending actor is
    /// `context.actor.user_id`.
    ///
    /// # Errors
    /// Returns [`ConnectionRequestError::InvalidTransition`] if called
    /// with any command other than `SendConnectionRequest`, or
    /// [`ConnectionRequestError::SelfRequest`] if `recipient_id` is the
    /// sending actor.
    pub fn create(
        command: ConnectionRequestCommand,
        context: &DecisionContext,
    ) -> Result<Vec<ConnectionRequestEvent>, ConnectionRequestError> {
        let ConnectionRequestCommand::SendConnectionRequest { recipient_id } = command else {
            return Err(ConnectionRequestError::InvalidTransition(
                "ConnectionRequest::create() called with a non-SendConnectionRequest command"
                    .to_string(),
            ));
        };
        if recipient_id == context.actor.user_id {
            return Err(ConnectionRequestError::SelfRequest);
        }

        Ok(vec![ConnectionRequestEvent::ConnectionRequestSent {
            connection_request_id: ConnectionRequestId(
                context.generated_id_generator.generate_object_id(),
            ),
            sender_id: context.actor.user_id,
            recipient_id,
            sent_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `ConnectionRequest` by folding its first event
    /// (`ConnectionRequestSent`).
    ///
    /// # Panics
    /// Panics if `event` is not a `ConnectionRequestSent` event.
    pub fn from_created_event(event: &ConnectionRequestEvent) -> Self {
        let ConnectionRequestEvent::ConnectionRequestSent {
            connection_request_id,
            sender_id,
            recipient_id,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-ConnectionRequestSent event");
        };

        Self {
            id: *connection_request_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: ConnectionRequestStatus::Pending,
            sender_id: *sender_id,
            recipient_id: *recipient_id,
        }
    }

    /// The request's current status.
    pub fn status(&self) -> ConnectionRequestStatus {
        self.status
    }

    /// Who sent the request.
    pub fn sender_id(&self) -> UserId {
        self.sender_id
    }

    /// Who the request is to.
    pub fn recipient_id(&self) -> UserId {
        self.recipient_id
    }

    fn invalid(&self, command: &str) -> ConnectionRequestError {
        ConnectionRequestError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for ConnectionRequest {
    type Id = ConnectionRequestId;
    type Command = ConnectionRequestCommand;
    type Event = ConnectionRequestEvent;
    type Error = ConnectionRequestError;

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
        if !context
            .authority
            .is_authorized("communication.connection_request.command")
        {
            return Err(ConnectionRequestError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            ConnectionRequestCommand::SendConnectionRequest { .. } => {
                Err(ConnectionRequestError::InvalidTransition(
                    "SendConnectionRequest must be dispatched via ConnectionRequest::create(), \
                     not decide()"
                        .to_string(),
                ))
            }

            ConnectionRequestCommand::AcceptConnectionRequest => match self.status {
                ConnectionRequestStatus::Pending => {
                    if context.actor.user_id != self.recipient_id {
                        return Err(ConnectionRequestError::Unauthorized(
                            "only the recipient may accept a connection request".to_string(),
                        ));
                    }
                    Ok(vec![ConnectionRequestEvent::ConnectionRequestAccepted {
                        accepted_at: context.trusted_now,
                    }])
                }
                _ => Err(self.invalid("AcceptConnectionRequest")),
            },

            ConnectionRequestCommand::DeclineConnectionRequest => match self.status {
                ConnectionRequestStatus::Pending => {
                    if context.actor.user_id != self.recipient_id {
                        return Err(ConnectionRequestError::Unauthorized(
                            "only the recipient may decline a connection request".to_string(),
                        ));
                    }
                    Ok(vec![ConnectionRequestEvent::ConnectionRequestDeclined {
                        declined_at: context.trusted_now,
                    }])
                }
                _ => Err(self.invalid("DeclineConnectionRequest")),
            },

            ConnectionRequestCommand::RevokeConnectionRequest => match self.status {
                ConnectionRequestStatus::Pending => {
                    if context.actor.user_id != self.sender_id {
                        return Err(ConnectionRequestError::Unauthorized(
                            "only the sender may revoke a connection request".to_string(),
                        ));
                    }
                    Ok(vec![ConnectionRequestEvent::ConnectionRequestRevoked {
                        revoked_at: context.trusted_now,
                    }])
                }
                _ => Err(self.invalid("RevokeConnectionRequest")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            ConnectionRequestEvent::ConnectionRequestSent { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            ConnectionRequestEvent::ConnectionRequestAccepted { .. } => {
                self.status = ConnectionRequestStatus::Accepted;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            ConnectionRequestEvent::ConnectionRequestDeclined { .. } => {
                self.status = ConnectionRequestStatus::Declined;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            ConnectionRequestEvent::ConnectionRequestRevoked { .. } => {
                self.status = ConnectionRequestStatus::Revoked;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
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
                parent_supergroup: None,
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
                    is_parent_supergroup_member: false,
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
                is_parent_supergroup_member: false,
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
                is_parent_supergroup_member: false,
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

    // ---- Supergroup / SubTeam (Phase 1 addition) ------------------------

    #[test]
    fn create_sub_team_without_parent_is_rejected() {
        let ctx = test_context();
        let result = Conversation::create(
            ConversationCommand::CreateConversation {
                conversation_type: ConversationType::SubTeam,
                parent_supergroup: None,
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(ConversationError::InvalidParentSupergroup(_))
        ));
    }

    #[test]
    fn create_non_sub_team_with_parent_is_rejected() {
        let ctx = test_context();
        let bogus_parent = crate::value::ConversationId::new_random();
        let result = Conversation::create(
            ConversationCommand::CreateConversation {
                conversation_type: ConversationType::Channel,
                parent_supergroup: Some(bogus_parent),
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(ConversationError::InvalidParentSupergroup(_))
        ));
    }

    #[test]
    fn create_sub_team_with_parent_succeeds() {
        let supergroup = crate::test_support::active_supergroup();
        let sub_team = crate::test_support::active_sub_team(*supergroup.id());
        assert_eq!(sub_team.conversation_type(), ConversationType::SubTeam);
        assert_eq!(sub_team.parent_supergroup(), Some(*supergroup.id()));
    }

    #[test]
    fn add_member_to_sub_team_without_supergroup_membership_is_rejected() {
        // "Supergroup membership required first" — the explicit product
        // decision this test locks in (PLAN_Desktop_Web_Completion.md §7.1).
        let supergroup = crate::test_support::active_supergroup();
        let sub_team = crate::test_support::active_sub_team(*supergroup.id());
        let ctx = test_context();
        let candidate = test_user_id();

        let result = sub_team.decide(
            ConversationCommand::AddMember {
                user_id: candidate,
                is_parent_supergroup_member: false,
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(ConversationError::NotSupergroupMember)
        ));
    }

    #[test]
    fn add_member_to_sub_team_with_supergroup_membership_succeeds() {
        let supergroup = crate::test_support::active_supergroup();
        let sub_team = crate::test_support::active_sub_team(*supergroup.id());
        let ctx = test_context();
        let candidate = test_user_id();

        let events = sub_team
            .decide(
                ConversationCommand::AddMember {
                    user_id: candidate,
                    is_parent_supergroup_member: true,
                },
                &ctx,
            )
            .expect("AddMember must succeed once the caller confirms supergroup membership");
        let mut updated = sub_team.clone();
        for e in &events {
            updated.apply(e);
        }
        assert!(updated.members().contains(&candidate));
    }

    #[test]
    fn add_member_to_non_sub_team_ignores_supergroup_membership_flag() {
        // A plain Channel has no parent supergroup to check against —
        // is_parent_supergroup_member is simply irrelevant there and
        // must not block an otherwise-valid AddMember.
        let conversation = active_conversation();
        let ctx = test_context();
        let candidate = test_user_id();

        let result = conversation.decide(
            ConversationCommand::AddMember {
                user_id: candidate,
                is_parent_supergroup_member: false,
            },
            &ctx,
        );
        assert!(result.is_ok());
    }

    // ---- ConnectionRequest (Phase 1 addition) ---------------------------

    #[test]
    fn send_connection_request_succeeds() {
        let ctx = test_context();
        let recipient = test_user_id();
        let events = ConnectionRequest::create(
            ConnectionRequestCommand::SendConnectionRequest {
                recipient_id: recipient,
            },
            &ctx,
        )
        .expect("create must succeed");
        let request = ConnectionRequest::from_created_event(&events[0]);

        assert_eq!(request.status(), ConnectionRequestStatus::Pending);
        assert_eq!(request.sender_id(), ctx.actor.user_id);
        assert_eq!(request.recipient_id(), recipient);
    }

    #[test]
    fn send_connection_request_to_self_is_rejected() {
        let ctx = test_context();
        let result = ConnectionRequest::create(
            ConnectionRequestCommand::SendConnectionRequest {
                recipient_id: ctx.actor.user_id,
            },
            &ctx,
        );
        assert!(matches!(result, Err(ConnectionRequestError::SelfRequest)));
    }

    #[test]
    fn recipient_can_accept_pending_request() {
        let request = crate::test_support::pending_connection_request();
        let mut ctx = test_context();
        ctx.actor.user_id = request.recipient_id();

        let events = request
            .decide(ConnectionRequestCommand::AcceptConnectionRequest, &ctx)
            .expect("recipient must be able to accept");
        let mut updated = request.clone();
        for e in &events {
            updated.apply(e);
        }
        assert_eq!(updated.status(), ConnectionRequestStatus::Accepted);
    }

    #[test]
    fn non_recipient_cannot_accept_pending_request() {
        let request = crate::test_support::pending_connection_request();
        let ctx = test_context(); // a third, unrelated actor
        let result = request.decide(ConnectionRequestCommand::AcceptConnectionRequest, &ctx);
        assert!(matches!(
            result,
            Err(ConnectionRequestError::Unauthorized(_))
        ));
    }

    #[test]
    fn recipient_can_decline_pending_request() {
        let request = crate::test_support::pending_connection_request();
        let mut ctx = test_context();
        ctx.actor.user_id = request.recipient_id();

        let events = request
            .decide(ConnectionRequestCommand::DeclineConnectionRequest, &ctx)
            .expect("recipient must be able to decline");
        let mut updated = request.clone();
        for e in &events {
            updated.apply(e);
        }
        assert_eq!(updated.status(), ConnectionRequestStatus::Declined);
    }

    #[test]
    fn sender_can_revoke_pending_request() {
        let request = crate::test_support::pending_connection_request();
        let mut ctx = test_context();
        ctx.actor.user_id = request.sender_id();

        let events = request
            .decide(ConnectionRequestCommand::RevokeConnectionRequest, &ctx)
            .expect("sender must be able to revoke");
        let mut updated = request.clone();
        for e in &events {
            updated.apply(e);
        }
        assert_eq!(updated.status(), ConnectionRequestStatus::Revoked);
    }

    #[test]
    fn non_sender_cannot_revoke_pending_request() {
        let request = crate::test_support::pending_connection_request();
        let ctx = test_context(); // a third, unrelated actor
        let result = request.decide(ConnectionRequestCommand::RevokeConnectionRequest, &ctx);
        assert!(matches!(
            result,
            Err(ConnectionRequestError::Unauthorized(_))
        ));
    }

    #[test]
    fn accept_on_already_accepted_request_is_rejected() {
        let request = crate::test_support::pending_connection_request();
        let mut ctx = test_context();
        ctx.actor.user_id = request.recipient_id();
        let events = request
            .decide(ConnectionRequestCommand::AcceptConnectionRequest, &ctx)
            .unwrap();
        let mut accepted = request.clone();
        for e in &events {
            accepted.apply(e);
        }

        let result = accepted.decide(ConnectionRequestCommand::AcceptConnectionRequest, &ctx);
        assert!(matches!(
            result,
            Err(ConnectionRequestError::InvalidTransition(_))
        ));
    }

    #[test]
    fn send_connection_request_called_via_decide_is_rejected() {
        let request = crate::test_support::pending_connection_request();
        let ctx = test_context();
        let result = request.decide(
            ConnectionRequestCommand::SendConnectionRequest {
                recipient_id: test_user_id(),
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(ConnectionRequestError::InvalidTransition(_))
        ));
    }

    // ---- Serde compatibility (Phase 1 addition regression guard) -------

    #[test]
    fn add_member_json_without_new_field_defaults_to_false() {
        // Verifies the #[serde(default)] annotation actually works, so a
        // JSON payload written before is_parent_supergroup_member
        // existed (e.g. from an already-deployed client, or any
        // Channel/Direct caller that never needs to set it) still
        // deserializes rather than erroring on a "missing field".
        let json = serde_json::json!({
            "AddMember": { "user_id": test_user_id() }
        });
        let command: ConversationCommand = serde_json::from_value(json)
            .expect("AddMember must deserialize even without is_parent_supergroup_member");
        assert!(matches!(
            command,
            ConversationCommand::AddMember {
                is_parent_supergroup_member: false,
                ..
            }
        ));
    }

    #[test]
    fn create_conversation_json_without_parent_supergroup_defaults_to_none() {
        let json = serde_json::json!({
            "CreateConversation": { "conversation_type": "Direct" }
        });
        let command: ConversationCommand = serde_json::from_value(json)
            .expect("CreateConversation must deserialize even without parent_supergroup");
        assert!(matches!(
            command,
            ConversationCommand::CreateConversation {
                parent_supergroup: None,
                ..
            }
        ));
    }
}
