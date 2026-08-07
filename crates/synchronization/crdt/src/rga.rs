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
use std::fmt::Debug;

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
    pub fn insert_after(
        &mut self,
        parent: ElementId,
        value: E,
        clock: VectorClock,
        local_replica: ReplicaId,
    ) -> ElementId {
        let id = ElementId {
            replica: local_replica,
            lamport: self.clock.get(&local_replica) + 1,
        };
        self.atoms.push(Atom {
            id,
            parent,
            value: Some(value),
            is_tombstone: false,
            clock: clock.clone(),
        });
        self.clock = self.clock.merge(&clock);
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

    /// Get the ordered sequence of values (skipping tombstones).
    pub fn to_vec(&self) -> Vec<&E> {
        let mut result = Vec::new();
        let mut current = None;
        // Find the root (parent = ElementId::root())
        for atom in &self.atoms {
            if atom.parent == ElementId::root() {
                current = Some(atom.id);
                break;
            }
        }
        while let Some(id) = current {
            let mut advanced = false;
            for atom in &self.atoms {
                if atom.id == id {
                    if let Some(ref val) = atom.value {
                        result.push(val);
                    }
                    current = self.find_next_after(id);
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                break;
            }
        }
        result
    }

    /// Internal: find the atom immediately after the given ID.
    fn find_next_after(&self, id: ElementId) -> Option<ElementId> {
        for atom in &self.atoms {
            if atom.parent == id {
                return Some(atom.id);
            }
        }
        None
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

        changed
    }

    fn causal_context(&self) -> &VectorClock {
        &self.clock
    }
}
