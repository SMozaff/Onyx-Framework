//! State machines for the Communication bounded context.

use serde::{Deserialize, Serialize};

/// A Conversation's lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversationStatus {
    /// Open; members may post and be added.
    Active,
    /// Closed to new activity. Per §4.7.4, archival does not erase
    /// history — messages remain readable, just no longer postable-to.
    Archived,
}

/// A Message's lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    /// The message as originally posted, unmodified.
    Posted,
    /// Edited at least once. Per §4.7.4, edits create revisions rather
    /// than overwriting — this status marks that a revision exists, the
    /// revision content itself lives in the event stream, not the status.
    Edited,
    /// Redacted per policy-governed takedown (§4.7.4: "Deletion is
    /// policy-governed redaction or tombstone, not silent erasure").
    /// Terminal: a redacted message cannot be edited or un-redacted.
    Redacted,
}
