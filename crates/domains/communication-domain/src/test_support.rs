//! Deterministic test fixtures for `communication-domain`'s test suites.
//!
//! Deliberately self-contained rather than reusing
//! `mission_domain::test_support`: this crate has no real business
//! dependency on `mission-domain` the way `work-domain` does (a Task's
//! `TaskMissionRef = MissionId` is a genuine invariant — "every Task
//! belongs to exactly one Mission"). Adding a dependency on a sibling
//! domain crate purely to borrow its test fixtures would be a real
//! dependency for a fixture-only reason, so the small amount of
//! duplication here is the correct trade, not an oversight.

use platform_contracts::{AggregateRoot, DecisionContext, IdGenerator};
use platform_kernel::{
    ActorContext, EventId, ObjectId, OperationId, PolicyDecisionSet, Timestamp, UserId,
    VerifiedAuthority,
};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::aggregate::{Conversation, Message};
use crate::command::{ConversationCommand, MessageCommand};
use crate::value::ConversationType;

/// A deterministic, counter-based `IdGenerator` for tests. See
/// `mission_domain::test_support::DeterministicIdGenerator`'s doc comment
/// for the full rationale (reproducibility over randomness; `Sync` via
/// `AtomicU64` rather than `Cell`) — identical reasoning applies here.
struct DeterministicIdGenerator {
    counter: AtomicU64,
}

impl DeterministicIdGenerator {
    fn new_seeded(seed: u64) -> Self {
        Self {
            counter: AtomicU64::new(seed),
        }
    }

    fn next_bytes(&self) -> [u8; 16] {
        let value = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut bytes = [0u8; 16];
        bytes[8..].copy_from_slice(&value.to_be_bytes());
        bytes
    }
}

impl IdGenerator for DeterministicIdGenerator {
    fn generate_object_id(&self) -> ObjectId {
        ObjectId(self.next_bytes())
    }
    fn generate_operation_id(&self) -> OperationId {
        OperationId(self.next_bytes())
    }
    fn generate_event_id(&self) -> EventId {
        EventId(self.next_bytes())
    }
}

/// Process-wide seed counter, same isolation rationale as
/// `mission_domain::test_support::TEST_CONTEXT_SEED`: concurrently-running
/// tests must never collide on a generated `ObjectId`.
static TEST_CONTEXT_SEED: AtomicU64 = AtomicU64::new(1);

/// Builds a `DecisionContext` suitable for deterministic tests.
pub fn test_context() -> DecisionContext {
    let seed = TEST_CONTEXT_SEED.fetch_add(1_000_000, Ordering::SeqCst);
    let org = ObjectId::new_random();
    DecisionContext {
        actor: ActorContext {
            user_id: ObjectId::new_random(),
            device_id: ObjectId::new_random(),
            organization_id: org,
        },
        authority: VerifiedAuthority,
        trusted_now: Timestamp::from_nanos(1_700_000_000_000_000_000),
        policy_outcomes: PolicyDecisionSet,
        generated_id_generator: Box::new(DeterministicIdGenerator::new_seeded(seed)),
    }
}

/// Generates a fresh random `UserId` for use as an actor/author in tests.
pub fn test_user_id() -> UserId {
    ObjectId::new_random()
}

/// A freshly created, `Active` conversation with one member (the creator).
pub fn active_conversation() -> Conversation {
    let ctx = test_context();
    let events = Conversation::create(
        ConversationCommand::CreateConversation {
            conversation_type: ConversationType::Channel,
        },
        &ctx,
    )
    .expect("create must succeed");
    Conversation::from_created_event(&events[0])
}

/// A freshly posted, `Posted`-status message in a fresh conversation.
pub fn posted_message() -> Message {
    let ctx = test_context();
    let conversation = active_conversation();
    let events = Message::create(
        MessageCommand::PostMessage {
            conversation_id: *conversation.id(),
            body: "hello team".to_string(),
        },
        &ctx,
    )
    .expect("create must succeed");
    Message::from_created_event(&events[0])
}
