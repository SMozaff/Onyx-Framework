//! First-run bootstrap and admin-only user management.
//!
//! # Provenance
//! Audit finding **H-01**. Owner decision: user management is exposed through
//! admin-only API endpoints.
//!
//! # The bootstrap problem
//! Admin-only endpoints cannot authenticate against an empty `users` table —
//! there is no admin yet, so no one could ever create the first one. This
//! module resolves that with a **one-time bootstrap token**:
//!
//! * `POST /api/admin/bootstrap` is the only unauthenticated write endpoint.
//! * It is refused unless the store is **completely empty**. The moment the
//!   first user exists it is permanently closed, so it cannot be used to add a
//!   back-door admin to a live system.
//! * It requires `ONYX_BOOTSTRAP_TOKEN`, compared in constant time. If that
//!   variable is unset the endpoint is disabled outright rather than
//!   defaulting to open — this fails **closed**, unlike the `ONYX_TEST_MODE`
//!   pattern flagged as audit finding M-03.
//!
//! The result is that the token is useless the instant it is used once, and
//! useless on any system that already has users.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use security_adapter::constant_time_eq;
use security_application::{NewUser, UserClass, UserRecord, UserStoreError};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{authenticate_headers, ApiError, ApiState, ORGANIZATION_ID};

/// Header carrying the one-time bootstrap token.
pub const BOOTSTRAP_TOKEN_HEADER: &str = "x-onyx-bootstrap-token";
/// Environment variable holding the expected bootstrap token.
pub const BOOTSTRAP_TOKEN_ENV: &str = "ONYX_BOOTSTRAP_TOKEN";

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub is_admin: bool,
    /// Phase 1 (Desktop & Web Completion) addition — see
    /// `security_application::ports::user_store`'s `is_manager` doc note.
    /// **Deprecated** — see `class` below.
    #[serde(default)]
    pub is_manager: bool,
    /// Phase A (User Hierarchy) addition. Confirmed usable at creation
    /// time (design doc §5 question 2). Wire format matches
    /// `UserClass::as_str()`/`UserClass::parse()` — e.g.
    /// `"top_level_manager"`, `"team_leader"`.
    #[serde(default)]
    pub class: Option<String>,
    /// Phase A addition. The new user's parent (owning Manager) in the
    /// reporting-line tree, if known at creation time.
    #[serde(default)]
    pub parent_user_id: Option<String>,
    /// Defaults to the deployment's default tenant when omitted.
    #[serde(default)]
    pub organization_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetPasswordRequest {
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SetManagerRequest {
    pub is_manager: bool,
}

/// Phase A (User Hierarchy) addition. `class: None` clears the user's
/// class back to unclassified — this is a real, distinct action, not
/// merely "no class provided" (see `UserRecord::class`'s doc comment on
/// why `None` is meaningful), so the field is not `#[serde(default)]`:
/// the caller must say `"class": null` explicitly to clear it, rather
/// than omitting the field and having that silently mean the same
/// thing as clearing.
#[derive(Debug, Deserialize)]
pub struct SetClassRequest {
    pub class: Option<String>,
}

/// Phase A addition. Same "explicit null, not an omittable default"
/// reasoning as `SetClassRequest` — clearing a user's parent is a real
/// action (e.g. correcting an org chart), not indistinguishable from
/// "field not sent."
#[derive(Debug, Deserialize)]
pub struct SetParentRequest {
    pub parent_user_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserDto {
    pub id: String,
    pub username: String,
    pub organization_id: String,
    pub is_admin: bool,
    pub is_manager: bool,
    pub class: Option<String>,
    pub parent_user_id: Option<String>,
    pub is_active: bool,
}

impl From<UserRecord> for UserDto {
    fn from(user: UserRecord) -> Self {
        // Note the absence of `password_hash`: the hash must never cross the
        // API boundary, so the DTO simply has no field for it.
        Self {
            id: user.user_id,
            username: user.username,
            organization_id: user.organization_id,
            is_admin: user.is_admin,
            is_manager: user.is_manager,
            class: user.class.map(|c| c.as_str().to_string()),
            parent_user_id: user.parent_user_id,
            is_active: user.is_active,
        }
    }
}

fn correlation() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn store_error(error: UserStoreError) -> ApiError {
    match error {
        UserStoreError::DuplicateUsername => ApiError::new(
            StatusCode::CONFLICT,
            "USERNAME_TAKEN",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"That username already exists"}),
        ),
        UserStoreError::NotFound => ApiError::new(
            StatusCode::NOT_FOUND,
            "USER_NOT_FOUND",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({}),
        ),
        UserStoreError::ParentNotFound => ApiError::new(
            StatusCode::BAD_REQUEST,
            "PARENT_USER_NOT_FOUND",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"The specified parent user does not exist"}),
        ),
        UserStoreError::ParentCycle => ApiError::new(
            StatusCode::BAD_REQUEST,
            "PARENT_CYCLE",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"That assignment would create a cycle in the reporting line"}),
        ),
        UserStoreError::Infrastructure(message) => {
            tracing::error!(error = %message, "user store failure");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "USER_STORE_UNAVAILABLE",
                "INFRASTRUCTURE",
                "TRANSIENT",
                correlation(),
                json!({}),
            )
        }
    }
}

