//! Staff profile routes: public view, admin-only edit, and admin-only
//! organization-wide batch import/export.
//!
//! Requested 2026-08-13: "a full detailed profile page for each Staff
//! member... shows info publicly and access for modifications for
//! admin," followed by "capability to import or export staff list
//! profiles by admin access as a batch for an organization."
//!
//! # Confirmed visibility (see `DECISIONS.md` "Staff Profiles")
//! - Basic identity + organizational info: public to every authenticated
//!   user in the organization.
//! - Work stats: visible only to `UserClass::TopLevelManager` and Admin.
//! - Editing (single or batch): Admin-only, no exceptions.
//!
//! # Why raw SQL against `aggregates` instead of `Repository::load`
//! `Repository` (see `query_application::Repository`) only supports
//! load-by-id — there is no "find the profile whose `owner_id` is X"
//! capability, and batch export needs "every profile in this org."
//! Reused the same `aggregates` table scan `query_handler.rs` already
//! uses for list-style queries (`aggregate_type` + JSON `state` column)
//! rather than inventing a second mechanism — see `find_by_owner`/
//! `list_all_in_org` below.

pub mod batch;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{ActorContext, PolicyDecisionSet, Timestamp, VerifiedAuthority};
use profile_domain::{BasicIdentity, OrganizationalInfo, ProfileCommand, StaffProfile, WorkStats};
use security_application::UserClass;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{authenticate_headers, ApiError, ApiState, ProjectionPool};
use crate::routes::admin::{require_admin, require_class};

pub(super) fn correlation() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(super) fn domain_error(message: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "PROFILE_DOMAIN_ERROR",
        "VALIDATION",
        "NON_RETRYABLE",
        correlation(),
        json!({"message": message.to_string()}),
    )
}

pub(super) fn infrastructure_error(message: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %message, "profile route infrastructure failure");
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "PROFILE_STORE_UNAVAILABLE",
        "INFRASTRUCTURE",
        "TRANSIENT",
        correlation(),
        json!({}),
    )
}

// ------------------------------------------------------------- DTOs --

/// Public-facing profile view. Excludes `work_stats` entirely at the
/// type level for viewers who aren't authorized to see it — see
/// `ProfileDtoWithStats` for the Manager/Admin variant. Two DTOs rather
/// than one with an `Option<WorkStats>` that's sometimes `None` for
/// authorization reasons and sometimes `None` because the data doesn't
/// exist yet (`WorkStats::unavailable()`): conflating "you can't see
/// this" with "there's nothing to see" would be a real information leak
/// in one direction (a viewer could infer whether stats exist to
/// someone who has no data) — a minor one, but avoidable by keeping the
/// gate at the type/route level instead.
#[derive(Debug, Serialize)]
pub struct PublicProfileDto {
    pub profile_id: String,
    pub owner_id: String,
    pub full_name: String,
    pub photo_blob_key: Option<String>,
    pub job_title: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub department: Option<String>,
    pub class_label: Option<String>,
    pub parent_display_name: Option<String>,
    pub team_memberships: Vec<String>,
}

/// Same fields as `PublicProfileDto`, plus `work_stats` — returned only
/// to callers `require_class`-gated to `TopLevelManager`/Admin.
#[derive(Debug, Serialize)]
pub struct ProfileDtoWithStats {
    #[serde(flatten)]
    pub public: PublicProfileDto,
    pub work_stats: WorkStatsDto,
}

#[derive(Debug, Serialize)]
pub struct WorkStatsDto {
    pub assigned_items_count: Option<u32>,
    pub completed_items_count: Option<u32>,
    pub target_summary: Option<String>,
}

impl From<&WorkStats> for WorkStatsDto {
    fn from(w: &WorkStats) -> Self {
        Self {
            assigned_items_count: w.assigned_items_count,
            completed_items_count: w.completed_items_count,
            target_summary: w.target_summary.clone(),
        }
    }
}

