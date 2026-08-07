//! Causal ordering primitives.
//! Source: Handover Document §1 "causality" (lines 90-116).

use crate::versioning::ReplicaId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Causal ordering between two vector clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CausalRelation {
    Before,
    After,
    Equal,
    Concurrent,
}

/// Causal frontier; maps replica to its event count.
/// Serialized as a JSON object with replica_id: count pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock(pub BTreeMap<ReplicaId, u64>);

impl VectorClock {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Returns BEFORE, AFTER, EQUAL, or CONCURRENT.
    pub fn compare(&self, other: &Self) -> CausalRelation {
        let mut self_less = false;
        let mut other_less = false;

        let mut keys: std::collections::BTreeSet<&ReplicaId> = std::collections::BTreeSet::new();
        for k in self.0.keys() {
            keys.insert(k);
        }
        for k in other.0.keys() {
            keys.insert(k);
        }

        for k in keys {
            let a = self.0.get(k).copied().unwrap_or(0);
            let b = other.0.get(k).copied().unwrap_or(0);
            if a < b {
                self_less = true;
            } else if a > b {
                other_less = true;
            }
        }

        match (self_less, other_less) {
            (false, false) => CausalRelation::Equal,
            (true, false) => CausalRelation::Before,
            (false, true) => CausalRelation::After,
            (true, true) => CausalRelation::Concurrent,
        }
    }

    /// Componentwise max of two vector clocks.
    pub fn merge(&self, other: &Self) -> Self {
        let mut result = self.0.clone();
        for (k, v) in other.0.iter() {
            let entry = result.entry(k.clone()).or_insert(0);
            if *v > *entry {
                *entry = *v;
            }
        }
        Self(result)
    }

    /// Increments count for the given replica.
    pub fn increment(&mut self, replica: ReplicaId) {
        let entry = self.0.entry(replica).or_insert(0);
        *entry += 1;
    }
}
