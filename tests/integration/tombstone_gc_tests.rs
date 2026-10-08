//! Tombstone GC Correctness Test.
//! Source: Team Prompt 3 §6.3, rebuilt against the real
//! `platform_kernel::VectorClock` (Architectural Ruling Q1), which has no
//! `.with()` fluent builder (Team Prompt 3 v1's frozen test used one that
//! doesn't exist anywhere in either the Handover Document or Team 1's real
//! kernel) — clocks here are built via repeated `.increment()` calls
//! instead, which is the real, documented `VectorClock` API.
//!
//! `gc.summary_vector` is public-field access: `GarbageCollector` is a type
//! this crate (`crdt`) owns, so no external-crate access defect applies.
//!
//! Deviation flagged: the frozen source annotates this with `#[tokio::test]`
//! even though nothing in the test body is `async` — `GarbageCollector` has
//! no async methods anywhere in §3.8. Kept as a plain `#[test]`.

use crdt::GarbageCollector;
use platform_kernel::{ReplicaId, VectorClock};

fn clock(pairs: &[(ReplicaId, u64)]) -> VectorClock {
    let mut vc = VectorClock::new();
    for (replica, count) in pairs {
        for _ in 0..*count {
            vc.increment(*replica);
        }
    }
    vc
}

#[test]
fn tombstone_not_collected_until_all_replicas_advanced() {
    let mut gc = GarbageCollector::new();
    let replica_x = ReplicaId::new_random();
    let replica_y = ReplicaId::new_random();

    // Tombstone clock: x:5, y:3
    let tombstone_clock = clock(&[(replica_x, 5), (replica_y, 3)]);

    // Summary clock so far: x:3, y:2 (replica_x hasn't seen x:5 yet)
    let summary = clock(&[(replica_x, 3), (replica_y, 2)]);

    gc.summary_vector = summary;

    // Should NOT be eligible.
    assert!(!gc.is_tombstone_eligible(&tombstone_clock, &VectorClock::new()));

    // Now replica_x catches up.
    let new_summary = clock(&[(replica_x, 6), (replica_y, 3)]);
    gc.summary_vector = new_summary;

    // Now eligible.
    assert!(gc.is_tombstone_eligible(&tombstone_clock, &VectorClock::new()));
}

/// Additional coverage beyond the frozen §6.3 body: an unknown replica in
/// the tombstone clock must block collection ("Replica unknown — cannot
/// safely collect" per §3.8's own comment), exercising the second early
/// return in `is_tombstone_eligible` that the frozen test never reaches.
#[test]
fn tombstone_not_collected_for_unknown_replica() {
    let gc = GarbageCollector::new();
    let replica_unknown = ReplicaId::new_random();
    let tombstone_clock = clock(&[(replica_unknown, 1)]);

    assert!(!gc.is_tombstone_eligible(&tombstone_clock, &VectorClock::new()));
}