pub(super) fn to_public_dto(profile: &StaffProfile) -> PublicProfileDto {
    let identity = profile.identity();
    let org = profile.organizational_info();
    PublicProfileDto {
        profile_id: profile.id().0.to_string(),
        owner_id: profile.owner_id().to_string(),
        full_name: identity.full_name.clone(),
        photo_blob_key: identity.photo_blob_key.clone(),
        job_title: identity.job_title.clone(),
        contact_email: identity.contact_email.clone(),
        contact_phone: identity.contact_phone.clone(),
        department: org.department.clone(),
        class_label: org.class_label.clone(),
        parent_display_name: org.parent_display_name.clone(),
        team_memberships: org.team_memberships.clone(),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpsertProfileRequest {
    pub owner_id: String,
    pub full_name: String,
    pub photo_blob_key: Option<String>,
    pub job_title: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub department: Option<String>,
    pub class_label: Option<String>,
    pub parent_display_name: Option<String>,
    #[serde(default)]
    pub team_memberships: Vec<String>,
}

// --------------------------------------------------- Storage helpers --

/// Loads and rehydrates a `StaffProfile` from its own aggregate id.
///
/// `#[allow(dead_code)]`: no caller yet — `get_profile`/`list_profiles`
/// both look up by `owner_id` (via `find_by_owner`), not by the
/// profile's own id. Kept for the admin edit screen (not yet built),
/// which will plausibly fetch-by-profile-id after a search/list step
/// rather than by owner_id a second time.
#[allow(dead_code)]
async fn load_profile(
    state: &ApiState,
    profile_object_id: platform_kernel::ObjectId,
) -> Result<Option<StaffProfile>, ApiError> {
    let loaded = state
        .profile_repo
        .load(&profile_object_id)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    let Some(loaded) = loaded else {
        return Ok(None);
    };
    let profile: StaffProfile = serde_json::from_value(loaded.aggregate)
        .map_err(|e| infrastructure_error(e.to_string()))?;
    Ok(Some(profile))
}

/// Finds the `StaffProfile` owned by `owner_id`, if any, by scanning the
/// `staff_profile` aggregates for this organization. See this module's
/// header comment for why `Repository` alone can't do this.
///
/// Compares as byte arrays, not strings: `ObjectId(pub [u8; 16])`'s
/// derived `Serialize` produces a JSON array of 16 numbers, not a UUID
/// string (confirmed by reading `platform_kernel::identifiers` — no
/// custom `Serialize` override exists there) — an earlier version of
/// this function compared `state["owner_id"].as_str()` against a UUID
/// string and always got `None`, caught by
/// `tests/staff_profile_routes.rs`'s upsert-then-list test actually
/// finding two profiles instead of one.
pub(super) async fn find_by_owner(
    state: &ApiState,
    organization_id: uuid::Uuid,
    owner_id: &str,
) -> Result<Option<StaffProfile>, ApiError> {
    let owner_uuid = uuid::Uuid::parse_str(owner_id)
        .map_err(|_| domain_error("owner_id is not a valid UUID"))?;
    let owner_bytes: Vec<serde_json::Value> = owner_uuid
        .as_bytes()
        .iter()
        .map(|b| serde_json::json!(b))
        .collect();

    let rows = fetch_staff_profile_rows(state, organization_id)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    for row in rows {
        let matches = row
            .get("owner_id")
            .and_then(|v| v.as_array())
            .map(|arr| arr == &owner_bytes)
            .unwrap_or(false);
        if matches {
            let profile: StaffProfile =
                serde_json::from_value(row).map_err(|e| infrastructure_error(e.to_string()))?;
            return Ok(Some(profile));
        }
    }
    Ok(None)
}

/// Every `staff_profile` aggregate's raw `state` JSON for this
/// organization. Shared by `find_by_owner` (single lookup) and
/// `list_profiles`/`export_profiles` (whole-org batch).
pub(super) async fn fetch_staff_profile_rows(
    state: &ApiState,
    organization_id: uuid::Uuid,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    match &state.projection_pool {
        ProjectionPool::Sqlite(pool) => {
            let rows = sqlx::query_as::<_, (String,)>(
                "SELECT state FROM aggregates WHERE organization_id = ? AND aggregate_type = ?",
            )
            .bind(organization_id.as_bytes().to_vec())
            .bind("staff_profile")
            .fetch_all(pool)
            .await?;
            Ok(rows
                .into_iter()
                .filter_map(|(state,)| serde_json::from_str(&state).ok())
                .collect())
        }
        ProjectionPool::Postgres(pool) => {
            let rows = sqlx::query_as::<_, (serde_json::Value,)>(
                "SELECT state FROM aggregates WHERE organization_id = $1 AND aggregate_type = $2",
            )
            .bind(organization_id)
            .bind("staff_profile")
            .fetch_all(pool)
            .await?;
            Ok(rows.into_iter().map(|(state,)| state).collect())
        }
    }
}

fn actor_context_for(user_id: &str, organization_id: uuid::Uuid) -> ActorContext {
    let user_uuid = uuid::Uuid::parse_str(user_id).unwrap_or_default();
    ActorContext {
        user_id: platform_kernel::ObjectId(*user_uuid.as_bytes()),
        device_id: platform_kernel::ObjectId([0u8; 16]),
        organization_id: platform_kernel::ObjectId(*organization_id.as_bytes()),
    }
}

fn decision_context_for(actor: ActorContext) -> DecisionContext {
    DecisionContext {
        actor,
        authority: VerifiedAuthority,
        trusted_now: Timestamp::from_nanos(
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64
        ),
        policy_outcomes: PolicyDecisionSet,
        generated_id_generator: Box::new(RandomIdGenerator),
    }
}

/// Combines `actor_context_for` + `decision_context_for` for callers
/// (e.g. `routes::policy_admin`) that just need a `DecisionContext` for
/// a given admin user id and org, without the two intermediate steps.
/// Returns a domain `ApiError` (400) rather than panicking/defaulting
/// if `user_id` isn't a valid UUID — the equivalent inline code in this
/// module's own routes silently falls back to `Uuid::default()` via
/// `actor_context_for`'s `unwrap_or_default()`, which is fine there
/// because those callers already validated the id earlier in the same
/// function; this helper has no such guarantee about its callers, so it
/// validates explicitly.
pub(super) fn decision_context_for_actor(
    user_id: &str,
    organization_id: uuid::Uuid,
) -> Result<DecisionContext, ApiError> {
    uuid::Uuid::parse_str(user_id).map_err(|_| domain_error("invalid actor user id"))?;
    Ok(decision_context_for(actor_context_for(
        user_id,
        organization_id,
    )))
}

/// A random `IdGenerator`, local to this module — the equivalent
/// private type in `command_handler.rs` (`DefaultIdGenerator`) is not
/// `pub`, so this is a small, deliberate duplicate rather than widening
/// an internal implementation detail's visibility for one caller.
struct RandomIdGenerator;
impl platform_contracts::IdGenerator for RandomIdGenerator {
    fn generate_object_id(&self) -> platform_kernel::ObjectId {
        platform_kernel::ObjectId::new_random()
    }
    fn generate_operation_id(&self) -> platform_kernel::OperationId {
        platform_kernel::OperationId::new_random()
    }
    fn generate_event_id(&self) -> platform_kernel::EventId {
        platform_kernel::EventId::new_random()
    }
}

fn request_to_identity(req: &UpsertProfileRequest) -> Result<BasicIdentity, ApiError> {
    BasicIdentity::new(
        req.full_name.clone(),
        req.photo_blob_key.clone(),
        req.job_title.clone(),
        req.contact_email.clone(),
        req.contact_phone.clone(),
    )
    .map_err(domain_error)
}

fn request_to_org_info(req: &UpsertProfileRequest) -> OrganizationalInfo {
    OrganizationalInfo {
        department: req.department.clone(),
        class_label: req.class_label.clone(),
        parent_display_name: req.parent_display_name.clone(),
        team_memberships: req.team_memberships.clone(),
    }
}

/// Creates a new profile, or updates the existing one for the same
/// `owner_id` — the confirmed upsert behavior (2026-08-13: "update
/// existing profiles, create new ones for unmatched entries").
pub(super) async fn upsert_profile(
    state: &ApiState,
    actor: &platform_kernel::UserId,
    organization_id: uuid::Uuid,
    req: UpsertProfileRequest,
) -> Result<StaffProfile, ApiError> {
    let identity = request_to_identity(&req)?;
    let org_info = request_to_org_info(&req);

    let actor_ctx = actor_context_for(&actor.to_string(), organization_id);
    let ctx = decision_context_for(actor_ctx.clone());

    if let Some(existing) = find_by_owner(state, organization_id, &req.owner_id).await? {
        let mut updated = existing.clone();
        let identity_events = existing
            .decide(
                ProfileCommand::UpdateBasicIdentity {
                    identity: identity.clone(),
                },
                &ctx,
            )
            .map_err(domain_error)?;
        for e in &identity_events {
            updated.apply(e);
        }
        let org_events = updated
            .decide(
                ProfileCommand::UpdateOrganizationalInfo {
                    organizational_info: org_info,
                },
                &ctx,
            )
            .map_err(domain_error)?;
        for e in &org_events {
            updated.apply(e);
        }

        let aggregate_state =
            serde_json::to_value(&updated).map_err(|e| infrastructure_error(e.to_string()))?;
        let mut unit = state
            .unit_factory
            .create(platform_kernel::ObjectId(*organization_id.as_bytes()))
            .await
            .map_err(|e| infrastructure_error(e.to_string()))?;
        state
            .profile_repo
            .commit(aggregate_state, &[], &mut *unit)
            .await
            .map_err(|e| infrastructure_error(e.to_string()))?;
        unit.commit()
            .await
            .map_err(|e| infrastructure_error(e.to_string()))?;
        Ok(updated)
    } else {
        let owner_id_bytes = uuid::Uuid::parse_str(&req.owner_id)
            .map_err(|_| domain_error("owner_id is not a valid UUID"))?;
        let create_ctx = decision_context_for(actor_ctx);
        let events = StaffProfile::create(
            ProfileCommand::CreateProfile {
                owner_id: platform_kernel::ObjectId(*owner_id_bytes.as_bytes()),
                identity,
                organizational_info: org_info,
            },
            &create_ctx,
        )
        .map_err(domain_error)?;
        let profile = StaffProfile::from_created_event(&events[0]);
        let aggregate_state =
            serde_json::to_value(&profile).map_err(|e| infrastructure_error(e.to_string()))?;

        let mut unit = state
            .unit_factory
            .create(platform_kernel::ObjectId(*organization_id.as_bytes()))
            .await
            .map_err(|e| infrastructure_error(e.to_string()))?;
        state
            .profile_repo
            .commit(aggregate_state, &[], &mut *unit)
            .await
            .map_err(|e| infrastructure_error(e.to_string()))?;
        unit.commit()
            .await
            .map_err(|e| infrastructure_error(e.to_string()))?;
        Ok(profile)
    }
}

// ------------------------------------------------------------ Routes --

/// `GET /api/profiles/:owner_id` — public view (any authenticated org
/// member). Returns basic identity + organizational info always;
/// includes `work_stats` only if the caller is Top-level Manager or
/// Admin (checked, not merely omitted from the DTO by convention).
pub async fn get_profile(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(owner_id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    let organization_id = uuid::Uuid::parse_str(&auth.organization_id)
        .map_err(|_| domain_error("invalid organization id on token"))?;

    let profile = find_by_owner(&state, organization_id, &owner_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "PROFILE_NOT_FOUND",
                "VALIDATION",
                "NON_RETRYABLE",
                correlation(),
                json!({}),
            )
        })?;

    // Work-stats visibility check — does not block the request; it only
    // decides whether work_stats is included in the response. A viewer
    // who fails this check still gets the public sections.
    let can_see_stats = require_class(&state, &headers, &[UserClass::TopLevelManager])
        .await
        .is_ok();

    if can_see_stats {
        let dto = ProfileDtoWithStats {
            public: to_public_dto(&profile),
            work_stats: profile.work_stats().into(),
        };
        Ok(Json(dto).into_response())
    } else {
        Ok(Json(to_public_dto(&profile)).into_response())
    }
}

/// `GET /api/profiles` — lists every profile in the caller's
/// organization (public fields only, same visibility rule as
/// `get_profile` applied per-row).
pub async fn list_profiles(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PublicProfileDto>>, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    let organization_id = uuid::Uuid::parse_str(&auth.organization_id)
        .map_err(|_| domain_error("invalid organization id on token"))?;

    let rows = fetch_staff_profile_rows(&state, organization_id)
        .await
        .map_err(|e| infrastructure_error(e.to_string()))?;
    let profiles: Vec<PublicProfileDto> = rows
        .into_iter()
        .filter_map(|row| serde_json::from_value::<StaffProfile>(row).ok())
        .map(|p| to_public_dto(&p))
        .collect();
    Ok(Json(profiles))
}

/// `PUT /api/admin/profiles` — admin-only single upsert. Confirmed
/// Admin-only per the original product request ("access for
/// modifications for admin") — no exceptions, matching
/// `require_admin`'s use throughout `routes::admin`.
pub async fn upsert_profile_route(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<UpsertProfileRequest>,
) -> Result<Json<PublicProfileDto>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    let organization_id = uuid::Uuid::parse_str(&admin.organization_id)
        .map_err(|_| domain_error("invalid organization id"))?;
    let admin_uuid =
        uuid::Uuid::parse_str(&admin.user_id).map_err(|_| domain_error("invalid admin user id"))?;
    let profile = upsert_profile(
        &state,
        &platform_kernel::ObjectId(*admin_uuid.as_bytes()),
        organization_id,
        req,
    )
    .await?;
    Ok(Json(to_public_dto(&profile)))
}
