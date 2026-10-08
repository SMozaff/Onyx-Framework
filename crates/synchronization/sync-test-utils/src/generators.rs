//! Proptest generators for CRDT determinism property tests (§6.1).

use crdt::Tag;
use platform_kernel::ReplicaId;
use proptest::collection::vec;
use proptest::prelude::*;

/// A single OR-Set operation for the property-test generator.
#[derive(Clone, Debug)]
pub enum OrSetOperation {
    /// Add `element` under the given tag.
    Add(String, Tag),
    /// Remove `element` under the given tag.
    Remove(String, Tag),
}

/// Generates a bounded list of OR-Set operations over a small alphabet of
/// elements and a small pool of replica ids, so that adds/removes actually
/// collide/interact in interesting ways rather than each being independent.
pub fn arbitrary_or_set_operations(
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

    vec(op_strategy, size)
}