fn password_error(error: security_adapter::PasswordError) -> ApiError {
    match error {
        security_adapter::PasswordError::Policy(message) => ApiError::new(
            StatusCode::BAD_REQUEST,
            "WEAK_PASSWORD",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            // The policy text is safe to return: it describes the rule, not
            // anything about the submitted secret.
            json!({ "message": message }),
        ),
        other => {
            tracing::error!(error = %other, "password hashing failure");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PASSWORD_HASHING_FAILED",
                "INFRASTRUCTURE",
                "TRANSIENT",
                correlation(),
                json!({}),
            )
        }
    }
}

/// Authenticates the caller and confirms their `UserClass` is one of
/// `allowed` (Admin always passes regardless of `allowed`, matching
/// `require_manager_or_admin`'s "Admin is a practical superset"
/// rationale below). Phase A (User Hierarchy) addition — see
/// `IMPLEMENTATION_PLAN_User_Hierarchy.md` §2 A.5.
///
/// Deliberately takes an explicit allow-list rather than a single
/// minimum rank compared with `>=`: per `UserClass`'s own doc comment,
/// the confirmed hierarchy is not a strict linear order for permission
/// purposes (Team Leader's real-but-narrow authority, design doc §2.2,
/// does not sit at a single comparable point next to Senior Manager's
/// differently-shaped authority) — every call site should state exactly
/// which classes it means, not lean on an ordering that does not
/// actually hold for every rule.
///
/// First real caller (2026-08-13): `routes::profiles`'s work-stats
/// visibility gate (`[UserClass::TopLevelManager]` — see
/// `DECISIONS.md`'s "Staff Profiles" section for the confirmed
/// visibility rule). `#[allow(dead_code)]` removed accordingly.
pub(super) async fn require_class(
    state: &ApiState,
    headers: &HeaderMap,
    allowed: &[UserClass],
) -> Result<UserRecord, ApiError> {
    let auth = authenticate_headers(state, headers).await?;
    let user = state
        .user_store
        .find_by_id(&auth.user_id)
        .await
        .map_err(store_error)?
        .ok_or_else(|| ApiError::unauthorized(correlation()))?;
    let permitted = user.is_admin || user.class.is_some_and(|c| allowed.contains(&c));
    if !user.is_active || !permitted {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "CLASS_REQUIRED",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Your account's class does not permit this action"}),
        ));
    }
    Ok(user)
}

/// Authenticates the caller and confirms they are an admin.
///
/// The `is_admin` flag is re-read from the store rather than trusted from the
/// token: a token minted before an admin was demoted must not retain admin
/// power until it expires.
///
/// `pub(super)`: reused by `routes::profiles` for the batch import/export
/// and profile-edit routes, which are also confirmed Admin-only (staff
/// profile requirement, 2026-08-13 — "access for modifications for
/// admin"). Kept as one implementation rather than a second copy in
/// `profiles.rs`.
pub(super) async fn require_admin(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<UserRecord, ApiError> {
    let auth = authenticate_headers(state, headers).await?;
    let user = state
        .user_store
        .find_by_id(&auth.user_id)
        .await
        .map_err(store_error)?
        .ok_or_else(|| ApiError::unauthorized(correlation()))?;
    if !user.is_active || !user.is_admin {
        // 403, not 404: the caller is authenticated, just not permitted.
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "ADMIN_REQUIRED",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Administrator privileges are required"}),
        ));
    }
    Ok(user)
}

