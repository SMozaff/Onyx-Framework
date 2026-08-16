//! Deterministic test fixtures for `profile-domain`'s test suites.

use platform_contracts::{DecisionContext, IdGenerator};
use platform_kernel::{
    ActorContext, EventId, ObjectId, OperationId, PolicyDecisionSet, Timestamp, VerifiedAuthority,
};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::aggregate::StaffProfile;
use crate::command::ProfileCommand;
use crate::value::{BasicIdentity, OrganizationalInfo};

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

/// A freshly created, `Active` profile with minimal identity and no
/// organizational info set.
pub fn active_profile() -> StaffProfile {
    let ctx = test_context();
    let events = StaffProfile::create(
        ProfileCommand::CreateProfile {
            owner_id: ctx.actor.user_id,
            identity: BasicIdentity::new("Test User", None, None, None, None)
                .expect("valid identity"),
            organizational_info: OrganizationalInfo {
                department: None,
                class_label: None,
                parent_display_name: None,
                team_memberships: vec![],
            },
        },
        &ctx,
    )
    .expect("create must succeed");
    StaffProfile::from_created_event(&events[0])
}
