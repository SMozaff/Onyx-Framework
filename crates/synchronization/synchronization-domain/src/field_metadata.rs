//! FieldMetadata and FieldShape.
//! Source: Team Prompt 3 §3.10, transcribed verbatim.

use serde::{Deserialize, Serialize};

/// Describes how a single aggregate field should be reconciled: its CRDT
/// shape (if any) and whether it's authority-controlled.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldMetadata {
    /// The field's name/path on its aggregate.
    pub name: String,
    /// Which CRDT shape (if any) this field uses for merging.
    pub shape: FieldShape,
    /// If true, concurrent writes must never be auto-merged — they always
    /// produce a `ConflictRecord` for human review.
    pub is_authority_controlled: bool,
    /// If true, this field has a CRDT-eligible shape and can be dispatched
    /// through `MergeStrategy`.
    pub is_crdt_eligible: bool,
}

/// The CRDT taxonomy shape a field is stored as, used to dispatch
/// `MergeStrategy::merge_field`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldShape {
    /// OR-Set
    ToggleSet,
    /// LWW-Register
    LwwSingle,
    /// MV-Register
    MvSingle,
    /// PN-Counter
    Counter,
    /// RGA
    OrderedSequence,
    /// Append-only causal log
    AppendOnlyLog,
    /// Bypass merge entirely; create ConflictRecord
    AuthorityControlled,
}

impl FieldMetadata {
    /// True if this field can be auto-merged (CRDT-eligible and not
    /// authority-controlled).
    pub fn is_mergeable(&self) -> bool {
        self.is_crdt_eligible && !self.is_authority_controlled
    }
}
