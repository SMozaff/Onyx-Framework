//! Domain events emitted by the Policy and LegalHold aggregates.
//! Source: Part I §4.18.5.

use crate::value::{HoldTarget, PolicyId, PolicyRule, PolicyScope};
use platform_kernel::{Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// Events emitted by the Policy aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PolicyEvent {
    /// A new policy container was created.
    PolicyCreated {
        /// The new policy's identity.
        policy_id: PolicyId,
        /// Its name.
        name: String,
        /// What it governs.
        scope: PolicyScope,
        /// Who created it.
        created_by: UserId,
        /// When.
        created_at: Timestamp,
    },
    /// A new draft version was created.
    PolicyVersionCreated {
        /// This version's sequence number within the policy (1-based;
        /// the first version is `1`).
        version_number: u32,
        /// The rules this draft proposes.
        rules: Vec<PolicyRule>,
        /// When.
        created_at: Timestamp,
    },
    /// The most recent draft version was published, becoming current.
    PolicyVersionPublished {
        /// Which version number was published.
        version_number: u32,
        /// Who published it.
        published_by: UserId,
        /// When.
        published_at: Timestamp,
    },
    /// A policy rule was evaluated.
    PolicyEvaluated {
        /// Which rule key was evaluated.
        rule_key: String,
        /// The evaluated value, if the key exists in the current
        /// published version (`None` if the key is absent — evaluation
        /// still succeeds and is still recorded; absence is not itself
        /// an error, per §4.18.4's determinism invariant, which
        /// requires a defined outcome for every input, not just every
        /// key that happens to exist).
        result: Option<serde_json::Value>,
        /// Which published version this evaluation was against.
        evaluated_version: u32,
        /// When.
        evaluated_at: Timestamp,
    },
    /// A policy violation was recorded.
    PolicyViolationDetected {
        /// Which rule was violated.
        rule_key: String,
        /// Why/what happened.
        reason: String,
        /// When.
        detected_at: Timestamp,
    },
    /// The policy was retired.
    PolicyRetired {
        /// When.
        retired_at: Timestamp,
    },
}

/// Events emitted by the LegalHold aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LegalHoldEvent {
    /// A legal hold was applied.
    LegalHoldApplied {
        /// The new hold's identity.
        legal_hold_id: crate::value::LegalHoldId,
        /// What's held.
        target: HoldTarget,
        /// The authorizing policy, if any.
        authorizing_policy: Option<PolicyId>,
        /// Why.
        reason: String,
        /// Who applied it.
        applied_by: UserId,
        /// When.
        applied_at: Timestamp,
    },
    /// A legal hold was released.
    LegalHoldReleased {
        /// Why.
        reason: String,
        /// Who released it.
        released_by: UserId,
        /// When.
        released_at: Timestamp,
    },
}
