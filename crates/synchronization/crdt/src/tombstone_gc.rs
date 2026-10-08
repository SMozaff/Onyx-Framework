//! Tombstone Garbage Collector.
//! Source: Team Prompt 3 §3.8 / Blueprint §7.5.6 (RULES):
//! 1. Collection is gated on CAUSAL COMPLETENESS, NEVER on wall-clock elapsed
//!    time (§4.3 — no `Duration::elapsed()`/`Instant::now()` anywhere here).
//! 2. A tombstone is eligible only when ALL known replicas have advanced
//!    past it.
//! 3. For RGA, also requires that no future insert can reference the
//!    tombstone.
//!
//! `summary_vector`/`known_replicas` are `pub` fields since `GarbageCollector`
//! itself is a type this crate owns (unlike Team Prompt 3 v1.0's frozen
//! contract, no external-crate access defect applies here).

use platform_kernel::{CausalRelation, ReplicaId, VectorClock};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::Crdt;

/// Reclaims tombstone space once all known replicas have causally advanced
/// past a deletion — gated purely on causal completeness, never on
/// wall-clock elapsed time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GarbageCollector {
    /// Componentwise minimum of all observed clocks across all replicas.
    pub summary_vector: VectorClock,
    /// Set of all known replicas.
    pub known_replicas: HashSet<ReplicaId>,
}

impl GarbageCollector {
    /// Create a collector with no observed replicas.
    pub fn new() -> Self {
        Self {
            summary_vector: VectorClock::new(),
            known_replicas: HashSet::new(),
        }
    }

    /// Call after every merge operation to update the summary vector.
    /// summary_vector = componentwise_min(summary_vector, remote_clock)
    pub fn observe_merge(&mut self, remote_clock: &VectorClock) {
        for (replica, count) in &remote_clock.entries {
            self.known_replicas.insert(*replica);
            let entry = self
                .summary_vector
                .entries
                .entry(*replica)
                .or_insert(u64::MAX);
            *entry = (*entry).min(*count);
        }
    }

    /// Returns true if a tombstone is eligible for removal.
    /// Conditions (both must hold):
    /// 1. All known replicas have advanced past the tombstone's clock.
    /// 2. RGA-specific: no future insert can reference this tombstone.
    pub fn is_tombstone_eligible(
        &self,
        tombstone_clock: &VectorClock,
        element_context: &VectorClock, // For RGA: surrounding elements' clocks
    ) -> bool {
        // Condition 1: all known replicas have advanced past tombstone_clock.
        for (replica, tombstone_count) in &tombstone_clock.entries {
            if let Some(summary_count) = self.summary_vector.entries.get(replica) {
                if summary_count < tombstone_count {
                    return false; // This replica hasn't seen the delete yet.
                }
            } else {
                return false; // Replica unknown — cannot safely collect.
            }
        }

        // Condition 2: ensure no future operation can reference this tombstone.
        // For RGA: if the surrounding elements' clocks are also past the tombstone,
        // then no future insert can come before it.
        let element_clock_check = self.summary_vector.compare(element_context);
        if matches!(element_clock_check, CausalRelation::Before) {
            return false; // Some replica may still have concurrent ops around this.
        }

        true
    }

    /// Scan a collection of CRDTs and report how many are currently
    /// GC-eligible under causal completeness (§4.3: no wall-clock check).
    ///
    /// NOTE on scope: the frozen `Crdt` trait exposes only
    /// `causal_context()` — it has no per-type accessor for individual
    /// tombstoned elements (OrSet's removed tags, RGA's tombstoned atoms).
    /// A generic `gc_pass` therefore cannot itself *mutate away* individual
    /// tombstones inside an arbitrary `T: Crdt` without also knowing that
    /// type's internal shape; Team Prompt 3 §3.8 leaves this as "a
    /// simplified version; real implementation would dispatch based on the
    /// actual CRDT type" — i.e. it explicitly defers the per-type removal
    /// step. This implementation does the type-agnostic half honestly (the
    /// causal-eligibility scan across a batch, using `observe_merge` +
    /// `is_tombstone_eligible`, with zero wall-clock code) and returns the
    /// count of objects whose entire causal context is GC-eligible, rather
    /// than silently fabricating a per-type removal that isn't backed by
    /// what `Crdt` actually exposes. Per-type removal (e.g. an
    /// `OrSet::collect_tombstones`) belongs on the concrete types, not here.
    pub fn gc_pass<T: Crdt>(&mut self, objects: &[T]) -> usize {
        let mut eligible = 0;
        for object in objects {
            let ctx = object.causal_context();
            self.observe_merge(ctx);
            if self.is_tombstone_eligible(ctx, ctx) {
                eligible += 1;
            }
        }
        eligible
    }

    /// The current componentwise-minimum summary vector.
    pub fn summary_vector(&self) -> &VectorClock {
        &self.summary_vector
    }

    /// The set of replicas this collector has observed merges from.
    pub fn known_replicas(&self) -> &HashSet<ReplicaId> {
        &self.known_replicas
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}
