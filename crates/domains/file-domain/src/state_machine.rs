//! State machines for the File bounded context.

use serde::{Deserialize, Serialize};

/// A FileAsset's lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileAssetStatus {
    /// Normal state; versions may be added, access may be granted/revoked.
    Active,
    /// Held pending a malware-scan or policy review (Part I §4.8.8,
    /// "Security service: quarantine based on scan results"). Blocks new
    /// versions and access grants; existing access is not itself revoked
    /// by quarantine alone — that is a separate, explicit command.
    Quarantined,
    /// Archived. Terminal in this crate's scope — restore workflows are a
    /// Policy/retention concern (Part I §4.18), not modeled here.
    Archived,
}

/// An UploadSession's lifecycle status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UploadSessionStatus {
    /// Accepting chunks.
    InProgress,
    /// All declared bytes received and finalized. Terminal — a finished
    /// upload does not resume; a new upload starts a new session.
    Finalized,
}
