//! PN-Counter: State-based counter with increments and decrements.
//! Source: Team Prompt 3 §3.5. Tracks per-replica increments and decrements
//! separately; merge takes the per-replica max (standard PN-Counter merge),
//! matching Blueprint §7.5.3 ("summed at merge time" — summation happens
//! once, at `value()` read time, over the per-replica maxima merged here).

use platform_kernel::{ReplicaId, VectorClock};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::Crdt;

/// A state-based counter that can be incremented and decremented from
/// multiple replicas concurrently, converging to the same total everywhere.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PnCounter {
    inc: BTreeMap<ReplicaId, u64>,
    dec: BTreeMap<ReplicaId, u64>,
    clock: VectorClock,
}

impl PnCounter {
    /// Create a counter starting at zero.
    pub fn new() -> Self {
        Self {
            inc: BTreeMap::new(),
            dec: BTreeMap::new(),
            clock: VectorClock::new(),
        }
    }

    /// Increment the counter by `n`, attributed to `replica`.
    pub fn increment(&mut self, replica: ReplicaId, n: u64, clock: VectorClock) {
        *self.inc.entry(replica).or_insert(0) += n;
        self.clock = self.clock.merge(&clock);
    }

    /// Decrement the counter by `n`, attributed to `replica`.
    pub fn decrement(&mut self, replica: ReplicaId, n: u64, clock: VectorClock) {
        *self.dec.entry(replica).or_insert(0) += n;
        self.clock = self.clock.merge(&clock);
    }

    /// The counter's current value (sum of all per-replica increments,
    /// minus the sum of all per-replica decrements).
    pub fn value(&self) -> i64 {
        let inc_sum: u64 = self.inc.values().sum();
        let dec_sum: u64 = self.dec.values().sum();
        (inc_sum as i64) - (dec_sum as i64)
    }

    /// The causal context for GC.
    pub fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}

impl Default for PnCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Crdt for PnCounter {
    fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;

        for (replica, count) in &other.inc {
            let entry = self.inc.entry(*replica).or_insert(0);
            if *entry < *count {
                *entry = *count;
                changed = true;
            }
        }

        for (replica, count) in &other.dec {
            let entry = self.dec.entry(*replica).or_insert(0);
            if *entry < *count {
                *entry = *count;
                changed = true;
            }
        }

        let new_clock = self.clock.merge(&other.clock);
        if self.clock != new_clock {
            self.clock = new_clock;
            changed = true;
        }

        changed
    }

    fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}
