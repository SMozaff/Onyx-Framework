//! Commands accepted by the Policy and LegalHold aggregates.
//! Source: Part I §4.18.5.

use crate::value::{HoldTarget, PolicyId, PolicyRule, PolicyScope};
use serde::{Deserialize, Serialize};

/// Commands accepted by the Policy aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PolicyCommand {
    /// Create a new policy container. Routed through `Policy::create()`,
    /// not `decide()` — mirrors every other domain's established
    /// creation-command pattern (Increment 1 ruling C).
    CreatePolicy {
        /// A short, human-readable name (e.g. `"Default Org Governance"`).
        name: String,
        /// What this policy governs.
        scope: PolicyScope,
    },
    /// Draft a new version under this policy. The new version starts in
    /// `Draft` and is not yet in effect until `PublishPolicyVersion`.
    CreatePolicyVersion {
        /// The rules this draft version proposes.
        rules: Vec<PolicyRule>,
    },
    /// Publish the most recent draft version, making it the policy's
    /// current, immutable, evaluatable version (§4.18.4 invariant 108b).
    PublishPolicyVersion,
    /// Evaluate the policy's current published version against a named
    /// rule key. Read-oriented but modeled as a command (not a query)
    /// per §4.18.5's own command list, since evaluation is itself a
    /// recorded domain event (`PolicyEvaluated`) for audit purposes —
    /// unlike a plain projection read.
    EvaluatePolicy {
        /// Which rule key to evaluate (e.g. `"messaging.enabled"`).
        rule_key: String,
    },
    /// Record that a policy rule was violated by some other domain's
    /// operation. `EvaluatePolicy`'s companion for the "governed
    /// operation was rejected" outcome, not a query result.
    RegisterViolation {
        /// Which rule was violated.
        rule_key: String,
        /// Why/what happened.
        reason: String,
    },
    /// Retire the policy. Terminal.
    RetirePolicy,
}

/// Commands accepted by the LegalHold aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LegalHoldCommand {
    /// Apply a legal hold to a target object. Routed through
    /// `LegalHold::create()`, not `decide()`.
    ApplyLegalHold {
        /// What's being held.
        target: HoldTarget,
        /// The policy this hold is authorized under, if any (a hold may
        /// also be applied ad hoc by a Compliance Officer without a
        /// pre-published policy rule driving it — §4.18.8).
        authorizing_policy: Option<PolicyId>,
        /// Why.
        reason: String,
    },
    /// Release a previously applied hold.
    ReleaseLegalHold {
        /// Why.
        reason: String,
    },
}
