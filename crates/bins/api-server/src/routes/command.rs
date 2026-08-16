use std::sync::Arc;

use audit_application::AuditRecord;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, SecondsFormat, Utc};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{
    ActorContext, AuthorityEpoch, CorrelationId, LifecycleEpoch, ObjectId, ObjectVersion,
    OperationId, ReplicaId, Timestamp, VectorClock,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::Instrument;

use super::{
    authenticate_headers, parse_object_id, web_device_object_id, ApiError, ApiState, CommandRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAggregate {
    pub id: ObjectId,
    pub public_id: String,
    pub title: String,
    pub message: String,
    pub priority: String,
    pub status: String,
    /// The user this notification is addressed to. Legacy notifications
    /// predate recipient targeting, so they deserialize as `None`.
    #[serde(default)]
    pub recipient_id: Option<String>,
    pub source_id: String,
    pub source_type: String,
    pub created_at: String,
    pub acknowledged_at: Option<String>,
    pub version: ObjectVersion,
    pub lifecycle_epoch: LifecycleEpoch,
    pub authority_epoch: AuthorityEpoch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationCommand {
    Acknowledge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationEvent {
    Acknowledged { acknowledged_at: String },
}

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("notification already acknowledged")]
    AlreadyAcknowledged,
}

impl AggregateRoot for NotificationAggregate {
    type Id = ObjectId;
    type Command = NotificationCommand;
    type Event = NotificationEvent;
    type Error = NotificationError;

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
        match command {
            NotificationCommand::Acknowledge if self.status == "acknowledged" => {
                Err(NotificationError::AlreadyAcknowledged)
            }
            NotificationCommand::Acknowledge => Ok(vec![NotificationEvent::Acknowledged {
                acknowledged_at: timestamp_to_iso(context.trusted_now.0),
            }]),
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            NotificationEvent::Acknowledged { acknowledged_at } => {
                self.status = "acknowledged".to_string();
                self.acknowledged_at = Some(acknowledged_at.clone());
                self.version = self.version.next();
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalAggregate {
    pub id: ObjectId,
    pub public_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub requested_by: String,
    pub target_id: String,
    pub target_type: String,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub decision_reason: Option<String>,
    pub web_action_permitted: bool,
    pub version: ObjectVersion,
    pub lifecycle_epoch: LifecycleEpoch,
    pub authority_epoch: AuthorityEpoch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalCommand {
    Approve { reason: Option<String> },
    Reject { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalEvent {
    Approved {
        decided_at: String,
        reason: Option<String>,
    },
    Rejected {
        decided_at: String,
        reason: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("approval is not pending")]
    NotPending,
    #[error("web action is not permitted for this approval")]
    WebActionNotPermitted,
}

impl AggregateRoot for ApprovalAggregate {
    type Id = ObjectId;
    type Command = ApprovalCommand;
    type Event = ApprovalEvent;
    type Error = ApprovalError;

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
        if self.status != "pending" {
            return Err(ApprovalError::NotPending);
        }
        if !self.web_action_permitted {
            return Err(ApprovalError::WebActionNotPermitted);
        }
        let decided_at = timestamp_to_iso(context.trusted_now.0);
        Ok(match command {
            ApprovalCommand::Approve { reason } => {
                vec![ApprovalEvent::Approved { decided_at, reason }]
            }
            ApprovalCommand::Reject { reason } => {
                vec![ApprovalEvent::Rejected { decided_at, reason }]
            }
        })
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            ApprovalEvent::Approved { decided_at, reason } => {
                self.status = "approved".to_string();
                self.decided_at = Some(decided_at.clone());
                self.decision_reason = reason.clone();
            }
            ApprovalEvent::Rejected { decided_at, reason } => {
                self.status = "rejected".to_string();
                self.decided_at = Some(decided_at.clone());
                self.decision_reason = Some(reason.clone());
            }
        }
        self.version = self.version.next();
    }
}

/// Confirms the caller is a Team Leader (or Admin) — the class design
/// doc §2.2 confirms carries "real, standing... authority to pre-check
/// Todo/Target items" (see
/// `security_application::ports::user_store::UserClass::TeamLeader`'s
/// own doc comment). **Added 2026-08-16, closing a real gap**: this
/// crate's own `todo_list.*`/`target_list.*` dispatch comment
/// previously stated `RecordTeamLeaderPreCheck` is "not gated here" —
/// true for verifier authority (D.4, correctly not required, since a
/// pre-check never gates verification per design doc §2.2), but that
/// reasoning had silently conflated "does this action gate
/// verification" with "who may perform this action" — the *class*
/// requirement (only a Team Leader may record one) had no check at
/// all, meaning any authenticated user could record a pre-check on any
/// list. This helper closes that gap without touching the non-gating
/// behavior, which remains correct and unchanged.
async fn require_team_leader_or_admin(
    state: &ApiState,
    actor: &ActorContext,
) -> Result<(), crate::CommandError> {
    let user_id = uuid::Uuid::from_bytes(actor.user_id.0).to_string();
    let user = state
        .user_store
        .find_by_id(&user_id)
        .await
        .map_err(|e| crate::CommandError::Persistence(e.to_string()))?
        .ok_or_else(|| crate::CommandError::Domain("actor not found".to_string()))?;
    let permitted =
        user.is_admin || user.class == Some(security_application::UserClass::TeamLeader);
    if permitted {
        Ok(())
    } else {
        Err(crate::CommandError::Domain(
            "only a Team Leader (or Admin) may record a pre-check".to_string(),
        ))
    }
}

/// Loads the `todo_list`/`target_list` aggregate at `target_id` and
/// confirms the caller (`actor`) is an authorized verifier for its
/// `owner`, per `verifier_resolution::is_authorized_verifier` (D.4).
/// Returns `Ok(())` if authorized; a `403`-mapped `CommandError`
/// otherwise. `aggregate_type` selects which repository to load from
/// (`"todo_list"` or `"target_list"`) — both aggregates carry an
/// `owner: ObjectId` field in the same position, so one function
/// serves both rather than two near-duplicates.
async fn require_verifier_authority(
    state: &ApiState,
    actor: &ActorContext,
    target_id: ObjectId,
    aggregate_type: &str,
) -> Result<(), crate::CommandError> {
    let repo = match aggregate_type {
        "todo_list" => &state.todo_list_repo,
        "target_list" => &state.target_list_repo,
        other => {
            return Err(crate::CommandError::Domain(format!(
                "require_verifier_authority called with unsupported aggregate_type {other}"
            )))
        }
    };
    let loaded = repo
        .load(&target_id)
        .await
        .map_err(|e| crate::CommandError::Persistence(e.to_string()))?
        .ok_or(crate::CommandError::NotFound(target_id))?;
    let owner_bytes = loaded
        .aggregate
        .get("owner")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            crate::CommandError::Domain("aggregate state missing 'owner' field".to_string())
        })?;
    if owner_bytes.len() != 16 {
        return Err(crate::CommandError::Domain(
            "'owner' field is not a 16-byte object id".to_string(),
        ));
    }
    let mut owner_id_bytes = [0u8; 16];
    for (i, b) in owner_bytes.iter().enumerate() {
        owner_id_bytes[i] = b
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| crate::CommandError::Domain("invalid 'owner' byte".to_string()))?;
    }
    let owner_id = ObjectId(owner_id_bytes);
    let organization_id_uuid = uuid::Uuid::from_bytes(actor.organization_id.0);

    let authorized = crate::verifier_resolution::is_authorized_verifier(
        actor.user_id,
        owner_id,
        organization_id_uuid,
        &state.user_store,
        &state.projection_pool,
    )
    .await
    .map_err(|e| crate::CommandError::Persistence(e.to_string()))?;

    if authorized {
        Ok(())
    } else {
        Err(crate::CommandError::Domain(
            "actor is not an authorized verifier for this list's owner".to_string(),
        ))
    }
}

pub async fn command_route(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(envelope): Json<CommandRequest>,
) -> Result<Json<Value>, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    if !auth
        .scope
        .command_types
        .iter()
        .any(|allowed| allowed == "*" || allowed == &envelope.command_type)
    {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "COMMAND_NOT_AUTHORIZED",
            "AUTHORITY",
            "NON_RETRYABLE",
            envelope.correlation_id.clone(),
            json!({"command_type": envelope.command_type}),
        ));
    }
    if auth.scope.object_type != "*" && auth.scope.object_type != envelope.target.object_type {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "OBJECT_SCOPE_MISMATCH",
            "AUTHORITY",
            "NON_RETRYABLE",
            envelope.correlation_id.clone(),
            json!({"target_type": envelope.target.object_type}),
        ));
    }
    if let Some(scoped_id) = &auth.scope.object_id {
        if scoped_id != &envelope.target.id {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "OBJECT_SCOPE_MISMATCH",
                "AUTHORITY",
                "NON_RETRYABLE",
                envelope.correlation_id.clone(),
                json!({"target_id": envelope.target.id}),
            ));
        }
    }
    if let Some(error) = super::test_mode_error(&headers, &envelope.correlation_id) {
        return Err(error);
    }
    if envelope.target.organization_id != auth.organization_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "TENANT_MISMATCH",
            "AUTHORITY",
            "NON_RETRYABLE",
            envelope.correlation_id,
            json!({}),
        ));
    }

    let expected_target_type = match envelope.command_type.as_str() {
        "notification.Acknowledge" => "notification",
        "approval.Approve" | "approval.Reject" => "approval",
        // Policy/LegalHold (added 2026-08-14 for the Admin platform —
        // `admin-shell` is a thin HTTP client, so Policy administration
        // has to be reachable over `/api/command`, not only through
        // `client-composition`'s separate registry that desktop-shell
        // uses. See DECISIONS.md's "Admin Platform" section.)
        "policy.CreatePolicyVersion"
        | "policy.PublishPolicyVersion"
        | "policy.EvaluatePolicy"
        | "policy.RegisterViolation"
        | "policy.RetirePolicy" => "policy",
        "legal_hold.ReleaseLegalHold" => "legal_hold",
        // TodoList/TargetList/StaffLoan (added 2026-08-16 — see
        // DESIGN_User_Hierarchy_Chain_of_Authority.md §4, §2.1 and
        // IMPLEMENTATION_PLAN_User_Hierarchy.md Phase C/D). Same
        // rationale as Policy/LegalHold above: `todo-domain` has no
        // `client-composition` wiring, so `/api/command` is these
        // aggregates' entry point. `CreateTodoList`/`CreateTargetList`/
        // `RequestStaffLoan` are `create()`-routed, not `decide()`-routed
        // — see `routes::todo_admin` for their dedicated REST routes,
        // mirroring `routes::policy_admin`'s precedent.
        "todo_list.AddItem"
        | "todo_list.SubmitTodoList"
        | "todo_list.RecordTeamLeaderPreCheck"
        | "todo_list.VerifyTodoList"
        | "todo_list.RejectTodoList"
        | "todo_list.EscalateTodoList" => "todo_list",
        "target_list.SubmitTargetList"
        | "target_list.RecordTeamLeaderPreCheck"
        | "target_list.VerifyTargetList"
        | "target_list.RejectTargetList"
        | "target_list.EscalateTargetList" => "target_list",
        "staff_loan.ApproveStaffLoan"
        | "staff_loan.DeclineStaffLoan"
        | "staff_loan.ExtendStaffLoan"
        | "staff_loan.EndStaffLoanEarly"
        | "staff_loan.ExpireStaffLoan" => "staff_loan",
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "UNSUPPORTED_WEB_COMMAND",
                "DOMAIN",
                "NON_RETRYABLE",
                envelope.correlation_id,
                json!({"command_type": envelope.command_type}),
            ));
        }
    };
    if envelope.target.object_type != expected_target_type {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "TARGET_TYPE_MISMATCH",
            "DOMAIN",
            "NON_RETRYABLE",
            envelope.correlation_id,
            json!({"expected": expected_target_type, "actual": envelope.target.object_type}),
        ));
    }

    let target_id = parse_object_id(&envelope.target.id)?;
    let operation_id = parse_operation_id(&envelope.operation_id)?;
    let correlation_id = parse_correlation_id(&envelope.correlation_id)?;
    let actor = ActorContext {
        user_id: parse_object_id(&auth.user_id)?,
        device_id: web_device_object_id(),
        organization_id: parse_object_id(&auth.organization_id)?,
    };
    let vector_clock = parse_vector_clock(&envelope.vector_clock.entries)?;

    // Validated up front, before the dispatch block below, specifically
    // so a malformed payload becomes a real 400 — inside that block the
    // error type is `CommandError`, which cannot express an
    // `ApiError`/HTTP status, so validation there would have to either
    // silently coerce bad input or panic. Only `policy.CreatePolicyVersion`
    // carries a structurally-typed payload today; every other supported
    // command's payload is plain strings read with `unwrap_or_default`.
    let policy_rules: Vec<policy_domain::value::PolicyRule> =
        if envelope.command_type == "policy.CreatePolicyVersion" {
            match envelope.payload.get("rules").cloned() {
                Some(raw) => serde_json::from_value(raw).map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "INVALID_PAYLOAD",
                        "DOMAIN",
                        "NON_RETRYABLE",
                        envelope.correlation_id.clone(),
                        json!({"message": format!("invalid 'rules' payload: {e}")}),
                    )
                })?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
    let was_duplicate = state
        .idempotency_store
        .get(&operation_id)
        .await
        .map_err(|_error| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "IDEMPOTENCY_LOOKUP_FAILED",
                "INFRASTRUCTURE",
                "TRANSIENT",
                envelope.correlation_id.clone(),
                json!({"message": "Idempotency service unavailable"}),
            )
        })?
        .is_some();

    let command_span = tracing::info_span!(
        "command.execute",
        operation_id = %envelope.operation_id,
        actor_id = %auth.user_id,
        organization_id = %auth.organization_id,
        command_type = %envelope.command_type,
    );
    let result = async {
        match envelope.command_type.as_str() {
            "notification.Acknowledge" => {
                crate::handle_command::<NotificationAggregate, _, _, _>(
                    NotificationCommand::Acknowledge,
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "notification",
                    Arc::clone(&state.notification_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            "approval.Approve" => {
                let reason = envelope
                    .payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                crate::handle_command::<ApprovalAggregate, _, _, _>(
                    ApprovalCommand::Approve { reason },
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "approval",
                    Arc::clone(&state.approval_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            "approval.Reject" => {
                let reason = envelope
                    .payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("Rejected from web client")
                    .to_string();
                crate::handle_command::<ApprovalAggregate, _, _, _>(
                    ApprovalCommand::Reject { reason },
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "approval",
                    Arc::clone(&state.approval_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            "policy.CreatePolicyVersion" => {
                // `policy_rules` was parsed and validated before this
                // block — see its declaration above for why validation
                // cannot happen here.
                crate::handle_command::<policy_domain::Policy, _, _, _>(
                    policy_domain::PolicyCommand::CreatePolicyVersion {
                        rules: policy_rules.clone(),
                    },
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "policy",
                    Arc::clone(&state.policy_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            "policy.PublishPolicyVersion" => {
                crate::handle_command::<policy_domain::Policy, _, _, _>(
                    policy_domain::PolicyCommand::PublishPolicyVersion,
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "policy",
                    Arc::clone(&state.policy_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            "policy.EvaluatePolicy" => {
                let rule_key = envelope
                    .payload
                    .get("rule_key")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                crate::handle_command::<policy_domain::Policy, _, _, _>(
                    policy_domain::PolicyCommand::EvaluatePolicy { rule_key },
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "policy",
                    Arc::clone(&state.policy_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            "policy.RegisterViolation" => {
                let rule_key = envelope
                    .payload
                    .get("rule_key")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let reason = envelope
                    .payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                crate::handle_command::<policy_domain::Policy, _, _, _>(
                    policy_domain::PolicyCommand::RegisterViolation { rule_key, reason },
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "policy",
                    Arc::clone(&state.policy_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            "policy.RetirePolicy" => {
                crate::handle_command::<policy_domain::Policy, _, _, _>(
                    policy_domain::PolicyCommand::RetirePolicy,
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "policy",
                    Arc::clone(&state.policy_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            "legal_hold.ReleaseLegalHold" => {
                let reason = envelope
                    .payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                crate::handle_command::<policy_domain::LegalHold, _, _, _>(
                    policy_domain::LegalHoldCommand::ReleaseLegalHold { reason },
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "legal_hold",
                    Arc::clone(&state.legal_hold_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            // TodoList/TargetList/StaffLoan (added 2026-08-16). Unlike
            // Policy/LegalHold above, these commands' payloads are
            // deserialized in one shot rather than field-by-field: the
            // command enums use plain externally-tagged serde (no
            // `#[serde(tag = ...)]` override), so a payload shaped like
            // `{"VerifyTodoList": {"outcome": "Flawless", "comment": null}}`
            // deserializes directly into `todo_domain::TodoListCommand`
            // — the same technique `client-composition`'s
            // `DecisionHandler`s use (see
            // `client_composition::handlers::decision_handler`), chosen
            // here over manual field extraction because these commands
            // carry richer typed fields (enums, `Option<String>`,
            // timestamps) that would be error-prone to pluck by hand.
            //
            // Verifier authorization (D.4, added 2026-08-16): only
            // `VerifyTodoList`/`RejectTodoList`/`EscalateTodoList`
            // require the caller to be an authorized verifier per
            // `verifier_resolution::is_authorized_verifier` — `AddItem`/
            // `SubmitTodoList` are performed by the owner (design doc
            // §4.0.1's bidirectional-creation rule), not a verifying
            // Manager, and are not gated here. `RecordTeamLeaderPreCheck`
            // has its own, separate class-based gate below
            // (`require_team_leader_or_admin`, added 2026-08-16) — see
            // that function's doc comment for why this was a real gap
            // this dispatch comment previously described incorrectly.
            // This check loads the current aggregate state first
            // specifically to read `owner` — the same load
            // `handle_command` performs internally moments later, so
            // this is a real second read, accepted as the simplest
            // correct implementation rather than threading owner-lookup
            // through `handle_command`'s generic signature.
            cmd if cmd.starts_with("todo_list.") => {
                if matches!(
                    envelope.command_type.as_str(),
                    "todo_list.VerifyTodoList"
                        | "todo_list.RejectTodoList"
                        | "todo_list.EscalateTodoList"
                ) {
                    require_verifier_authority(&state, &actor, target_id, "todo_list").await?;
                }
                if envelope.command_type == "todo_list.RecordTeamLeaderPreCheck" {
                    require_team_leader_or_admin(&state, &actor).await?;
                }
                let command: todo_domain::TodoListCommand =
                    serde_json::from_value(envelope.payload.clone()).map_err(|e| {
                        crate::CommandError::Domain(format!("invalid payload: {e}"))
                    })?;
                crate::handle_command::<todo_domain::TodoList, _, _, _>(
                    command,
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "todo_list",
                    Arc::clone(&state.todo_list_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            cmd if cmd.starts_with("target_list.") => {
                if matches!(
                    envelope.command_type.as_str(),
                    "target_list.VerifyTargetList"
                        | "target_list.RejectTargetList"
                        | "target_list.EscalateTargetList"
                ) {
                    require_verifier_authority(&state, &actor, target_id, "target_list").await?;
                }
                if envelope.command_type == "target_list.RecordTeamLeaderPreCheck" {
                    require_team_leader_or_admin(&state, &actor).await?;
                }
                let command: todo_domain::TargetListCommand =
                    serde_json::from_value(envelope.payload.clone()).map_err(|e| {
                        crate::CommandError::Domain(format!("invalid payload: {e}"))
                    })?;
                crate::handle_command::<todo_domain::TargetList, _, _, _>(
                    command,
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "target_list",
                    Arc::clone(&state.target_list_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            cmd if cmd.starts_with("staff_loan.") => {
                let command: todo_domain::StaffLoanCommand =
                    serde_json::from_value(envelope.payload.clone()).map_err(|e| {
                        crate::CommandError::Domain(format!("invalid payload: {e}"))
                    })?;
                crate::handle_command::<todo_domain::StaffLoan, _, _, _>(
                    command,
                    target_id,
                    operation_id,
                    actor.clone(),
                    ObjectVersion(envelope.expected_version),
                    LifecycleEpoch(envelope.expected_lifecycle_epoch),
                    AuthorityEpoch(envelope.expected_authority_epoch),
                    vector_clock.clone(),
                    correlation_id,
                    "staff_loan",
                    Arc::clone(&state.staff_loan_repo),
                    Arc::clone(&state.unit_factory),
                    Arc::clone(&state.idempotency_store),
                )
                .await
            }
            _ => unreachable!("command type was validated before dispatch"),
        }
    }
    .instrument(command_span)
    .await;

    let result = match result {
        Ok(result) => result,
        Err(error) => {
            append_command_audit(
                &state,
                &envelope,
                &actor,
                "rejected",
                json!({"error_class": command_error_class(&error)}),
            )
            .await;
            return Err(map_command_error(error, &envelope.correlation_id));
        }
    };

    let public_result = publicize_command_result(&result)?;
    append_command_audit(&state, &envelope, &actor, "accepted", json!({
        "duplicate": was_duplicate,
        "event_count": public_result.get("events").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
    })).await;
    if !was_duplicate {
        if let Some(events) = public_result.get("events").and_then(Value::as_array) {
            for event in events {
                let _ = state.events.send(event.clone());
            }
        }
    }

    Ok(Json(public_result))
}

async fn append_command_audit(
    state: &ApiState,
    envelope: &CommandRequest,
    actor: &ActorContext,
    outcome: &str,
    safe_details: Value,
) {
    let operation_id = parse_operation_id(&envelope.operation_id).ok();
    let correlation_id = parse_correlation_id(&envelope.correlation_id).ok();
    let record = AuditRecord {
        organization_id: actor.organization_id,
        actor_id: Some(actor.user_id),
        operation_id,
        correlation_id,
        category: "command".to_string(),
        action: envelope.command_type.clone(),
        outcome: outcome.to_string(),
        occurred_at: Timestamp::now(),
        safe_details: json!({
            "target_id": envelope.target.id,
            "target_type": envelope.target.object_type,
            "details": safe_details,
        }),
    };
    match state.audit_writer.append(&record).await {
        Ok(_) => state.metrics.audit_entries_total.inc(),
        Err(_error) => {
            state.metrics.observability_export_errors_total.inc();
            tracing::error!(
                operation_id = %envelope.operation_id,
                outcome = "audit_append_failed",
                error_class = "audit_persistence",
                "failed to append command audit entry"
            );
        }
    }
}

fn command_error_class(error: &crate::CommandError) -> &'static str {
    match error {
        crate::CommandError::NotFound(_) => "not_found",
        crate::CommandError::VersionConflict { .. } => "version_conflict",
        crate::CommandError::EpochConflict { .. } => "lifecycle_epoch_conflict",
        crate::CommandError::AuthorityEpochConflict { .. } => "authority_epoch_conflict",
        crate::CommandError::Domain(_) => "domain_rejection",
        crate::CommandError::Persistence(_) => "persistence_failure",
        crate::CommandError::Serialization(_) => "serialization_failure",
        crate::CommandError::Idempotency(_) => "idempotency_failure",
    }
}

fn parse_operation_id(value: &str) -> Result<OperationId, ApiError> {
    parse_object_id(value).map(|id| OperationId(id.0))
}

fn parse_correlation_id(value: &str) -> Result<CorrelationId, ApiError> {
    parse_object_id(value).map(|id| CorrelationId(id.0))
}

fn parse_vector_clock(
    entries: &std::collections::HashMap<String, u64>,
) -> Result<VectorClock, ApiError> {
    let mut clock = VectorClock::new();
    for (replica, count) in entries {
        let parsed = uuid::Uuid::parse_str(replica).map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_REPLICA_ID",
                "DOMAIN",
                "NON_RETRYABLE",
                uuid::Uuid::new_v4().to_string(),
                json!({"replica_id": replica}),
            )
        })?;
        clock.entries.insert(ReplicaId(*parsed.as_bytes()), *count);
    }
    Ok(clock)
}

fn publicize_command_result(result: &Value) -> Result<Value, ApiError> {
    let operation_id = result
        .get("operation_id")
        .and_then(kernel_id_to_string)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INVALID_COMMAND_RESULT",
                "INFRASTRUCTURE",
                "TRANSIENT",
                uuid::Uuid::new_v4().to_string(),
                json!({"field":"operation_id"}),
            )
        })?;
    let events = result
        .get("events")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(publicize_event)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(json!({
        "success": result.get("success").and_then(Value::as_bool).unwrap_or(true),
        "operation_id": operation_id,
        "new_version": result.get("new_version").cloned().unwrap_or(Value::Null),
        "new_lifecycle_epoch": result.get("new_lifecycle_epoch").cloned().unwrap_or(Value::Null),
        "new_authority_epoch": result.get("new_authority_epoch").cloned().unwrap_or(Value::Null),
        "events": events,
    }))
}

fn publicize_event(event: &Value) -> Result<Value, ApiError> {
    let missing_container = |field: &str| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INVALID_DOMAIN_EVENT",
            "INFRASTRUCTURE",
            "TRANSIENT",
            uuid::Uuid::new_v4().to_string(),
            json!({"field": field}),
        )
    };
    let aggregate_ref = event
        .get("aggregate_ref")
        .ok_or_else(|| missing_container("aggregate_ref"))?;
    let actor = event
        .get("actor")
        .ok_or_else(|| missing_container("actor"))?;
    let payload = event.get("payload").cloned().unwrap_or(Value::Null);
    let aggregate_type = aggregate_ref
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let event_type = match aggregate_type {
        "notification" => "notification.NotificationAcknowledged",
        "approval" if payload.get("Approved").is_some() => "approval.ApprovalGranted",
        "approval" if payload.get("Rejected").is_some() => "approval.ApprovalRejected",
        _ => "web.CommandAccepted",
    };

    let required_id = |container: &Value, field: &str| {
        container
            .get(field)
            .and_then(kernel_id_to_string)
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INVALID_DOMAIN_EVENT",
                    "INFRASTRUCTURE",
                    "TRANSIENT",
                    uuid::Uuid::new_v4().to_string(),
                    json!({"field": field}),
                )
            })
    };

    Ok(json!({
        "event_id": required_id(event, "event_id")?,
        "event_type": event_type,
        "schema_version": event.get("schema_version").cloned().unwrap_or(json!("1.0")),
        "aggregate_ref": {
            "id": required_id(aggregate_ref, "id")?,
            "type": aggregate_type,
            "organization_id": required_id(aggregate_ref, "organization_id")?,
        },
        "aggregate_version": event.get("aggregate_version").cloned().unwrap_or(json!(0)),
        "lifecycle_epoch": event.get("lifecycle_epoch").cloned().unwrap_or(json!(0)),
        "authority_epoch": event.get("authority_epoch").cloned().unwrap_or(json!(0)),
        "operation_id": required_id(event, "operation_id")?,
        "actor": {
            "user_id": required_id(actor, "user_id")?,
            "device_id": "web-client",
            "organization_id": required_id(actor, "organization_id")?,
        },
        "occurred_at": timestamp_value_to_iso(event.get("occurred_at")),
        "recorded_at": timestamp_value_to_iso(event.get("recorded_at")),
        "vector_clock": event.get("vector_clock").cloned().unwrap_or(json!({"entries":{}})),
        "correlation_id": required_id(event, "correlation_id")?,
        "causation_id": event.get("causation_id").and_then(kernel_id_to_string),
        "payload": payload,
    }))
}

fn kernel_id_to_string(value: &Value) -> Option<String> {
    let bytes = value.as_array()?;
    if bytes.len() != 16 {
        return None;
    }
    let mut raw = [0_u8; 16];
    for (index, byte) in bytes.iter().enumerate() {
        raw[index] = u8::try_from(byte.as_u64()?).ok()?;
    }
    Some(uuid::Uuid::from_bytes(raw).to_string())
}

fn timestamp_value_to_iso(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_u64)
        .map(timestamp_to_iso)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

fn map_command_error(error: crate::CommandError, correlation_id: &str) -> ApiError {
    match error {
        crate::CommandError::VersionConflict { expected, actual } => ApiError::new(
            StatusCode::CONFLICT,
            "VERSION_CONFLICT",
            "CONCURRENCY",
            "RETRYABLE",
            correlation_id,
            json!({"expected": expected.0, "actual": actual.0}),
        ),
        crate::CommandError::EpochConflict { expected, actual } => ApiError::new(
            StatusCode::CONFLICT,
            "LIFECYCLE_EPOCH_CONFLICT",
            "CONCURRENCY",
            "RETRYABLE",
            correlation_id,
            json!({"expected": expected.0, "actual": actual.0}),
        ),
        crate::CommandError::AuthorityEpochConflict { expected, actual } => ApiError::new(
            StatusCode::CONFLICT,
            "AUTHORITY_EPOCH_CONFLICT",
            "CONCURRENCY",
            "RETRYABLE",
            correlation_id,
            json!({"expected": expected.0, "actual": actual.0}),
        ),
        crate::CommandError::NotFound(_) => ApiError::new(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "DOMAIN",
            "NON_RETRYABLE",
            correlation_id,
            json!({}),
        ),
        crate::CommandError::Domain(message) if message.contains("not permitted") => ApiError::new(
            StatusCode::FORBIDDEN,
            "WEB_ACTION_NOT_PERMITTED",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation_id,
            json!({"message": message}),
        ),
        crate::CommandError::Domain(message)
            if message.contains("already acknowledged") || message.contains("not pending") =>
        {
            ApiError::new(
                StatusCode::CONFLICT,
                "STATE_CONFLICT",
                "CONCURRENCY",
                "RETRYABLE",
                correlation_id,
                json!({"message": message}),
            )
        }
        crate::CommandError::Domain(message) => ApiError::new(
            StatusCode::BAD_REQUEST,
            "COMMAND_REJECTED",
            "DOMAIN",
            "NON_RETRYABLE",
            correlation_id,
            json!({"message": message}),
        ),
        other => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "COMMAND_FAILED",
            "INFRASTRUCTURE",
            "TRANSIENT",
            correlation_id,
            json!({"message": other.to_string()}),
        ),
    }
}

fn timestamp_to_iso(nanos: u64) -> String {
    let seconds = (nanos / 1_000_000_000) as i64;
    let subsecond_nanos = (nanos % 1_000_000_000) as u32;
    DateTime::<Utc>::from_timestamp(seconds, subsecond_nanos)
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}
