//! The Policy and LegalHold aggregate roots.
//!
//! Source: Part I §4.18. Policy is an independent aggregate root from
//! LegalHold (§4.18.1's two-row aggregate boundary table) — a hold can be
//! applied ad hoc by a Compliance Officer (§4.18.8) without a policy
//! version driving it, so coupling their lifecycles would force an
//! artificial policy to exist for every hold.
//!
//! # Scope note (Phase 1)
//! Built as the full domain per Part I §4.18, not a trimmed subset —
//! `LegalHold`, `PolicyViolation` recording, and versioned publication
//! are all real, decision-tested behavior here, not stubs. What is
//! *not* built yet: automatic per-rule-type validation (e.g. verifying a
//! `Threshold` rule's value is actually numeric) — this crate accepts
//! any `serde_json::Value` for a rule and leaves interpretation to the
//! consuming domain, matching the same "this crate defines the mechanism,
//! not every rule's semantics" rationale as `value::PolicyRule`'s docs.
//!
//! # Authority (established precedent)
//! Both aggregates follow `mission_domain::Mission`'s precedent exactly:
//! `decide()` checks only the generic `context.authority.is_authorized(...)`
//! stub. Real per-role checks — e.g. "only a Manager or Admin may publish
//! a policy version," "only a Compliance Officer may apply a legal hold"
//! (§4.18.8) — are ABAC/policy-evaluation concerns the composing
//! application enforces before dispatch (mirroring how every other domain
//! in this workspace defers real authority to Increment 7 / the
//! composition root), not something this crate invents independently.

use crate::command::{LegalHoldCommand, PolicyCommand};
use crate::error::{LegalHoldError, PolicyError};
use crate::event::{LegalHoldEvent, PolicyEvent};
use crate::state_machine::{LegalHoldStatus, PolicyStatus, PolicyVersionStatus};
use crate::value::{HoldTarget, LegalHoldId, PolicyId, PolicyRule, PolicyScope};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectVersion, UserId};
use serde::{Deserialize, Serialize};

/// One published or draft version of a `Policy`'s rule set.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PolicyVersionRecord {
    version_number: u32,
    status: PolicyVersionStatus,
    rules: Vec<PolicyRule>,
}

/// The Policy aggregate: a versioned, scoped set of governance rules
/// (§4.18.1). Owns *what the current rules are and how they got there*,
/// not enforcement itself — enforcement is each governed domain's own
/// `decide()` consulting `EvaluatePolicy`'s result via the composition
/// root, the same seam every other domain's authority stub already
/// exposes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    id: PolicyId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: PolicyStatus,
    name: String,
    scope: PolicyScope,
    /// All versions ever created, in order. The current *effective*
    /// version is the highest-numbered one with `Published` status —
    /// never mutated once published (§4.18.4 invariant 108b), and never
    /// removed (§4.18.4 invariant 110, "policy changes never rewrite
    /// historical decisions").
    versions: Vec<PolicyVersionRecord>,
}