/// Authenticates the caller and confirms they are a Manager **or** an
/// Admin. Phase 1 addition, gating Policy/Settings administration
/// (`policy-domain`'s commands) — narrower than `require_admin`'s
/// user-management gate, per the Manager role's own scope note in
/// `security_application::ports::user_store`. Admin is accepted too
/// since Admin is a superset in practice for this workspace (every
/// deployment needs at least one account that can do everything), not
/// because Manager formally implies or is implied by Admin — the two
/// flags remain independent on `UserRecord`.
///
/// `#[allow(dead_code)]`: no caller exists yet. This guard is added now,
/// alongside the role itself, so the Policy/Settings routes (tracked
/// separately — see `PLAN_Desktop_Web_Completion.md`) have this in place
/// to call the moment they're written, rather than that work silently
/// reusing `require_admin` (too broad — see this fn's own doc comment)
/// or forgetting a guard entirely.
#[allow(dead_code)]
async fn require_manager_or_admin(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<UserRecord, ApiError> {
    let auth = authenticate_headers(state, headers).await?;
    let user = state
        .user_store
        .find_by_id(&auth.user_id)
        .await
        .map_err(store_error)?
        .ok_or_else(|| ApiError::unauthorized(correlation()))?;
    if !user.is_active || !(user.is_admin || user.is_manager) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "MANAGER_OR_ADMIN_REQUIRED",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Manager or administrator privileges are required"}),
        ));
    }
    Ok(user)
}

/// `POST /api/admin/bootstrap` — creates the first admin. See module docs.
pub async fn bootstrap(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    // 1. The endpoint is disabled unless a token is configured (fail closed).
    let expected = std::env::var(BOOTSTRAP_TOKEN_ENV)
        .ok()
        .filter(|token| !token.is_empty());
    let Some(expected) = expected else {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "BOOTSTRAP_DISABLED",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Bootstrap is not enabled"}),
        ));
    };

    // 2. Constant-time token comparison.
    let presented = headers
        .get(BOOTSTRAP_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !constant_time_eq(presented, &expected) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "BOOTSTRAP_TOKEN_INVALID",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation(),
            json!({}),
        ));
    }

    // 3. Refuse once ANY user exists. This is what makes the token one-time:
    //    a second call can never succeed, even with the correct token.
    if state.user_store.count().await.map_err(store_error)? > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "BOOTSTRAP_ALREADY_COMPLETED",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Bootstrap has already been completed"}),
        ));
    }

    let created = create_user_record(&state, payload, true).await?;
    tracing::warn!(
        user_id = %created.id,
        username = %created.username,
        "bootstrap admin created; ONYX_BOOTSTRAP_TOKEN should now be removed from the environment"
    );
    Ok((StatusCode::CREATED, Json(created)))
}

/// `POST /api/admin/users` — admin-only user creation.
pub async fn create_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    require_admin(&state, &headers).await?;
    let is_admin = payload.is_admin;
    let created = create_user_record(&state, payload, is_admin).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Shared creation path for bootstrap and admin creation, so password policy,
/// hashing and duplicate handling cannot diverge between the two.
async fn create_user_record(
    state: &ApiState,
    payload: CreateUserRequest,
    is_admin: bool,
) -> Result<UserDto, ApiError> {
    let username = payload.username.trim().to_string();
    if username.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_USERNAME",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"Username must not be empty"}),
        ));
    }
    let password_hash = state
        .password_hasher
        .hash(&payload.password)
        .map_err(password_error)?;
    let organization_id = payload
        .organization_id
        .unwrap_or_else(|| ORGANIZATION_ID.to_string());

    // is_manager always comes from the payload, even for bootstrap — unlike
    // is_admin (forced true for bootstrap so the very first account can
    // administer the system), there is no equivalent reason to force the
    // narrower Manager role on for the bootstrap account.
    let is_manager = payload.is_manager;
    let class = parse_class_field(payload.class.as_deref())?;

    let record = state
        .user_store
        .create(NewUser {
            user_id: uuid::Uuid::new_v4().to_string(),
            username,
            organization_id,
            password_hash,
            is_admin,
            is_manager,
            class,
            parent_user_id: payload.parent_user_id,
        })
        .await
        .map_err(store_error)?;
    Ok(record.into())
}

/// Parses a `class` field's wire-format string into a [`UserClass`],
/// returning a 400 `ApiError` for an unrecognized value rather than
/// silently treating it as `None` — a caller who mistypes
/// `"team-leader"` for `"team_leader"` should get a clear rejection,
/// not a request that silently succeeds with no class assigned.
fn parse_class_field(raw: Option<&str>) -> Result<Option<UserClass>, ApiError> {
    match raw {
        None => Ok(None),
        Some(s) => UserClass::parse(s).map(Some).ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_CLASS",
                "VALIDATION",
                "NON_RETRYABLE",
                correlation(),
                json!({"message": format!("Unrecognized class: {s}")}),
            )
        }),
    }
}

