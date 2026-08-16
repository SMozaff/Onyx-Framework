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

/// A ConnectionRequest's lifecycle status. New in Phase 1 — see
/// `value::ConnectionRequestId`'s doc comment for the aggregate's
/// purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionRequestStatus {
    /// Awaiting the recipient's decision.
    Pending,
    /// Accepted; the two users now have a direct connection.
    /// Terminal — a further request between the same two users, if ever
    /// wanted again after some other change severs the connection, is a
    /// new `ConnectionRequest`, not a reopening of this one, keeping the
    /// history of who connected with whom and when intact (same
    /// rationale as `LegalHoldStatus::Released` in `policy-domain`).
    Accepted,
    /// Declined by the recipient. Terminal.
    Declined,
    /// Withdrawn by the sender before the recipient decided. Terminal.
    Revoked,
}
