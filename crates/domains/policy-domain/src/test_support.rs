//! Deterministic test fixtures for `policy-domain`'s test suites.
//!
//! Deliberately self-contained rather than reusing another domain crate's
//! fixtures — same rationale as `file_domain::test_support`'s doc
//! comment.

use platform_contracts::{DecisionContext, IdGenerator};
use platform_kernel::{
    ActorContext, EventId, ObjectId, OperationId, PolicyDecisionSet, Timestamp, UserId,
    VerifiedAuthority,
};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::aggregate::{LegalHold, Policy};
use crate::command::{LegalHoldCommand, PolicyCommand};
use crate::value::{HoldTarget, PolicyRule, PolicyScope, PolicyType};
use platform_contracts::AggregateRoot;

/// A deterministic, counter-based `IdGenerator` for tests. See
/// `mission_domain::test_support::DeterministicIdGenerator`'s doc comment
/// for the full rationale.
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
/// `file_domain::test_support::TEST_CONTEXT_SEED`.
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

/// Generates a fresh random `UserId` for use as an actor in tests.
pub fn test_user_id() -> UserId {
    ObjectId::new_random()
}

/// A single feature-toggle rule fixture (`{key}.enabled` = `enabled`).
pub fn toggle_rule(key: &str, enabled: bool) -> PolicyRule {
    PolicyRule {
        rule_type: PolicyType::FeatureToggle,
        key: key.to_string(),
        value: serde_json::json!(enabled),
    }
}

/// A freshly created, `Active` policy with an organization scope and no
/// versions yet.
pub fn active_policy() -> Policy {
    let ctx = test_context();
    let events = Policy::create(
        PolicyCommand::CreatePolicy {
            name: "Default Org Governance".to_string(),
            scope: PolicyScope::Organization(ctx.actor.organization_id),
        },
        &ctx,
    )
    .expect("create must succeed");
    Policy::from_created_event(&events[0])
}

/// An `Active` policy with one published version containing the given
/// rules — the common starting point for `EvaluatePolicy` tests.
pub fn policy_with_published_rules(rules: Vec<PolicyRule>) -> Policy {
    let mut policy = active_policy();
    let ctx = test_context();

    let create_events = policy
        .decide(PolicyCommand::CreatePolicyVersion { rules }, &ctx)
        .expect("create version must succeed");
    for event in &create_events {
        policy.apply(event);
    }

    let publish_events = policy
        .decide(PolicyCommand::PublishPolicyVersion, &ctx)
        .expect("publish must succeed");
    for event in &publish_events {
        policy.apply(event);
    }

    policy
}

/// A freshly applied, `Applied` legal hold on an arbitrary file-asset-like
/// target.
pub fn applied_legal_hold() -> LegalHold {
    let ctx = test_context();
    let events = LegalHold::create(
        LegalHoldCommand::ApplyLegalHold {
            target: HoldTarget {
                target_id: ObjectId::new_random(),
                target_type: "file_asset".to_string(),
            },
            authorizing_policy: None,
            reason: "pending litigation".to_string(),
        },
        &ctx,
    )
    .expect("create must succeed");
    LegalHold::from_created_event(&events[0])
}