/// `POST /api/admin/users/:id/manager` — admin-only Manager-role grant/revoke.
/// Deliberately admin-only (not manager-or-admin): a Manager must not be
/// able to grant itself or others additional Manager scope, mirroring how
/// `is_admin` itself can only be set by an existing admin
/// (`create_user`/`bootstrap`).
///
/// **Deprecated** — see `set_class` below. Kept operational during the
/// migration window (see `security_application`'s `is_manager` doc
/// note); not yet removed since the backfill decision for existing
/// `is_manager = true` users has not been made
/// (`IMPLEMENTATION_PLAN_User_Hierarchy.md` §2 A.3).
pub async fn set_manager(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(payload): Json<SetManagerRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers).await?;
    state
        .user_store
        .set_manager(&user_id, payload.is_manager)
        .await
        .map_err(store_error)?;
    Ok(Json(json!({"success": true})))
}

/// `POST /api/admin/users/:id/class` — admin-only class assignment.
/// Admin-only per design doc §1: "no one else could add or remove users
/// or change user's class and types." Supersedes `set_manager` for new
/// integrations — see that function's doc comment.
pub async fn set_class(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(payload): Json<SetClassRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers).await?;
    let class = parse_class_field(payload.class.as_deref())?;
    state
        .user_store
        .set_class(&user_id, class)
        .await
        .map_err(store_error)?;
    Ok(Json(json!({"success": true})))
}

#[derive(serde::Serialize)]
pub struct MobileAccessResponse {
    pub allowed_classes: Vec<String>,
}

#[derive(serde::Deserialize)]
pub struct SetMobileAccessRequest {
    pub allowed_classes: Vec<String>,
}

/// `GET /api/admin/mobile-access` — admin-only. Returns the caller's own
/// organization's current mobile-access grants (which `UserClass`
/// values, as wire strings, may currently log in with
/// `client_type: "mobile"`). Scoped to the admin's own organization —
/// there is no cross-org admin concept anywhere else in this file
/// either, so this follows that same precedent rather than accepting an
/// organization_id from the caller.
pub async fn get_mobile_access(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<MobileAccessResponse>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    let allowed_classes = state
        .user_store
        .list_mobile_access(&admin.organization_id)
        .await
        .map_err(store_error)?;
    Ok(Json(MobileAccessResponse { allowed_classes }))
}

/// `PUT /api/admin/mobile-access` — admin-only. Replaces the caller's
/// organization's full mobile-access grant list with exactly the
/// `allowed_classes` given (validated against `UserClass::parse` so an
/// unrecognized class string is rejected as `INVALID_CLASS` rather than
/// silently stored). An empty list is valid and means "no class may log
/// in from mobile" — the restrictive default this table exists to
/// implement, not an error.
pub async fn set_mobile_access(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<SetMobileAccessRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    for class in &payload.allowed_classes {
        parse_class_field(Some(class.as_str()))?;
    }
    state
        .user_store
        .set_mobile_access(&admin.organization_id, &payload.allowed_classes)
        .await
        .map_err(store_error)?;
    Ok(Json(json!({"success": true})))
}

/// `POST /api/admin/users/:id/parent` — admin-only reporting-line
/// assignment. Admin-only for the same reason as `set_class` above —
/// the reporting line determines who verifies whose Todo/Target lists
/// (design doc §4) and who a staff loan's "real owner" is (design doc
/// §2.1), so it carries the same weight as class assignment itself.
pub async fn set_parent(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(payload): Json<SetParentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers).await?;
    state
        .user_store
        .set_parent(&user_id, payload.parent_user_id.as_deref())
        .await
        .map_err(store_error)?;
    Ok(Json(json!({"success": true})))
}

/// Reduced identity shape used by non-admin assignment and staff-loan
/// pickers. Administrative privilege, class, reporting-line, and account
/// activation details are deliberately absent from this wire contract.
#[derive(Debug, Serialize)]
pub struct PickerUserDto {
    pub id: String,
    pub username: String,
}