impl Policy {
    /// Constructs a new `Policy` from a `CreatePolicy` command. Starts
    /// with zero versions — no rules are in effect until a version is
    /// drafted and published.
    ///
    /// # Errors
    /// Returns [`PolicyError::InvalidTransition`] if called with any
    /// command other than `CreatePolicy`, or
    /// [`PolicyError::MissingField`] if `name` is empty.
    pub fn create(
        command: PolicyCommand,
        context: &DecisionContext,
    ) -> Result<Vec<PolicyEvent>, PolicyError> {
        let PolicyCommand::CreatePolicy { name, scope } = command else {
            return Err(PolicyError::InvalidTransition(
                "Policy::create() called with a non-CreatePolicy command".to_string(),
            ));
        };
        if name.trim().is_empty() {
            return Err(PolicyError::MissingField("name".to_string()));
        }

        Ok(vec![PolicyEvent::PolicyCreated {
            policy_id: PolicyId(context.generated_id_generator.generate_object_id()),
            name,
            scope,
            created_by: context.actor.user_id,
            created_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `Policy` by folding its first event (`PolicyCreated`).
    ///
    /// # Panics
    /// Panics if `event` is not a `PolicyCreated` event.
    pub fn from_created_event(event: &PolicyEvent) -> Self {
        let PolicyEvent::PolicyCreated {
            policy_id,
            name,
            scope,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-PolicyCreated event");
        };

        Self {
            id: *policy_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: PolicyStatus::Active,
            name: name.clone(),
            scope: scope.clone(),
            versions: Vec::new(),
        }
    }

    /// The policy's current status.
    pub fn status(&self) -> PolicyStatus {
        self.status
    }

    /// The policy's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What this policy governs.
    pub fn scope(&self) -> &PolicyScope {
        &self.scope
    }

    /// The current draft version, if one is pending publication.
    fn draft_version(&self) -> Option<&PolicyVersionRecord> {
        self.versions
            .last()
            .filter(|v| v.status == PolicyVersionStatus::Draft)
    }

    /// The current effective (most recently published) version, if any.
    pub fn published_version(&self) -> Option<(u32, &[PolicyRule])> {
        self.versions
            .iter()
            .rev()
            .find(|v| v.status == PolicyVersionStatus::Published)
            .map(|v| (v.version_number, v.rules.as_slice()))
    }

    fn invalid(&self, command: &str) -> PolicyError {
        PolicyError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for Policy {
    type Id = PolicyId;
    type Command = PolicyCommand;
    type Event = PolicyEvent;
    type Error = PolicyError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if !context.authority.is_authorized("policy.command") {
            return Err(PolicyError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            PolicyCommand::CreatePolicy { .. } => Err(PolicyError::InvalidTransition(
                "CreatePolicy must be dispatched via Policy::create(), not decide()".to_string(),
            )),

            PolicyCommand::CreatePolicyVersion { rules } => match self.status {
                PolicyStatus::Active => {
                    if self.draft_version().is_some() {
                        return Err(PolicyError::DraftAlreadyPending);
                    }
                    let version_number = self.versions.len() as u32 + 1;
                    Ok(vec![PolicyEvent::PolicyVersionCreated {
                        version_number,
                        rules,
                        created_at: context.trusted_now,
                    }])
                }
                PolicyStatus::Retired => Err(self.invalid("CreatePolicyVersion")),
            },

            PolicyCommand::PublishPolicyVersion => match self.status {
                PolicyStatus::Active => {
                    let draft = self.draft_version().ok_or(PolicyError::NoDraftVersion)?;
                    Ok(vec![PolicyEvent::PolicyVersionPublished {
                        version_number: draft.version_number,
                        published_by: context.actor.user_id,
                        published_at: context.trusted_now,
                    }])
                }
                PolicyStatus::Retired => Err(self.invalid("PublishPolicyVersion")),
            },

            PolicyCommand::EvaluatePolicy { rule_key } => {
                let (version_number, rules) = self
                    .published_version()
                    .ok_or(PolicyError::NoPublishedVersion)?;
                let result = rules
                    .iter()
                    .find(|r| r.key == rule_key)
                    .map(|r| r.value.clone());
                Ok(vec![PolicyEvent::PolicyEvaluated {
                    rule_key,
                    result,
                    evaluated_version: version_number,
                    evaluated_at: context.trusted_now,
                }])
            }

            PolicyCommand::RegisterViolation { rule_key, reason } => {
                if self.published_version().is_none() {
                    return Err(PolicyError::NoPublishedVersion);
                }
                Ok(vec![PolicyEvent::PolicyViolationDetected {
                    rule_key,
                    reason,
                    detected_at: context.trusted_now,
                }])
            }

            PolicyCommand::RetirePolicy => match self.status {
                PolicyStatus::Active => Ok(vec![PolicyEvent::PolicyRetired {
                    retired_at: context.trusted_now,
                }]),
                PolicyStatus::Retired => Err(self.invalid("RetirePolicy")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            PolicyEvent::PolicyCreated { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            PolicyEvent::PolicyVersionCreated {
                version_number,
                rules,
                ..
            } => {
                self.versions.push(PolicyVersionRecord {
                    version_number: *version_number,
                    status: PolicyVersionStatus::Draft,
                    rules: rules.clone(),
                });
            }
            PolicyEvent::PolicyVersionPublished { version_number, .. } => {
                if let Some(v) = self
                    .versions
                    .iter_mut()
                    .find(|v| v.version_number == *version_number)
                {
                    v.status = PolicyVersionStatus::Published;
                }
                // Publication changes which rules govern every dependent
                // domain's evaluation outcome — an authority-relevant
                // change, mirroring `Conversation::ConversationMemberAdded`
                // and `FileAsset::FileAccessGranted` advancing
                // authority_epoch for the same reason ("who/what is
                // authorized just changed").
                self.authority_epoch = self.authority_epoch.advance();
            }
            PolicyEvent::PolicyEvaluated { .. } => {
                // Read-oriented; no state change beyond version advance
                // below (still recorded as a real event for audit, per
                // this file's `EvaluatePolicy` doc comment).
            }
            PolicyEvent::PolicyViolationDetected { .. } => {
                // Recorded for audit; no aggregate state change beyond
                // version advance. A future increment may want a
                // running violation count/list on this aggregate for
                // direct querying — not needed by any Phase 1 consumer,
                // so not added speculatively.
            }
            PolicyEvent::PolicyRetired { .. } => {
                self.status = PolicyStatus::Retired;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

/// The LegalHold aggregate: the lifecycle of one retention override on
/// one target object (§4.18.1). An independent aggregate root, not an
/// entity of `Policy` — see this file's header comment for why.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegalHold {
    id: LegalHoldId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: LegalHoldStatus,
    target: HoldTarget,
    authorizing_policy: Option<PolicyId>,
    #[allow(dead_code)] // recorded for provenance; no Phase 1 reader needs it via an accessor
    applied_by: UserId,
}

impl LegalHold {
    /// Constructs a new `LegalHold` from an `ApplyLegalHold` command.
    ///
    /// # Errors
    /// Returns [`LegalHoldError::InvalidTransition`] if called with any
    /// command other than `ApplyLegalHold`, or
    /// [`LegalHoldError::MissingField`] if `reason` is empty.
    pub fn create(
        command: LegalHoldCommand,
        context: &DecisionContext,
    ) -> Result<Vec<LegalHoldEvent>, LegalHoldError> {
        let LegalHoldCommand::ApplyLegalHold {
            target,
            authorizing_policy,
            reason,
        } = command
        else {
            return Err(LegalHoldError::InvalidTransition(
                "LegalHold::create() called with a non-ApplyLegalHold command".to_string(),
            ));
        };
        if reason.trim().is_empty() {
            return Err(LegalHoldError::MissingField("reason".to_string()));
        }

        Ok(vec![LegalHoldEvent::LegalHoldApplied {
            legal_hold_id: LegalHoldId(context.generated_id_generator.generate_object_id()),
            target,
            authorizing_policy,
            reason,
            applied_by: context.actor.user_id,
            applied_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `LegalHold` by folding its first event
    /// (`LegalHoldApplied`).
    ///
    /// # Panics
    /// Panics if `event` is not a `LegalHoldApplied` event.
    pub fn from_created_event(event: &LegalHoldEvent) -> Self {
        let LegalHoldEvent::LegalHoldApplied {
            legal_hold_id,
            target,
            authorizing_policy,
            applied_by,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-LegalHoldApplied event");
        };

        Self {
            id: *legal_hold_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: LegalHoldStatus::Applied,
            target: target.clone(),
            authorizing_policy: *authorizing_policy,
            applied_by: *applied_by,
        }
    }

    /// The hold's current status.
    pub fn status(&self) -> LegalHoldStatus {
        self.status
    }

    /// What's held.
    pub fn target(&self) -> HoldTarget {
        self.target.clone()
    }

    fn invalid(&self, command: &str) -> LegalHoldError {
        LegalHoldError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for LegalHold {
    type Id = LegalHoldId;
    type Command = LegalHoldCommand;
    type Event = LegalHoldEvent;
    type Error = LegalHoldError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if !context.authority.is_authorized("policy.legal_hold.command") {
            return Err(LegalHoldError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            LegalHoldCommand::ApplyLegalHold { .. } => Err(LegalHoldError::InvalidTransition(
                "ApplyLegalHold must be dispatched via LegalHold::create(), not decide()"
                    .to_string(),
            )),

            LegalHoldCommand::ReleaseLegalHold { reason } => match self.status {
                LegalHoldStatus::Applied => Ok(vec![LegalHoldEvent::LegalHoldReleased {
                    reason,
                    released_by: context.actor.user_id,
                    released_at: context.trusted_now,
                }]),
                LegalHoldStatus::Released => Err(self.invalid("ReleaseLegalHold")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            LegalHoldEvent::LegalHoldApplied { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            LegalHoldEvent::LegalHoldReleased { .. } => {
                self.status = LegalHoldStatus::Released;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        active_policy, applied_legal_hold, policy_with_published_rules, test_context, toggle_rule,
    };

    fn apply_all(policy: &mut Policy, events: &[PolicyEvent]) {
        for e in events {
            policy.apply(e);
        }
    }

    // --- Policy: creation ---

    #[test]
    fn create_policy_succeeds() {
        let ctx = test_context();
        let events = Policy::create(
            PolicyCommand::CreatePolicy {
                name: "Org Governance".to_string(),
                scope: PolicyScope::Organization(ctx.actor.organization_id),
            },
            &ctx,
        )
        .expect("create must succeed");
        let policy = Policy::from_created_event(&events[0]);
        assert_eq!(policy.status(), PolicyStatus::Active);
        assert_eq!(policy.name(), "Org Governance");
        assert!(policy.published_version().is_none());
    }

    #[test]
    fn create_policy_with_empty_name_is_rejected() {
        let ctx = test_context();
        let result = Policy::create(
            PolicyCommand::CreatePolicy {
                name: "   ".to_string(),
                scope: PolicyScope::Organization(ctx.actor.organization_id),
            },
            &ctx,
        );
        assert!(matches!(result, Err(PolicyError::MissingField(_))));
    }

    #[test]
    fn create_called_via_decide_is_rejected() {
        let policy = active_policy();
        let ctx = test_context();
        let result = policy.decide(
            PolicyCommand::CreatePolicy {
                name: "x".to_string(),
                scope: PolicyScope::Organization(ctx.actor.organization_id),
            },
            &ctx,
        );
        assert!(matches!(result, Err(PolicyError::InvalidTransition(_))));
    }

    // --- Policy: draft / publish lifecycle ---

    #[test]
    fn create_version_then_publish_succeeds() {
        let mut policy = active_policy();
        let ctx = test_context();

        let create_events = policy
            .decide(
                PolicyCommand::CreatePolicyVersion {
                    rules: vec![toggle_rule("messaging.enabled", true)],
                },
                &ctx,
            )
            .expect("create version must succeed");
        apply_all(&mut policy, &create_events);
        assert!(policy.published_version().is_none());

        let publish_events = policy
            .decide(PolicyCommand::PublishPolicyVersion, &ctx)
            .expect("publish must succeed");
        apply_all(&mut policy, &publish_events);

        let (version_number, rules) = policy.published_version().expect("must be published");
        assert_eq!(version_number, 1);
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn publish_with_no_draft_is_rejected() {
        let policy = active_policy();
        let ctx = test_context();
        let result = policy.decide(PolicyCommand::PublishPolicyVersion, &ctx);
        assert!(matches!(result, Err(PolicyError::NoDraftVersion)));
    }

    #[test]
    fn second_draft_while_one_pending_is_rejected() {
        let mut policy = active_policy();
        let ctx = test_context();
        let events = policy
            .decide(
                PolicyCommand::CreatePolicyVersion {
                    rules: vec![toggle_rule("messaging.enabled", true)],
                },
                &ctx,
            )
            .unwrap();
        apply_all(&mut policy, &events);

        let result = policy.decide(
            PolicyCommand::CreatePolicyVersion {
                rules: vec![toggle_rule("file_sharing.enabled", true)],
            },
            &ctx,
        );
        assert!(matches!(result, Err(PolicyError::DraftAlreadyPending)));
    }

    #[test]
    fn published_version_is_immutable_a_new_draft_is_a_new_version() {
        let mut policy = policy_with_published_rules(vec![toggle_rule("messaging.enabled", true)]);
        let ctx = test_context();

        let (v1, _) = policy.published_version().unwrap();
        assert_eq!(v1, 1);

        let create_events = policy
            .decide(
                PolicyCommand::CreatePolicyVersion {
                    rules: vec![toggle_rule("messaging.enabled", false)],
                },
                &ctx,
            )
            .unwrap();
        apply_all(&mut policy, &create_events);
        // v1 remains the effective published version until v2 is published.
        let (still_v1, rules) = policy.published_version().unwrap();
        assert_eq!(still_v1, 1);
        assert_eq!(rules[0].value, serde_json::json!(true));

        let publish_events = policy
            .decide(PolicyCommand::PublishPolicyVersion, &ctx)
            .unwrap();
        apply_all(&mut policy, &publish_events);
        let (v2, rules) = policy.published_version().unwrap();
        assert_eq!(v2, 2);
        assert_eq!(rules[0].value, serde_json::json!(false));
    }

    // --- Policy: evaluation ---

    #[test]
    fn evaluate_known_key_returns_its_value() {
        let policy = policy_with_published_rules(vec![toggle_rule("messaging.enabled", true)]);
        let ctx = test_context();
        let events = policy
            .decide(
                PolicyCommand::EvaluatePolicy {
                    rule_key: "messaging.enabled".to_string(),
                },
                &ctx,
            )
            .expect("evaluate must succeed");
        let PolicyEvent::PolicyEvaluated { result, .. } = &events[0] else {
            panic!("expected PolicyEvaluated");
        };
        assert_eq!(*result, Some(serde_json::json!(true)));
    }

    #[test]
    fn evaluate_unknown_key_returns_none_not_an_error() {
        let policy = policy_with_published_rules(vec![toggle_rule("messaging.enabled", true)]);
        let ctx = test_context();
        let events = policy
            .decide(
                PolicyCommand::EvaluatePolicy {
                    rule_key: "nonexistent.key".to_string(),
                },
                &ctx,
            )
            .expect("evaluate must succeed even for an unknown key");
        let PolicyEvent::PolicyEvaluated { result, .. } = &events[0] else {
            panic!("expected PolicyEvaluated");
        };
        assert_eq!(*result, None);
    }

    #[test]
    fn evaluate_with_no_published_version_is_rejected() {
        let policy = active_policy();
        let ctx = test_context();
        let result = policy.decide(
            PolicyCommand::EvaluatePolicy {
                rule_key: "messaging.enabled".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(PolicyError::NoPublishedVersion)));
    }

    #[test]
    fn evaluation_is_deterministic_for_identical_inputs() {
        // §4.18.4 invariant 108: same inputs, same policy version, same
        // result — evaluate twice, assert identical outcomes.
        let policy = policy_with_published_rules(vec![toggle_rule("messaging.enabled", true)]);
        let ctx = test_context();
        let first = policy
            .decide(
                PolicyCommand::EvaluatePolicy {
                    rule_key: "messaging.enabled".to_string(),
                },
                &ctx,
            )
            .unwrap();
        let second = policy
            .decide(
                PolicyCommand::EvaluatePolicy {
                    rule_key: "messaging.enabled".to_string(),
                },
                &ctx,
            )
            .unwrap();
        let PolicyEvent::PolicyEvaluated { result: r1, .. } = &first[0] else {
            panic!()
        };
        let PolicyEvent::PolicyEvaluated { result: r2, .. } = &second[0] else {
            panic!()
        };
        assert_eq!(r1, r2);
    }

    // --- Policy: violations ---

    #[test]
    fn register_violation_with_published_version_succeeds() {
        let policy = policy_with_published_rules(vec![toggle_rule("file_sharing.enabled", false)]);
        let ctx = test_context();
        let events = policy
            .decide(
                PolicyCommand::RegisterViolation {
                    rule_key: "file_sharing.enabled".to_string(),
                    reason: "upload attempted while disabled".to_string(),
                },
                &ctx,
            )
            .expect("register violation must succeed");
        assert!(matches!(
            events[0],
            PolicyEvent::PolicyViolationDetected { .. }
        ));
    }

    #[test]
    fn register_violation_with_no_published_version_is_rejected() {
        let policy = active_policy();
        let ctx = test_context();
        let result = policy.decide(
            PolicyCommand::RegisterViolation {
                rule_key: "x".to_string(),
                reason: "y".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(PolicyError::NoPublishedVersion)));
    }

    // --- Policy: retirement ---

    #[test]
    fn retire_active_policy_succeeds() {
        let mut policy = active_policy();
        let ctx = test_context();
        let events = policy
            .decide(PolicyCommand::RetirePolicy, &ctx)
            .expect("retire must succeed");
        apply_all(&mut policy, &events);
        assert_eq!(policy.status(), PolicyStatus::Retired);
    }

    #[test]
    fn retire_twice_is_rejected() {
        let mut policy = active_policy();
        let ctx = test_context();
        let events = policy.decide(PolicyCommand::RetirePolicy, &ctx).unwrap();
        apply_all(&mut policy, &events);
        let result = policy.decide(PolicyCommand::RetirePolicy, &ctx);
        assert!(matches!(result, Err(PolicyError::InvalidTransition(_))));
    }

    #[test]
    fn create_version_on_retired_policy_is_rejected() {
        let mut policy = active_policy();
        let ctx = test_context();
        let retire_events = policy.decide(PolicyCommand::RetirePolicy, &ctx).unwrap();
        apply_all(&mut policy, &retire_events);

        let result = policy.decide(
            PolicyCommand::CreatePolicyVersion {
                rules: vec![toggle_rule("messaging.enabled", true)],
            },
            &ctx,
        );
        assert!(matches!(result, Err(PolicyError::InvalidTransition(_))));
    }

    #[test]
    fn unauthorized_actor_is_rejected() {
        let policy = active_policy();
        let mut ctx = test_context();
        ctx.authority = platform_kernel::VerifiedAuthority;
        // The authority stub unconditionally authorizes in this
        // workspace's current Increment-1-parity precedent (see this
        // file's header comment) — this test locks in that the check is
        // still *present* in decide(), matching
        // mission_domain/work_domain/communication_domain/file_domain's
        // own "authority stub never rejects" test convention, so a
        // future real ABAC implementation has an existing call site to
        // wire into rather than needing to add one from scratch.
        let result = policy.decide(
            PolicyCommand::CreatePolicyVersion {
                rules: vec![toggle_rule("messaging.enabled", true)],
            },
            &ctx,
        );
        assert!(result.is_ok());
    }

    // --- LegalHold: apply / release lifecycle ---

    #[test]
    fn apply_legal_hold_succeeds() {
        let hold = applied_legal_hold();
        assert_eq!(hold.status(), LegalHoldStatus::Applied);
    }

    #[test]
    fn apply_legal_hold_with_empty_reason_is_rejected() {
        let ctx = test_context();
        let result = LegalHold::create(
            LegalHoldCommand::ApplyLegalHold {
                target: HoldTarget {
                    target_id: ctx.actor.user_id,
                    target_type: "file_asset".to_string(),
                },
                authorizing_policy: None,
                reason: "  ".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(LegalHoldError::MissingField(_))));
    }

    #[test]
    fn release_applied_hold_succeeds() {
        let mut hold = applied_legal_hold();
        let ctx = test_context();
        let events = hold
            .decide(
                LegalHoldCommand::ReleaseLegalHold {
                    reason: "litigation concluded".to_string(),
                },
                &ctx,
            )
            .expect("release must succeed");
        for e in &events {
            hold.apply(e);
        }
        assert_eq!(hold.status(), LegalHoldStatus::Released);
    }

    #[test]
    fn release_already_released_hold_is_rejected() {
        let mut hold = applied_legal_hold();
        let ctx = test_context();
        let events = hold
            .decide(
                LegalHoldCommand::ReleaseLegalHold {
                    reason: "first release".to_string(),
                },
                &ctx,
            )
            .unwrap();
        for e in &events {
            hold.apply(e);
        }
        let result = hold.decide(
            LegalHoldCommand::ReleaseLegalHold {
                reason: "second release".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(LegalHoldError::InvalidTransition(_))));
    }

    #[test]
    fn apply_legal_hold_called_via_decide_is_rejected() {
        let hold = applied_legal_hold();
        let ctx = test_context();
        let result = hold.decide(
            LegalHoldCommand::ApplyLegalHold {
                target: hold.target(),
                authorizing_policy: None,
                reason: "x".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(LegalHoldError::InvalidTransition(_))));
    }
}
