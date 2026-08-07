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
    authenticate_headers, parse_object_id, web_device_object_id, ApiError,
    ApiState, CommandRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAggregate {
    pub id: ObjectId,
    pub public_id: String,
    pub title: String,
    pub message: String,
    pub priority: String,
    pub status: String,
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
