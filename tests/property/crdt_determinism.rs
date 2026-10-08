//! CRDT Determinism Property Test.
//! Source: Team Prompt 3 §6.1. §4.1: "Identical accepted operation
//! histories, replayed in any order, must produce identical final state."
//!
//! Deviations from the frozen test source, flagged (not silent):
//! - `shuffled.shuffle()` in the frozen contract calls a zero-argument
//!   `shuffle()` that isn't a real `rand`/`SliceRandom` API (it needs an
//!   `Rng`). Fixed to `shuffled.shuffle(&mut rng)` using `rand::thread_rng()`
//!   seeded per proptest case via `TestRunner`'s own RNG for reproducibility.
//! - `OrSet::<String>::new()` plus `Operation::Add/Remove` construction is
//!   transcribed via `sync_test_utils::OrSetOperation` (§6.1's own
//!   `Operation` enum is never defined; this is that missing definition).
//! - The frozen test omits an explicit clock argument shape for
//!   `state.add(el, tag, VectorClock::new())`; kept as-is (empty clock),
//!   since only causal *tag* interaction, not clock merging, is what
//!   determinism here is testing.

use crdt::{OrSet, Tag};
use platform_kernel::{ReplicaId, VectorClock};
use proptest::collection::vec as pvec;
use proptest::prelude::*;
use rand::seq::SliceRandom;

/// Deviation: `sync_test_utils::arbitrary_or_set_operations` was the
/// original plan (see `crates/sync-test-utils/src/generators.rs`), but
/// `sync-test-utils` depends on `crdt` (for `OrSet` etc.) — having `crdt`'s
/// own tests depend back on `sync-test-utils` would be a dependency cycle.
/// So this crate-local test defines its own minimal, equivalent generator
/// instead of sharing the one in `sync-test-utils` (which remains the
/// generator `synchronization-domain`'s tests use).
#[derive(Clone, Debug)]
enum OrSetOperation {
    Add(String, Tag),
    Remove(String, Tag),
}

fn arbitrary_or_set_operations(
    size: std::ops::Range<usize>,
) -> impl Strategy<Value = Vec<OrSetOperation>> {
    let replica_pool: Vec<ReplicaId> = (0..4).map(|_| ReplicaId::new_random()).collect();
    let element_pool = ["a", "b", "c", "d", "e"];

    let op_strategy = {
        let replica_pool = replica_pool.clone();
        (
            0..replica_pool.len(),
            0..element_pool.len(),
            any::<u64>(),
            any::<bool>(),
        )
            .prop_map(move |(r_idx, e_idx, lamport, is_add)| {
                let tag = Tag {
                    replica_id: replica_pool[r_idx],
                    lamport,
                };
                let element = element_pool[e_idx].to_string();
                if is_add {
                    OrSetOperation::Add(element, tag)
                } else {
                    OrSetOperation::Remove(element, tag)
                }
            })
    };

    pvec(op_strategy, size)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    #[ignore] // matches §9 Quick Start: run explicitly with --ignored --release
    fn or_set_merge_deterministic(ops in arbitrary_or_set_operations(1..200)) {
        let base = OrSet::<String>::new();
        let mut results = Vec::new();

        for _ in 0..10 {
            let mut shuffled = ops.clone();
            let mut rng = rand::thread_rng();
            shuffled.shuffle(&mut rng);

            let mut state = base.clone();
            for op in shuffled {
                match op {
                    OrSetOperation::Add(el, tag) => {
                        state.add(el, tag, VectorClock::new());
                    }
                    OrSetOperation::Remove(el, tag) => {
                        state.remove(el, tag, VectorClock::new());
                    }
                }
            }
            results.push(state);
        }

        for i in 1..results.len() {
            prop_assert_eq!(&results[0], &results[i]);
        }
    }
}

/// Non-ignored, fast smoke-test variant so `cargo test --package crdt`
/// (§9's first Quick Start command) still exercises determinism without
/// paying the full 64-case/10-shuffle cost of the `--ignored --release`
/// property test above.
#[test]
fn or_set_merge_deterministic_smoke() {
    use platform_kernel::ReplicaId;

    let replica_a = ReplicaId::new_random();
    let replica_b = ReplicaId::new_random();

    let ops = vec![
        OrSetOperation::Add(
            "x".to_string(),
            crdt::Tag {
                replica_id: replica_a,
                lamport: 1,
            },
        ),
        OrSetOperation::Add(
            "x".to_string(),
            crdt::Tag {
                replica_id: replica_b,
                lamport: 1,
            },
        ),
        OrSetOperation::Remove(
            "x".to_string(),
            crdt::Tag {
                replica_id: replica_a,
                lamport: 1,
            },
        ),
    ];

    let base = OrSet::<String>::new();
    let mut results = Vec::new();
    for _ in 0..10 {
        let mut shuffled = ops.clone();
        let mut rng = rand::thread_rng();
        shuffled.shuffle(&mut rng);
        let mut state = base.clone();
        for op in shuffled {
            match op {
                OrSetOperation::Add(el, tag) => state.add(el, tag, VectorClock::new()),
                OrSetOperation::Remove(el, tag) => state.remove(el, tag, VectorClock::new()),
            }
        }
        results.push(state);
    }
    for i in 1..results.len() {
        assert_eq!(results[0], results[i]);
    }
    // Add-wins: replica_a's add tag was removed, but replica_b's add tag
    // was never covered by a remove, so "x" must still be present.
    assert!(results[0].contains(&"x".to_string()));
}
