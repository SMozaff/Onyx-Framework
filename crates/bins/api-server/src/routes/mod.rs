//! Team 6 HTTP/WebSocket integration surface, hardened by Team 7.
//! Binding rulings are recorded in the workspace DECISIONS.md.

pub mod admin;
pub mod auth;
pub mod command;
pub mod events;
pub mod policy_admin;
pub mod profiles;
pub mod query;
pub mod relay;
pub mod todo_admin;

use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use audit_application::AuditWriter;
use axum::{
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use observability_adapter::{HashChainAuditWriter, Metrics};
use persistence_postgres::{PostgresRepository, PostgresUnitOfWorkFactory};
use persistence_sqlite::{SqliteRepository, SqliteUnitOfWorkFactory};
use platform_kernel::{ObjectId, OrganizationId};
use query_application::{IdempotencyError, IdempotencyStore, Repository, UnitOfWorkFactory};
use security_adapter::{
    Ed25519JwtCodec, EnvironmentSecretProvider, InMemorySlidingWindowRateLimiter, PasswordHasher,
    PostgresSlidingWindowRateLimiter, PostgresUserStore, SqliteUserStore,
};
use security_application::{RateLimiter, SecretProvider, UserStore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{
    postgres::PgPoolOptions,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    PgPool, SqlitePool,
};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::{Any, CorsLayer};

use crate::query_handler::ProjectionPool;

pub const ORGANIZATION_ID: &str = "11111111-1111-1111-1111-111111111111";
pub const USER_ID: &str = "22222222-2222-2222-2222-222222222222";
pub const WEB_DEVICE_ID: &str = "web-client";
// NOTE (audit finding H-01): `DEFAULT_USERNAME` / `DEFAULT_PASSWORD` were
// removed. Credentials are now stored as Argon2id hashes in the `users` table
// and verified via `ApiState::user_store`. `ORGANIZATION_ID` and `USER_ID`
// above are retained ONLY as the default tenant/actor used by the bootstrap
// admin and by fixtures; they are no longer authentication material.

#[derive(Clone)]
pub struct ApiState {
    pub projection_pool: ProjectionPool,
    pub notification_repo: Arc<dyn Repository>,
    pub approval_repo: Arc<dyn Repository>,
    pub unit_factory: Arc<dyn UnitOfWorkFactory>,
    pub idempotency_store: Arc<dyn IdempotencyStore>,
    pub events: broadcast::Sender<Value>,
    pub revoked_tokens: Arc<RwLock<HashSet<String>>>,
    pub secret_provider: Arc<dyn SecretProvider>,
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub audit_writer: Arc<dyn AuditWriter>,
    pub metrics: Metrics,
    /// Identity store backing `/api/auth/login` (audit finding H-01).
    /// Replaces the former `DEFAULT_USERNAME`/`DEFAULT_PASSWORD` constants.
    pub user_store: Arc<dyn UserStore>,
    /// Repository for `profile_domain::StaffProfile`. Backs the staff
    /// profile view/edit routes and the batch import/export feature —
    /// see `routes::profiles`.
    pub profile_repo: Arc<dyn Repository>,
    /// Repository for `policy_domain::Policy`. Added 2026-08-14 so the
    /// Admin platform (`admin-shell`, a thin HTTP client) can
    /// administer Policy over `/api/command` — `client-composition`'s
    /// registry, which desktop-shell uses, is a separate wiring path
    /// that this crate does not share.
    pub policy_repo: Arc<dyn Repository>,
    /// Repository for `policy_domain::LegalHold`. See `policy_repo`.
    pub legal_hold_repo: Arc<dyn Repository>,
    /// Repository for `todo_domain::TodoList`. Added 2026-08-16 —
    /// `client-composition` has no equivalent wiring for `todo-domain`
    /// yet, so (mirroring `policy_repo`'s own precedent) this crate's
    /// `/api/command` dispatch is these aggregates' first real
    /// integration point. See `routes::command`'s `todo_list.*` dispatch
    /// arms and `routes::todo_admin` for the `CreateTodoList` route.
    pub todo_list_repo: Arc<dyn Repository>,
    /// Repository for `todo_domain::TargetList`. See `todo_list_repo`.
    pub target_list_repo: Arc<dyn Repository>,
    /// Repository for `todo_domain::StaffLoan`. See `todo_list_repo`.
    pub staff_loan_repo: Arc<dyn Repository>,
    /// Argon2id hasher. Held on state rather than constructed per request:
    /// building it derives a dummy hash, which is deliberately expensive.
    pub password_hasher: Arc<PasswordHasher>,
    /// Live Cloud Relay connections, keyed by replica. Empty until a replica
    /// dials `/api/relay/:target`; see `routes::relay` for why presence is
    /// per-instance and what that means for horizontal scaling.
    pub relay_registry: relay::RelayRegistry,
}

/// The storage-backend-specific handles `ApiState::new` assembles, before
/// they're destructured into named fields. One tuple, constructed once per
/// process start (Postgres branch or SQLite branch), never passed further —
/// named here only to satisfy `clippy::type_complexity`, not because it's
/// reused elsewhere.
type StorageBackendHandles = (
    ProjectionPool,
    Arc<dyn Repository>,
    Arc<dyn Repository>,
    Arc<dyn Repository>,
    Arc<dyn Repository>,
    Arc<dyn Repository>,
    Arc<dyn Repository>,
    Arc<dyn Repository>,
    Arc<dyn Repository>,
    Arc<dyn UnitOfWorkFactory>,
    Arc<dyn IdempotencyStore>,
    Option<SqlitePool>,
    Option<PgPool>,
);

impl ApiState {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let environment = std::env::var("ONYX_ENV").unwrap_or_else(|_| "development".to_string());
        let postgres_primary =
            database_url.starts_with("postgres://") || database_url.starts_with("postgresql://");
        if environment == "production" && !postgres_primary {
            anyhow::bail!(
                "production API storage must use PostgreSQL; per-instance SQLite cannot back a scaled deployment"
            );
        }

        let (
            projection_pool,
            notification_repo,
            approval_repo,
            profile_repo,
            policy_repo,
            legal_hold_repo,
            todo_list_repo,
            target_list_repo,
            staff_loan_repo,
            unit_factory,
            idempotency_store,
            sqlite_pool,
            primary_postgres_pool,
        ): StorageBackendHandles = if postgres_primary {
            let pool = PgPoolOptions::new()
                .max_connections(20)
                .connect(database_url)
                .await?;
            sqlx::migrate!("../../../migrations/postgres")
                .run(&pool)
                .await?;
            (
                ProjectionPool::Postgres(pool.clone()),
                Arc::new(PostgresRepository::new(pool.clone(), "notification")),
                Arc::new(PostgresRepository::new(pool.clone(), "approval")),
                Arc::new(PostgresRepository::new(pool.clone(), "staff_profile")),
                Arc::new(PostgresRepository::new(pool.clone(), "policy")),
                Arc::new(PostgresRepository::new(pool.clone(), "legal_hold")),
                Arc::new(PostgresRepository::new(pool.clone(), "todo_list")),
                Arc::new(PostgresRepository::new(pool.clone(), "target_list")),
                Arc::new(PostgresRepository::new(pool.clone(), "staff_loan")),
                Arc::new(PostgresUnitOfWorkFactory::new(pool.clone())),
                Arc::new(PostgresIdempotencyStore::new(pool.clone())),
                None,
                Some(pool),
            )
        } else {
            let options = SqliteConnectOptions::from_str(database_url)?
                .create_if_missing(true)
                // Phase A fix (User Hierarchy): without this, SQLite
                // never enforces the FK on users.parent_user_id (or any
                // other FK in this schema) — confirmed by grep before
                // this fix that no connection anywhere set this pragma,
                // documented as a real gap in DECISIONS.md. Per-
                // connection, not database-wide, so it must be set here
                // on every connection the pool opens, not as a one-time
                // PRAGMA query against a single connection that would
                // be lost once the pool cycles connections.
                .foreign_keys(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(if database_url.contains(":memory:") {
                    1
                } else {
                    5
                })
                .connect_with(options)
                .await?;
            sqlx::migrate!("../../../migrations/sqlite")
                .run(&pool)
                .await?;
            seed_if_empty(&pool).await?;
            (
                ProjectionPool::Sqlite(pool.clone()),
                Arc::new(SqliteRepository::new(pool.clone(), "notification")),
                Arc::new(SqliteRepository::new(pool.clone(), "approval")),
                Arc::new(SqliteRepository::new(pool.clone(), "staff_profile")),
                Arc::new(SqliteRepository::new(pool.clone(), "policy")),
                Arc::new(SqliteRepository::new(pool.clone(), "legal_hold")),
                Arc::new(SqliteRepository::new(pool.clone(), "todo_list")),
                Arc::new(SqliteRepository::new(pool.clone(), "target_list")),
                Arc::new(SqliteRepository::new(pool.clone(), "staff_loan")),
                Arc::new(SqliteUnitOfWorkFactory::new(pool.clone())),
                Arc::new(SqliteIdempotencyStore::new(pool.clone())),
                Some(pool),
                None,
            )
        };
        let (events, _) = broadcast::channel(512);

        if std::env::var("ONYX_AUTHORITY_SIGNING_KEY").is_err() {
            if environment == "production" {
                anyhow::bail!("ONYX_AUTHORITY_SIGNING_KEY is required in production");
            }
            std::env::set_var(
                "ONYX_AUTHORITY_SIGNING_KEY",
                "hex:4242424242424242424242424242424242424242424242424242424242424242",
            );
        }
        let secret_provider: Arc<dyn SecretProvider> = Arc::new(EnvironmentSecretProvider);
        let initial_secret = secret_provider.get("ONYX_AUTHORITY_SIGNING_KEY").await?;
        Ed25519JwtCodec::from_rotating_secret(&initial_secret)?;

        // Identity store follows the primary database (audit finding H-01).
        // Both backends are supported per the audit decision log; the `users`
        // table ships in both migration sets and is applied above.
        let user_store: Arc<dyn UserStore> = if let Some(pool) = primary_postgres_pool.clone() {
            Arc::new(PostgresUserStore::new(pool))
        } else {
            Arc::new(SqliteUserStore::new(
                sqlite_pool
                    .clone()
                    .expect("one of the primary pools must exist"),
            ))
        };

        let governance_url = std::env::var("ONYX_GOVERNANCE_DATABASE_URL").ok();
        if environment == "production" && governance_url.is_none() {
            anyhow::bail!("ONYX_GOVERNANCE_DATABASE_URL is required in production");
        }
        let (rate_limiter, audit_writer): (Arc<dyn RateLimiter>, Arc<dyn AuditWriter>) =
            if let Some(url) = governance_url {
                let governance_pool = PgPoolOptions::new()
                    .max_connections(10)
                    .connect(&url)
                    .await?;
                sqlx::migrate!("../../../migrations/postgres")
                    .run(&governance_pool)
                    .await?;
                (
                    Arc::new(PostgresSlidingWindowRateLimiter::new(
                        governance_pool.clone(),
                    )),
                    Arc::new(HashChainAuditWriter::postgres(governance_pool)),
                )
            } else if let Some(pool) = primary_postgres_pool {
                (
                    Arc::new(PostgresSlidingWindowRateLimiter::new(pool.clone())),
                    Arc::new(HashChainAuditWriter::postgres(pool)),
                )
            } else {
                let pool = sqlite_pool.expect("SQLite composition must retain its pool");
                (
                    Arc::new(InMemorySlidingWindowRateLimiter::default()),
                    Arc::new(HashChainAuditWriter::sqlite(pool)),
                )
            };

        Ok(Self {
            projection_pool,
            notification_repo,
            approval_repo,
            profile_repo,
            policy_repo,
            legal_hold_repo,
            todo_list_repo,
            target_list_repo,
            staff_loan_repo,
            unit_factory,
            idempotency_store,
            events,
            revoked_tokens: Arc::new(RwLock::new(HashSet::new())),
            secret_provider,
            rate_limiter,
            audit_writer,
            metrics: Metrics::new("onyx_api_server")?,
            user_store,
            password_hasher: Arc::new(PasswordHasher::new()),
            relay_registry: relay::RelayRegistry::new(),
        })
    }

    pub async fn is_ready(&self) -> bool {
        match &self.projection_pool {
            ProjectionPool::Sqlite(pool) => sqlx::query_scalar::<_, i64>("SELECT 1")
                .fetch_one(pool)
                .await
                .is_ok(),
            ProjectionPool::Postgres(pool) => sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(pool)
                .await
                .is_ok(),
        }
    }
}

async fn readiness(State(state): State<ApiState>) -> StatusCode {
    if state.is_ready().await {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

pub fn router(state: ApiState) -> Router {
    let command_route = post(command::command_route).route_layer(middleware::from_fn_with_state(
        state.clone(),
        crate::middleware::rate_limit::rate_limit_command,
    ));
    Router::new()
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/ready", get(readiness))
        .route("/api/auth/login", post(auth::login))
        // Bootstrap is intentionally unauthenticated; it self-closes once any
        // user exists and requires ONYX_BOOTSTRAP_TOKEN. See routes::admin.
        .route("/api/admin/bootstrap", post(admin::bootstrap))
        .route(
            "/api/admin/users",
            post(admin::create_user).get(admin::list_users),
        )
        .route(
            "/api/admin/users/:id/deactivate",
            post(admin::deactivate_user),
        )
        .route("/api/admin/users/:id/activate", post(admin::activate_user))
        .route(
            "/api/admin/users/:id/password",
            post(admin::set_user_password),
        )
        // Phase 1 (Desktop & Web Completion): admin-only Manager-role
        // grant/revoke. See admin::set_manager's doc comment for why this
        // stays admin-only rather than manager-or-admin.
        .route("/api/admin/users/:id/manager", post(admin::set_manager))
        // Phase A (User Hierarchy): admin-only class + reporting-line
        // assignment. Supersedes /manager above — see admin::set_class's
        // and admin::set_parent's doc comments.
        .route("/api/admin/users/:id/class", post(admin::set_class))
        .route("/api/admin/users/:id/parent", post(admin::set_parent))
        // Staff profiles (2026-08-13): public view/list, admin-only
        // upsert and batch import/export. See routes::profiles for the
        // confirmed visibility/editing rules.
        .route("/api/profiles", get(profiles::list_profiles))
        .route("/api/profiles/:owner_id", get(profiles::get_profile))
        .route(
            "/api/admin/profiles",
            put(profiles::upsert_profile_route),
        )
        .route(
            "/api/admin/profiles/import",
            post(profiles::batch::import_profiles),
        )
        .route(
            "/api/admin/profiles/export",
            get(profiles::batch::export_profiles),
        )
        // Policy/LegalHold creation (2026-08-14) — see
        // routes::policy_admin's module doc comment for why these need
        // their own routes rather than going through /api/command.
        .route("/api/admin/policies", post(policy_admin::create_policy))
        .route(
            "/api/admin/legal-holds",
            post(policy_admin::apply_legal_hold),
        )
        // TodoList/TargetList/StaffLoan creation (2026-08-16) — see
        // routes::todo_admin's module doc comment for why these need
        // their own routes (create()-routed commands) and why they are
        // not admin-gated the way the Policy/LegalHold routes above
        // are.
        .route("/api/todo/lists", post(todo_admin::create_todo_list))
        .route("/api/todo/targets", post(todo_admin::create_target_list))
        .route(
            "/api/todo/staff-loans",
            post(todo_admin::request_staff_loan),
        )
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/command", command_route)
        .route("/api/query", get(query::query_route))
        .route("/api/events", get(events::websocket_route))
        // Cloud Relay (Part II §8.2). The path segment is the replica being
        // dialled, matching the URL CloudRelayTransport::connect builds.
        .route("/api/relay/:target_id", get(relay::relay_route))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::rate_limit::observe_request,
        ))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS]),
        )
        .with_state(state)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainObjectRefDto {
    pub id: String,
    #[serde(rename = "type")]
    pub object_type: String,
    pub organization_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorContextDto {
    pub user_id: String,
    pub device_id: String,
    pub organization_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityScopeDto {
    pub organization_id: String,
    pub object_type: String,
    pub object_id: Option<String>,
    pub command_types: Vec<String>,
    pub delegation_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityProofDto {
    pub proof_type: String,
    pub scope: AuthorityScopeDto,
    pub issued_at: String,
    pub expires_at: String,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VectorClockDto {
    #[serde(default)]
    pub entries: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub command_id: String,
    pub operation_id: String,
    pub command_type: String,
    pub schema_version: String,
    pub target: DomainObjectRefDto,
    pub expected_version: u64,
    pub expected_lifecycle_epoch: u64,
    pub expected_authority_epoch: u64,
    pub issued_at: String,
    #[serde(default)]
    pub vector_clock: VectorClockDto,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub payload: Value,
    // R4: accepted for schema compatibility but ignored. Authority is the
    // bearer access_token JWT and is extracted server-side.
    #[serde(default)]
    pub actor: Option<ActorContextDto>,
    #[serde(default)]
    pub authority_proof: Option<AuthorityProofDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEnvelopeDto {
    pub query_id: String,
    pub query_type: String,
    pub schema_version: String,
    pub organization_id: String,
    #[serde(default)]
    pub actor: Option<ActorContextDto>,
    #[serde(default)]
    pub authority_proof: Option<AuthorityProofDto>,
    #[serde(default)]
    pub filters: HashMap<String, Value>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessDto {
    pub projection_version: u64,
    pub last_updated_at: String,
    pub is_stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub category: String,
    pub retryability: String,
    pub safe_details: Value,
    pub correlation_id: String,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub body: ApiErrorBody,
}

impl ApiError {
    pub fn new(
        status: StatusCode,
        code: impl Into<String>,
        category: impl Into<String>,
        retryability: impl Into<String>,
        correlation_id: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            status,
            body: ApiErrorBody {
                code: code.into(),
                category: category.into(),
                retryability: retryability.into(),
                safe_details: details,
                correlation_id: correlation_id.into(),
            },
        }
    }

    pub fn unauthorized(correlation_id: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "AUTHORITY",
            "NON_RETRYABLE",
            correlation_id,
            json!({"message":"Authentication required"}),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({"error": self.body}))).into_response()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenScope {
    pub object_type: String,
    pub object_id: Option<String>,
    pub command_types: Vec<String>,
    pub delegation_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub username: String,
    pub organization_id: String,
    pub token_type: String,
    pub scope: TokenScope,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub username: String,
    pub organization_id: String,
    pub token: String,
    pub scope: TokenScope,
}

pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Mints a signed token for an **already-authenticated** principal.
///
/// Audit finding H-01: this previously hardcoded `sub`, `username` and
/// `organization_id`, so every token identified the same synthetic user. It
/// now takes the resolved [`UserRecord`], so claims reflect the real identity.
///
/// The `scope` remains uniform by explicit decision — this change is
/// authentication only, and does not introduce per-user authorization.
pub async fn issue_token(
    state: &ApiState,
    user: &security_application::UserRecord,
    token_type: &str,
    ttl_seconds: u64,
) -> anyhow::Result<String> {
    let now = unix_seconds();
    let claims = TokenClaims {
        sub: user.user_id.clone(),
        username: user.username.clone(),
        organization_id: user.organization_id.clone(),
        token_type: token_type.to_string(),
        scope: TokenScope {
            object_type: "*".to_string(),
            object_id: None,
            command_types: vec![
                "notification.Acknowledge".to_string(),
                "approval.Approve".to_string(),
                "approval.Reject".to_string(),
                // Policy/LegalHold (2026-08-14, Admin Platform) — added
                // alongside routes::command's matching dispatch arms.
                // Found and fixed via a real end-to-end test failure
                // (403 COMMAND_NOT_AUTHORIZED), not assumed to be
                // covered by the routing change alone — this token
                // scope allowlist is a second, separate gate the
                // envelope has to pass before command.rs's own
                // command_type match is ever reached.
                "policy.CreatePolicyVersion".to_string(),
                "policy.PublishPolicyVersion".to_string(),
                "policy.EvaluatePolicy".to_string(),
                "policy.RegisterViolation".to_string(),
                "policy.RetirePolicy".to_string(),
                "legal_hold.ReleaseLegalHold".to_string(),
                // TodoList/TargetList/StaffLoan (2026-08-16). Same
                // second-gate requirement as Policy/LegalHold above —
                // confirmed by an actual 403 COMMAND_NOT_AUTHORIZED
                // during end-to-end smoke testing, not assumed.
                "todo_list.AddItem".to_string(),
                "todo_list.SubmitTodoList".to_string(),
                "todo_list.RecordTeamLeaderPreCheck".to_string(),
                "todo_list.VerifyTodoList".to_string(),
                "todo_list.RejectTodoList".to_string(),
                "todo_list.EscalateTodoList".to_string(),
                "target_list.SubmitTargetList".to_string(),
                "target_list.RecordTeamLeaderPreCheck".to_string(),
                "target_list.VerifyTargetList".to_string(),
                "target_list.RejectTargetList".to_string(),
                "target_list.EscalateTargetList".to_string(),
                "staff_loan.ApproveStaffLoan".to_string(),
                "staff_loan.DeclineStaffLoan".to_string(),
                "staff_loan.ExtendStaffLoan".to_string(),
                "staff_loan.EndStaffLoanEarly".to_string(),
                "staff_loan.ExpireStaffLoan".to_string(),
            ],
            delegation_depth: 0,
        },
        iat: now,
        exp: now + ttl_seconds,
        jti: uuid::Uuid::new_v4().to_string(),
    };
    let secret = state
        .secret_provider
        .get("ONYX_AUTHORITY_SIGNING_KEY")
        .await?;
    Ed25519JwtCodec::from_rotating_secret(&secret)?
        .encode(&claims)
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

pub async fn validate_token(
    state: &ApiState,
    token: &str,
    expected_type: &str,
) -> Result<TokenClaims, ApiError> {
    if state.revoked_tokens.read().await.contains(token) {
        return Err(ApiError::unauthorized(uuid::Uuid::new_v4().to_string()));
    }
    let secret = state
        .secret_provider
        .get("ONYX_AUTHORITY_SIGNING_KEY")
        .await
        .map_err(|_| ApiError::unauthorized(uuid::Uuid::new_v4().to_string()))?;
    let codec = Ed25519JwtCodec::from_rotating_secret(&secret)
        .map_err(|_| ApiError::unauthorized(uuid::Uuid::new_v4().to_string()))?;
    let claims: TokenClaims = codec
        .decode(token)
        .map_err(|_| ApiError::unauthorized(uuid::Uuid::new_v4().to_string()))?;
    if claims.exp <= unix_seconds()
        || claims.iat > unix_seconds()
        || claims.token_type != expected_type
    {
        return Err(ApiError::unauthorized(uuid::Uuid::new_v4().to_string()));
    }
    Ok(claims)
}

pub fn test_mode_error(headers: &HeaderMap, correlation_id: &str) -> Option<ApiError> {
    if std::env::var("ONYX_TEST_MODE").ok().as_deref() != Some("1") {
        return None;
    }
    let status = headers.get("x-onyx-test-status")?.to_str().ok()?;
    match status {
        "429" => Some(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "AUTHORITY",
            "TRANSIENT",
            correlation_id,
            json!({"message":"Test quota reached","reset_at":"2026-08-05T13:00:00Z"}),
        )),
        "500" => Some(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "TEST_SERVER_ERROR",
            "INFRASTRUCTURE",
            "TRANSIENT",
            correlation_id,
            json!({"message":"Deterministic Team 6 test failure"}),
        )),
        _ => None,
    }
}

pub async fn authenticate_headers(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<AuthenticatedUser, ApiError> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized(uuid::Uuid::new_v4().to_string()))?;
    let claims = validate_token(state, token, "access").await?;
    Ok(AuthenticatedUser {
        user_id: claims.sub,
        username: claims.username,
        organization_id: claims.organization_id,
        token: token.to_string(),
        scope: claims.scope,
    })
}

pub fn parse_object_id(value: &str) -> Result<ObjectId, ApiError> {
    uuid::Uuid::parse_str(value)
        .map(|u| ObjectId(*u.as_bytes()))
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_UUID",
                "DOMAIN",
                "NON_RETRYABLE",
                uuid::Uuid::new_v4().to_string(),
                json!({"value": value}),
            )
        })
}

