//! State machines for the Policy bounded context.

use serde::{Deserialize, Serialize};

/// A Policy's lifecycle status. Distinct from `PolicyVersion` status
/// below — a `Policy` is the durable container; its versions are the
/// things that get drafted, published, and superseded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyStatus {
    /// Normal state; new versions may be drafted and published.
    Active,
    /// Retired. Terminal — its last-published version remains valid for
    /// historical evaluation/audit (§4.18.4 "policy changes never
    /// rewrite historical decisions"), but no new version may be
    /// created or published against it.
    Retired,
}

/// A single `PolicyVersion`'s lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyVersionStatus {
    /// Being authored; mergeable/editable per §4.18.7 ("Draft rules —
    /// collaborative editing may be mergeable outside published state").
    Draft,
    /// Published. Terminal and immutable (§4.18.4 invariant 108b,
    /// "published policy versions are immutable") — a correction is a
    /// new `PolicyVersion`, never a mutation of this one.
    Published,
}

/// A LegalHold's lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegalHoldStatus {
    /// In effect; supersedes normal retention/deletion for its target
    /// (§4.18.4 invariant 109).
    Applied,
    /// No longer in effect. Terminal in this crate's scope — a new hold
    /// on the same target is a new `LegalHold`, not a re-applied one,
    /// keeping the history of who held what and when intact.
    Released,
}
