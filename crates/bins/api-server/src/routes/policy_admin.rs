//! Admin-only creation routes for `Policy` and `LegalHold`. Added
//! 2026-08-14, closing a gap flagged in `DECISIONS.md`'s "Admin
//! Platform" section: `CreatePolicy` and `ApplyLegalHold` are creation
//! commands, routed through `Aggregate::create()` rather than
//! `decide()`, so they cannot go through `routes::command`'s
//! `handle_command`-based dispatch the way every other Policy/LegalHold
//! command added there does. This module gives them their own,
//! REST-shaped entry points instead — the same pattern
//! `routes::profiles::upsert_profile_route` already uses for
//! `StaffProfile`'s own creation command.

use axum::{extract::State, http::HeaderMap, Json};
use platform_contracts::AggregateRoot;
use policy_domain::{value::PolicyScope, LegalHold, LegalHoldCommand, Policy, PolicyCommand};
use serde::{Deserialize, Serialize};

use super::admin::require_admin_mutation;
use super::profiles::{decision_context_for_actor, domain_error, infrastructure_error};
use super::ApiState;
use crate::routes::ApiError;

#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreatedPolicyDto {
    pub policy_id: String,
}

/// `POST /api/admin/policies` — admin-only. Creates a new,
/// organization-scoped `Policy` container with zero versions (no rules
/// are in effect until a version is drafted and published via
/// `policy.CreatePolicyVersion`/`policy.PublishPolicyVersion` over
/// `/api/command`, both of which already work — see
/// `routes::command`'s Policy dispatch arms).
pub async fn create_policy(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<CreatePolicyRequest>,
) -> Result<Json<CreatedPolicyDto>, ApiError> {
    let admin = require_admin_mutation(&state, &headers).await?;
    let organization_id = uuid::Uuid::parse_str(&admin.organization_id)
        .map_err(|_| domain_error("invalid organization id"))?;
    let ctx = decision_context_for_actor(&admin.user_id, organization_id)?;

    let events = Policy::create(
        PolicyCommand::CreatePolicy {
            name: req.name,
            scope: PolicyScope::Organization(platform_kernel::ObjectId(
                *organization_id.as_bytes(),
            )),
        },
        &ctx,
    )
    .map_err(domain_error)?;
    let policy = Policy::from_created_event(&events[0]);
    let aggregate_state =
        serde_json::to_value(&policy).map_err(|e| infrastructure_error(e.to_string()))?;

    let mut unit = state
        .unit_factory
        .create(platform_kernel::ObjectId(*organization_id.as_bytes()))
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    state
        .policy_repo
        .commit(aggregate_state, &[], &mut *unit)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    unit.commit()
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;

    Ok(Json(CreatedPolicyDto {
        policy_id: policy.id().0.to_string(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ApplyLegalHoldRequest {
    pub target_id: String,
    pub target_type: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct CreatedLegalHoldDto {
    pub legal_hold_id: String,
}

/// `POST /api/admin/legal-holds` — admin-only. Applies a new
/// `LegalHold` ad hoc (no authorizing policy reference from this
/// route — see `LegalHoldCommand::ApplyLegalHold`'s own doc comment on
/// why a hold need not always trace back to a published policy rule).
pub async fn apply_legal_hold(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<ApplyLegalHoldRequest>,
) -> Result<Json<CreatedLegalHoldDto>, ApiError> {
    let admin = require_admin_mutation(&state, &headers).await?;
    let organization_id = uuid::Uuid::parse_str(&admin.organization_id)
        .map_err(|_| domain_error("invalid organization id"))?;
    let ctx = decision_context_for_actor(&admin.user_id, organization_id)?;

    let target_uuid = uuid::Uuid::parse_str(&req.target_id)
        .map_err(|_| domain_error("target_id is not a valid UUID"))?;

    let events = LegalHold::create(
        LegalHoldCommand::ApplyLegalHold {
            target: policy_domain::value::HoldTarget {
                target_id: platform_kernel::ObjectId(*target_uuid.as_bytes()),
                target_type: req.target_type,
            },
            authorizing_policy: None,
            reason: req.reason,
        },
        &ctx,
    )
    .map_err(domain_error)?;
    let hold = LegalHold::from_created_event(&events[0]);
    let aggregate_state =
        serde_json::to_value(&hold).map_err(|e| infrastructure_error(e.to_string()))?;

    let mut unit = state
        .unit_factory
        .create(platform_kernel::ObjectId(*organization_id.as_bytes()))
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    state
        .legal_hold_repo
        .commit(aggregate_state, &[], &mut *unit)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    unit.commit()
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;

    Ok(Json(CreatedLegalHoldDto {
        legal_hold_id: hold.id().0.to_string(),
    }))
}