/// `GET /api/users` — authenticated, same-organization active-user list.
///
/// This is deliberately distinct from `/api/admin/users`: ordinary Staff and
/// Managers need enough identity information to name a colleague in an
/// assignment or staff-loan request, but they do not need administrative
/// attributes. Filtering to the bearer token's organization prevents this
/// route from becoming a cross-tenant directory, and inactive accounts are
/// omitted because they cannot participate in new work.
pub async fn list_picker_users(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PickerUserDto>>, ApiError> {
    let authenticated = authenticate_headers(&state, &headers).await?;
    let users = state.user_store.list().await.map_err(store_error)?;
    let visible = users
        .into_iter()
        .filter(|user| user.organization_id == authenticated.organization_id && user.is_active)
        .map(|user| PickerUserDto {
            id: user.user_id,
            username: user.username,
        })
        .collect();
    Ok(Json(visible))
}

/// Identity shape for `desktop-shell`'s local Task/Mission approval-
/// authority cache — see `list_hierarchy_users` below. Includes exactly
/// the fields `verifier_resolution`-style tree-parent authority needs
/// (`id`, `parent_user_id`) plus `is_admin` (an Admin may always
/// approve, mirroring `require_admin`'s own "admin bypasses the
/// narrower check" pattern used throughout `routes::admin`). Everything
/// more sensitive than this (password hash — never serialized anywhere;
/// `class`; account activation state) is intentionally omitted, same
/// restraint as `PickerUserDto` above, since this is not an admin-only
/// route.
#[derive(Debug, Serialize)]
pub struct HierarchyUserDto {
    pub id: String,
    pub parent_user_id: Option<String>,
    pub is_admin: bool,
}

/// `GET /api/users/hierarchy` — authenticated, same-organization,
/// `{id, parent_user_id, is_admin}` only.
///
/// Built specifically so `desktop-shell` (which has no local
/// `UserStore` — its embedded `AppState` composes only the domain
/// aggregates it needs offline, and the org's account/reporting-line
/// directory is deliberately not one of them, see
/// `client-composition::AppState`'s own doc comment) can fetch and
/// cache the organization's reporting-line tree once at login, then
/// resolve "is the current user this task's owner's manager?" purely
/// from that local cache when approving/rejecting a `Task`/`Mission` —
/// see `TaskDecisionHandler`'s doc comment for the full authority-gap
/// history this closes. Deliberately not admin-gated (`require_admin`
/// would be wrong here: an ordinary Manager approving their own
/// report's work is not an administrative action), but deliberately
/// minimal in fields returned for the same reason `PickerUserDto` is —
/// this is a directory lookup any authenticated org member can make,
/// not a privilege-escalation surface.
pub async fn list_hierarchy_users(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<HierarchyUserDto>>, ApiError> {
    let authenticated = authenticate_headers(&state, &headers).await?;
    let users = state.user_store.list().await.map_err(store_error)?;
    let visible = users
        .into_iter()
        .filter(|user| user.organization_id == authenticated.organization_id && user.is_active)
        .map(|user| HierarchyUserDto {
            id: user.user_id,
            parent_user_id: user.parent_user_id,
            is_admin: user.is_admin,
        })
        .collect();
    Ok(Json(visible))
}

/// `GET /api/admin/users` — admin-only listing.
pub async fn list_users(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserDto>>, ApiError> {
    require_admin(&state, &headers).await?;
    let users = state.user_store.list().await.map_err(store_error)?;
    Ok(Json(users.into_iter().map(UserDto::from).collect()))
}

/// `POST /api/admin/users/:id/deactivate`
pub async fn deactivate_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    // Guard against an admin locking the system out of itself.
    if admin.user_id == user_id {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "CANNOT_DEACTIVATE_SELF",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation(),
            json!({"message":"An administrator cannot deactivate their own account"}),
        ));
    }
    state
        .user_store
        .set_active(&user_id, false)
        .await
        .map_err(store_error)?;
    Ok(Json(json!({"success": true})))
}

/// `POST /api/admin/users/:id/activate`
pub async fn activate_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers).await?;
    state
        .user_store
        .set_active(&user_id, true)
        .await
        .map_err(store_error)?;
    Ok(Json(json!({"success": true})))
}

/// `POST /api/admin/users/:id/password` — admin-driven password reset.
pub async fn set_user_password(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(payload): Json<SetPasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&state, &headers).await?;
    let hash = state
        .password_hasher
        .hash(&payload.password)
        .map_err(password_error)?;
    state
        .user_store
        .set_password_hash(&user_id, &hash)
        .await
        .map_err(store_error)?;
    // NOTE: existing tokens for this user remain valid until they expire.
    // Durable, cross-replica revocation is audit finding H-02 and is tracked
    // separately; the in-memory revocation set cannot express "revoke all
    // tokens for a user" across pods.
    Ok(Json(json!({"success": true})))
}
