//! MV-Register: Preserves all concurrent values.
//! Source: Team Prompt 3 §3.4. Use for fields where silently losing a
//! concurrent write is unacceptable. Surfaces all concurrent values to a
//! ConflictRecord for human review.
//!
//! Deviation from Team Prompt 3 v1.0 (defect B5 in the original report):
//! the frozen contract stores `BTreeMap<VectorClock, V>`, which requires
//! `VectorClock: Ord`. Under Team 3 v1's own invented kernel stub we added
//! that `Ord` impl ourselves. Under the REAL `platform_kernel::VectorClock`
//! (Architectural Ruling Q1, Team 3 Initiation) we cannot do that — it is
//! Team 1's authoritative crate, not ours to extend, and vector clocks are
//! only ever partially ordered by design (adding a fabricated total order
//! to the type itself risks it leaking into causal-comparison code
//! elsewhere and being used where it shouldn't be). Instead, concurrent
//! versions are stored as a `Vec<(VectorClock, V)>` — a small, deduplicated
//! set of versions, not a sorted map — which needs no `Ord` on
//! `VectorClock` at all. Lookups are O(n) but n is the number of
//! *concurrent* writers to one field, which is expected to be tiny (a
//! handful at most).
//!
//! Deviation (not itself a DECISIONS.md item but flagged per the "never
//! guess" instruction): Team Prompt 3's `MvRegister::new` calls
//! `ReplicaId::random()` with an inline comment "Should be provided by
//! context" — i.e. the spec author already knew this was a placeholder that
//! silently manufactures a random replica identity instead of using the
//! caller's real one. Manufacturing identity silently is exactly the kind
//! of non-determinism §4.1 forbids, so `new` here takes `replica: ReplicaId`
//! as an explicit parameter instead.

use platform_kernel::{ReplicaId, VectorClock};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::Crdt;

/// A field that preserves all concurrently-written values rather than
/// silently discarding one, until a human (or `MergeStrategy`) resolves
/// the conflict.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "V: Serialize", deserialize = "V: DeserializeOwned"))]
pub struct MvRegister<V: Clone + Debug + Serialize + DeserializeOwned> {
    /// The set of concurrent (clock, value) versions. Not a `BTreeMap`:
    /// see module doc comment for why `VectorClock` cannot be a map key
    /// here.
    values: Vec<(VectorClock, V)>,
    clock: VectorClock,
}

impl<V: Clone + Debug + Serialize + DeserializeOwned> MvRegister<V> {
    /// See module doc comment: `replica` is an explicit parameter, not
    /// silently randomized.
    pub fn new(value: V, replica: ReplicaId) -> Self {
        let mut clock = VectorClock::new();
        clock.increment(replica);
        Self {
            values: vec![(clock.clone(), value)],
            clock,
        }
    }

    /// Set a value with a causal context.
    /// Adds a new version entry; does NOT replace existing ones — that is
    /// the entire point of an MV-Register (preserve concurrent writes for
    /// human/`MergeStrategy` review rather than picking one silently).
    pub fn set(&mut self, value: V, clock: VectorClock) {
        if !self.values.iter().any(|(vc, _)| vc == &clock) {
            self.values.push((clock.clone(), value));
        }
        self.clock = self.clock.merge(&clock);
    }

    /// Get all concurrent values.
    pub fn get_all(&self) -> Vec<(&VectorClock, &V)> {
        self.values.iter().map(|(vc, v)| (vc, v)).collect()
    }

    /// For contexts where a single value is required: returns a value only
    /// when there is exactly one (no unresolved concurrency). Returns
    /// `None` when there are 0 or 2+ concurrent versions — deliberately,
    /// rather than silently picking one, since arbitrarily picking a
    /// "winner" among genuinely concurrent values is exactly what an
    /// MV-Register exists to avoid (contrast with LWW-Register, which
    /// picks deterministically by design). Callers that need a single
    /// value from a genuinely divergent register should go through
    /// `MergeStrategy`/`ConflictRecord`, not this method.
    pub fn get_single(&self) -> Option<&V> {
        if self.values.len() == 1 {
            self.values.first().map(|(_, v)| v)
        } else {
            None
        }
    }

    /// True if there are 2+ concurrent, unreconciled values.
    pub fn has_conflict(&self) -> bool {
        self.values.len() > 1
    }

    /// The causal context for GC.
    pub fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}

impl<V: Clone + Debug + Send + Sync + Serialize + DeserializeOwned> Crdt for MvRegister<V> {
    fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;

        // Union of all versions. A version already causally dominated by
        // an existing one is still kept — pruning subsumed versions would
        // require comparing every pair, which is a real future
        // optimization but not required for correctness (merge stays a
        // pure union, deterministic and commutative either way).
        for (vc, val) in &other.values {
            if !self.values.iter().any(|(existing_vc, _)| existing_vc == vc) {
                self.values.push((vc.clone(), val.clone()));
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
