//! RGA: Replicated Growable Array.
//! Source: Team Prompt 3 §3.6. A sequence CRDT with concurrent insert
//! support. Uses a linked-list of atoms with unique IDs.
//!
//! DECISIONS.md B7: `insert_after` no longer calls a nonexistent
//! `self.get_local_replica_id()`; the caller supplies `local_replica`
//! explicitly (Rga holds no replica identity of its own — nothing in the
//! frozen struct shape gives it one). `ElementId::root()` is defined as the
//! all-zero sentinel parent for the first inserted atom.

use platform_kernel::{ReplicaId, VectorClock};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt::Debug};

use crate::Crdt;

/// A sequence CRDT (Replicated Growable Array) supporting concurrent
/// insert/delete at any position, converging to the same ordered sequence
/// on every replica.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "E: Serialize", deserialize = "E: DeserializeOwned"))]
pub struct Rga<E: Clone + Debug + Serialize + DeserializeOwned> {
    atoms: Vec<Atom<E>>,
    clock: VectorClock,
    /// For GC tracking.
    summary_clock: VectorClock,
}

/// A single element in an `Rga` sequence, linked to its predecessor.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "E: Serialize",
    deserialize = "E: serde::de::DeserializeOwned"
))]
pub struct Atom<E> {
    /// This atom's unique, causally-ordered identifier.
    pub id: ElementId,
    /// The identifier of the atom immediately before this one.
    pub parent: ElementId,
    /// `None` means tombstone (deleted).
    pub value: Option<E>,
    /// Whether this atom has been deleted (tombstoned).
    pub is_tombstone: bool,
    /// When this atom was inserted/deleted.
    pub clock: VectorClock,
}

/// A globally unique, causally-ordered identifier for one `Atom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ElementId {
    /// The replica that created this element.
    pub replica: ReplicaId,
    /// Lamport timestamp (causal order).
    pub lamport: u64,
}

impl ElementId {
    /// The all-zero sentinel parent used as the "before the first element"
    /// marker. Never assigned to a real inserted atom.
    ///
    /// Constructed directly via `ReplicaId`'s public `[u8; 16]` field
    /// (`platform_kernel::ReplicaId` has no `zero`/sentinel constructor of
    /// its own — Team 1's real kernel crate is not ours to extend, per
    /// Architectural Ruling Q1, so this stays local to `crdt`).
    pub fn root() -> Self {
        Self {
            replica: ReplicaId([0u8; 16]),
            lamport: 0,
        }
    }
}

impl<E: Clone + Debug + Send + Sync + Serialize + DeserializeOwned> Rga<E> {
    /// Create an empty sequence.
    pub fn new() -> Self {
        Self {
            atoms: Vec::new(),
            clock: VectorClock::new(),
            summary_clock: VectorClock::new(),
        }
    }

    /// Insert a value after the given parent element.
    ///
    /// DECISIONS.md B7: `local_replica` is now an explicit parameter (the
    /// frozen signature's implicit `self.get_local_replica_id()` call had no
    /// backing field to read from).
    ///
    /// The local replica's Lamport timestamp is folded in *after* absorbing
    /// the caller's causal context and *then* incremented, which is what
    /// keeps `ElementId`s unique. Reading the counter before folding in
    /// `clock` (and never writing the increment back) let two consecutive
    /// inserts from one replica both mint `lamport: 1` whenever the caller
    /// passed a clock that didn't already mention that replica — colliding
    /// ids that then silently dropped atoms during traversal and made
    /// merge order observable in `to_vec()`, i.e. a non-convergent CRDT.
    pub fn insert_after(
        &mut self,
        parent: ElementId,
        value: E,
        clock: VectorClock,
        local_replica: ReplicaId,
    ) -> ElementId {
        self.clock = self.clock.merge(&clock);
        self.clock.increment(local_replica);
        let id = ElementId {
            replica: local_replica,
            lamport: self.clock.get(&local_replica),
        };
        self.atoms.push(Atom {
            id,
            parent,
            value: Some(value),
            is_tombstone: false,
            clock: clock.clone(),
        });
        self.atoms.sort_unstable_by_key(|atom| atom.id);
        id
    }

    /// Delete an element (tombstone).
    pub fn delete(&mut self, id: ElementId, clock: VectorClock) {
        for atom in &mut self.atoms {
            if atom.id == id && !atom.is_tombstone {
                atom.is_tombstone = true;
                atom.value = None;
                atom.clock = atom.clock.merge(&clock);
                break;
            }
        }
        self.clock = self.clock.merge(&clock);
    }

    /// Direct children of `parent`, ascending by [`ElementId`].
    fn children_of(&self, parent: ElementId) -> Vec<ElementId> {
        let mut children: Vec<ElementId> = self
            .atoms
            .iter()
            .filter(|atom| atom.parent == parent)
            .map(|atom| atom.id)
            .collect();
        children.sort_unstable();
        children
    }