pub fn object_id_to_string(id: ObjectId) -> String {
    uuid::Uuid::from_bytes(id.0).to_string()
}

pub fn organization_id() -> OrganizationId {
    parse_object_id(ORGANIZATION_ID).expect("constant organization UUID is valid")
}

pub fn web_device_object_id() -> ObjectId {
    let digest = Sha256::digest(WEB_DEVICE_ID.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    ObjectId(bytes)
}

#[derive(Clone)]
struct PostgresIdempotencyStore {
    pool: PgPool,
}

impl PostgresIdempotencyStore {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IdempotencyStore for PostgresIdempotencyStore {
    async fn get(
        &self,
        operation_id: &platform_kernel::OperationId,
    ) -> Result<Option<Value>, IdempotencyError> {
        let row: Option<Value> =
            sqlx::query_scalar("SELECT result FROM idempotency WHERE operation_id = $1")
                .bind(uuid::Uuid::from_bytes(operation_id.0))
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| IdempotencyError::Database(error.to_string()))?;
        Ok(row)
    }

    async fn put(
        &self,
        operation_id: platform_kernel::OperationId,
        result: Value,
    ) -> Result<(), IdempotencyError> {
        sqlx::query(
            "INSERT INTO idempotency (operation_id, result) VALUES ($1, $2) \
             ON CONFLICT(operation_id) DO UPDATE SET result = EXCLUDED.result",
        )
        .bind(uuid::Uuid::from_bytes(operation_id.0))
        .bind(result)
        .execute(&self.pool)
        .await
        .map_err(|error| IdempotencyError::Database(error.to_string()))?;
        Ok(())
    }
}

#[derive(Clone)]
struct SqliteIdempotencyStore {
    pool: SqlitePool,
}

impl SqliteIdempotencyStore {
    fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IdempotencyStore for SqliteIdempotencyStore {
    async fn get(
        &self,
        operation_id: &platform_kernel::OperationId,
    ) -> Result<Option<Value>, IdempotencyError> {
        let row: Option<String> =
            sqlx::query_scalar("SELECT result FROM idempotency WHERE operation_id = ?")
                .bind(operation_id.0.to_vec())
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| IdempotencyError::Database(error.to_string()))?;

        row.map(|value| {
            serde_json::from_str(&value)
                .map_err(|error| IdempotencyError::Serialization(error.to_string()))
        })
        .transpose()
    }

    async fn put(
        &self,
        operation_id: platform_kernel::OperationId,
        result: Value,
    ) -> Result<(), IdempotencyError> {
        let encoded = serde_json::to_string(&result)
            .map_err(|error| IdempotencyError::Serialization(error.to_string()))?;
        sqlx::query(
            "INSERT INTO idempotency (operation_id, result, created_at) VALUES (?, ?, ?) ON CONFLICT(operation_id) DO UPDATE SET result = excluded.result",
        )
        .bind(operation_id.0.to_vec())
        .bind(encoded)
        .bind(unix_seconds().saturating_mul(1_000) as i64)
        .execute(&self.pool)
        .await
        .map_err(|error| IdempotencyError::Database(error.to_string()))?;
        Ok(())
    }
}

async fn seed_if_empty(pool: &SqlitePool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM aggregates")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }

    let org = uuid::Uuid::parse_str(ORGANIZATION_ID)?;
    let now = 1_785_923_200_000_i64; // 2026-08-05T12:00:00Z
    let fixtures = vec![
        (
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1",
            "mission",
            json!({
                "public_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","name":"Coastal Response Readiness","summary":"Coordinate readiness work across field teams.","status":"active","owner":"Operations Lead","priority":"high","progress":68,"version":4,"lifecycle_epoch":2,"authority_epoch":1,"updated_at":"2026-08-05T11:45:00Z"
            }),
            4,
            2,
            1,
        ),
        (
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2",
            "mission",
            json!({
                "public_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2","name":"Infrastructure Recovery","summary":"Restore critical service capacity and evidence completion.","status":"paused","owner":"Recovery Manager","priority":"critical","progress":42,"version":3,"lifecycle_epoch":2,"authority_epoch":1,"updated_at":"2026-08-05T10:20:00Z"
            }),
            3,
            2,
            1,
        ),
        (
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1",
            "task",
            json!({
                "public_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1","mission_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","title":"Validate emergency communications","status":"active","owner":"Field Coordinator","priority":"high","due_at":"2026-08-06T16:00:00Z","version":5,"lifecycle_epoch":1,"authority_epoch":1,"updated_at":"2026-08-05T11:50:00Z"
            }),
            5,
            1,
            1,
        ),
        (
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2",
            "task",
            json!({
                "public_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2","mission_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2","title":"Verify restoration evidence","status":"blocked","owner":"Evidence Reviewer","priority":"critical","due_at":"2026-08-05T18:00:00Z","version":2,"lifecycle_epoch":1,"authority_epoch":1,"updated_at":"2026-08-05T10:35:00Z"
            }),
            2,
            1,
            1,
        ),
        (
            "cccccccc-cccc-4ccc-8ccc-ccccccccccc1",
            "timeline",
            json!({
                "public_id":"cccccccc-cccc-4ccc-8ccc-ccccccccccc1","subject_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","subject_type":"mission","label":"Readiness checkpoint","kind":"critical_marker","at":"2026-08-06T12:00:00Z","status":"upcoming","version":1,"lifecycle_epoch":0,"authority_epoch":0,"updated_at":"2026-08-05T11:00:00Z"
            }),
            1,
            0,
            0,
        ),
        (
            "dddddddd-dddd-4ddd-8ddd-ddddddddddd1",
            "notification",
            json!({
                "public_id":"dddddddd-dddd-4ddd-8ddd-ddddddddddd1","title":"Critical marker approaching","message":"Coastal Response Readiness reaches its critical marker tomorrow.","priority":"high","status":"unacknowledged","source_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","source_type":"mission","created_at":"2026-08-05T11:30:00Z","acknowledged_at":null,"version":1,"lifecycle_epoch":0,"authority_epoch":0
            }),
            1,
            0,
            0,
        ),
        (
            "dddddddd-dddd-4ddd-8ddd-ddddddddddd2",
            "notification",
            json!({
                "public_id":"dddddddd-dddd-4ddd-8ddd-ddddddddddd2","title":"Task blocked","message":"Verify restoration evidence is blocked and requires attention.","priority":"critical","status":"unacknowledged","source_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2","source_type":"task","created_at":"2026-08-05T10:40:00Z","acknowledged_at":null,"version":1,"lifecycle_epoch":0,"authority_epoch":0
            }),
            1,
            0,
            0,
        ),
        (
            "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee1",
            "approval",
            json!({
                "public_id":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee1","title":"Approve task completion evidence","description":"Review the submitted evidence package for emergency communications.","status":"pending","requested_by":"Field Coordinator","target_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1","target_type":"task","created_at":"2026-08-05T11:35:00Z","decided_at":null,"decision_reason":null,"web_action_permitted":true,"version":1,"lifecycle_epoch":0,"authority_epoch":0
            }),
            1,
            0,
            0,
        ),
        (
            "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee2",
            "approval",
            json!({
                "public_id":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee2","title":"Restricted policy exception","description":"This decision requires a native client and senior authority.","status":"pending","requested_by":"Recovery Manager","target_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2","target_type":"mission","created_at":"2026-08-05T09:00:00Z","decided_at":null,"decision_reason":null,"web_action_permitted":false,"version":1,"lifecycle_epoch":0,"authority_epoch":0
            }),
            1,
            0,
            0,
        ),
        (
            "ffffffff-ffff-4fff-8fff-fffffffffff1",
            "report",
            json!({
                "public_id":"ffffffff-ffff-4fff-8fff-fffffffffff1","title":"Readiness Status Report","status":"approved","subject_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","subject_type":"mission","author":"Operations Analyst","submitted_at":"2026-08-05T08:30:00Z","summary":"Readiness is progressing with one communications dependency under review.","evidence":[{"label":"Field checklist","file_name":"field-checklist.pdf"}],"version":2,"lifecycle_epoch":0,"authority_epoch":0,"updated_at":"2026-08-05T09:15:00Z"
            }),
            2,
            0,
            0,
        ),
    ];

    for (id_str, aggregate_type, mut state, version, lifecycle_epoch, authority_epoch) in fixtures {
        let id = uuid::Uuid::parse_str(id_str)?;
        state["id"] = serde_json::to_value(id.as_bytes())?;
        state["version"] = json!(version);
        state["lifecycle_epoch"] = json!(lifecycle_epoch);
        state["authority_epoch"] = json!(authority_epoch);
        sqlx::query("INSERT INTO aggregates (id, aggregate_type, version, lifecycle_epoch, authority_epoch, state, updated_at, organization_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(id.as_bytes().to_vec())
            .bind(aggregate_type)
            .bind(version)
            .bind(lifecycle_epoch)
            .bind(authority_epoch)
            .bind(state.to_string())
            .bind(now)
            .bind(org.as_bytes().to_vec())
            .execute(pool)
            .await?;
    }
    Ok(())
}
