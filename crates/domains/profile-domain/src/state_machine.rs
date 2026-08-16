//! State machine for the StaffProfile aggregate.

use serde::{Deserialize, Serialize};

/// A StaffProfile's lifecycle status. Simple by design — a profile
/// exists once created and is either `Active` or `Archived` (e.g. the
/// owning user was deactivated/removed); no draft/review workflow was
/// requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileStatus {
    /// Normal state; editable per this crate's authority checks.
    Active,
    /// Terminal. An archived profile's historical field values remain
    /// readable (event-sourced, same as every other aggregate in this
    /// workspace), but no further edits are accepted.
    Archived,
}