    /// Get the ordered sequence of values (skipping tombstones).
    ///
    /// RGA siblings are ordered deterministically by ElementId, never by
    /// the local Vec insertion order, and each atom is emitted immediately
    /// before its own subtree is traversed: a strict pre-order depth-first
    /// walk, so a value inserted after an earlier sibling still precedes
    /// that sibling's descendants on every replica.
    ///
    /// Both properties are load-bearing for convergence. Collecting a
    /// parent's children and pushing their values in one pass while the
    /// traversal continues from the ids emitted siblings in *descending*
    /// ElementId order and interleaved whole branches with descendants —
    /// making the result depend on the order replicas happened to merge in,
    /// which is exactly what a CRDT must not do.
    pub fn to_vec(&self) -> Vec<&E> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        // The stack is LIFO, so push in reverse (descending) order to visit
        // the smallest ElementId of each sibling group first.
        for child in self.children_of(ElementId::root()).into_iter().rev() {
            stack.push(child);
        }

        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            let Some(atom) = self.atoms.iter().find(|atom| atom.id == id) else {
                continue;
            };
            // Tombstones carry no value but are still traversed, so their
            // descendants keep their place in the sequence.
            if let Some(value) = atom.value.as_ref() {
                result.push(value);
            }
            for child in self.children_of(id).into_iter().rev() {
                stack.push(child);
            }
        }

        result
    }

    /// The causal context for GC.
    pub fn causal_context(&self) -> &VectorClock {
        &self.clock
    }

    /// The componentwise-merged summary clock, used for GC eligibility
    /// tracking distinct from the main causal context.
    pub fn summary_clock(&self) -> &VectorClock {
        &self.summary_clock
    }
}

impl<E: Clone + Debug + Send + Sync + Serialize + DeserializeOwned> Default for Rga<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Clone + Debug + Send + Sync + Serialize + DeserializeOwned> Crdt for Rga<E> {
    fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;

        for atom in &other.atoms {
            if !self.atoms.iter().any(|a| a.id == atom.id) {
                self.atoms.push(atom.clone());
                changed = true;
            } else if let Some(existing) = self.atoms.iter_mut().find(|a| a.id == atom.id) {
                if atom.is_tombstone && !existing.is_tombstone {
                    existing.is_tombstone = true;
                    existing.value = None;
                    existing.clock = existing.clock.merge(&atom.clock);
                    changed = true;
                }
            }
        }

        let new_clock = self.clock.merge(&other.clock);
        if self.clock != new_clock {
            self.clock = new_clock;
            changed = true;
        }

        let new_summary = self.summary_clock.merge(&other.summary_clock);
        if self.summary_clock != new_summary {
            self.summary_clock = new_summary;
            changed = true;
        }
        self.atoms.sort_unstable_by_key(|atom| atom.id);

        changed
    }

    fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replica(byte: u8) -> ReplicaId {
        ReplicaId([byte; 16])
    }

    /// `Rga<E>` requires `E: DeserializeOwned`, so tests must use an owned
    /// element type. `&'static str` literals would infer `E = &str`, which
    /// violates that bound (and makes rustc report it as an opaque
    /// "Deserialize is not general enough" error rather than a bound
    /// violation), so the sequences under test carry `String` elements and
    /// this helper projects them back to `&str` for readable assertions.
    fn texts(sequence: &Rga<String>) -> Vec<&str> {
        sequence.to_vec().into_iter().map(String::as_str).collect()
    }

    #[test]
    fn concurrent_siblings_converge_independently_of_merge_order() {
        let local = replica(1);
        let remote = replica(2);
        let root = ElementId::root();

        let mut left: Rga<String> = Rga::new();
        let left_id = left.insert_after(root, "left".to_string(), VectorClock::new(), local);

        let mut right: Rga<String> = Rga::new();
        let right_id = right.insert_after(root, "right".to_string(), VectorClock::new(), remote);

        assert_ne!(left_id, right_id);

        let mut left_then_right = left.clone();
        left_then_right.merge(&right);

        let mut right_then_left = right.clone();
        right_then_left.merge(&left);

        assert_eq!(texts(&left_then_right), texts(&right_then_left));
        assert_eq!(texts(&left_then_right), vec!["left", "right"]);
    }

    #[test]
    fn descendants_of_sibling_branches_are_not_dropped() {
        let first = replica(1);
        let second = replica(2);
        let root = ElementId::root();

        let mut a: Rga<String> = Rga::new();
        let a_id = a.insert_after(root, "a".to_string(), VectorClock::new(), first);
        a.insert_after(a_id, "a-child".to_string(), VectorClock::new(), first);

        let mut b: Rga<String> = Rga::new();
        b.insert_after(root, "b".to_string(), VectorClock::new(), second);

        let mut merged = a.clone();
        merged.merge(&b);

        assert_eq!(texts(&merged), vec!["a", "a-child", "b"]);
    }
}
