//! Deterministic test fixtures shared across `mission-domain`'s test suites,
//! and re-exported for `work-domain` to reuse.

use platform_contracts::{DecisionContext, IdGenerator};
use platform_kernel::{
    ActorContext, EventId, ObjectId, OperationId, PolicyDecisionSet, Timestamp, UserId,
    VerifiedAuthority,
};
use std::sync::atomic::{AtomicU64, Ordering};

/// A deterministic, counter-based `IdGenerator` for tests. Not
/// cryptographically random by design — reproducibility matters more than
/// randomness in a test fixture. Uses an atomic counter (rather than
/// `Cell`) because [`IdGenerator`] requires `Sync`.
pub struct DeterministicIdGenerator {
    counter: AtomicU64,
}

impl DeterministicIdGenerator {
    /// Constructs a generator whose first generated id is `1`.
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(1),
        }
    }

    /// Constructs a generator whose first generated id is `seed`. Used by
    /// `test_context()` (via a process-wide atomic seed counter) so that
    /// concurrently-running integration tests each get a distinct id
    /// space and cannot collide on the same generated `ObjectId` — see
    /// Architectural Ruling "Test Isolation Fix (Deterministic ID
    /// Collision)", DECISIONS.md item T1 (Increment 2).
    pub fn new_seeded(seed: u64) -> Self {
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

impl Default for DeterministicIdGenerator {
    fn default() -> Self {
        Self::new()
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

/// Process-wide seed counter ensuring every call to `test_context()`
/// starts its `DeterministicIdGenerator` at a fresh, non-overlapping
/// range (spaced widely apart so a single test generating many ids can
/// never run into the next test's range). Per Architectural Ruling "Test
/// Isolation Fix (Deterministic ID Collision)" — DECISIONS.md item T1
/// (Increment 2). This does not affect `DeterministicIdGenerator::new()`
/// or its `Default` impl, which Team 1's own existing unit tests may
/// depend on starting at exactly `1`.
static TEST_CONTEXT_SEED: AtomicU64 = AtomicU64::new(1);

/// Builds a `DecisionContext` suitable for deterministic tests: a fixed
/// `trusted_now`, a stub-authorized `VerifiedAuthority` (per Increment 1
/// ruling E), and a fresh `DeterministicIdGenerator` seeded from a
/// process-wide counter so concurrently-running tests never collide on
/// generated ids (Increment 2 ruling T1).
pub fn test_context() -> DecisionContext {
    // Reserve a wide range (1,000,000 ids) per call so that no single
    // test can plausibly generate enough ids to collide with the next
    // test's range.
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

/// Generates a fresh random `UserId` for use as a mission/task owner in
/// tests.
pub fn test_user_id() -> UserId {
    ObjectId::new_random()
}
