//! Team 6 HTTP/WebSocket integration surface, hardened by Team 7.
//! Binding rulings are recorded in the workspace DECISIONS.md.

pub mod admin;
pub mod auth;
#[path = "../clerk.rs"]
pub mod clerk;
pub mod client_type;
pub mod command;
pub mod events;
pub mod files;
pub mod policy_admin;
pub mod profiles;
pub mod push;
pub mod query;
pub mod relay;
pub mod todo_admin;

use std::{
    collections::HashMap,
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use audit_application::AuditWriter;
use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use observability_adapter::{HashChainAuditWriter, Metrics};
use persistence_postgres::{PostgresRepository, PostgresUnitOfWorkFactory};
use persistence_sqlite::{SqliteRepository, SqliteUnitOfWorkFactory};
use platform_kernel::{ObjectId, OrganizationId};
use query_application::{
    BlobStore, IdempotencyError, IdempotencyStore, Repository, UnitOfWorkFactory,
};
use security_adapter::{
    Ed25519JwtCodec, EnvironmentSecretProvider, InMemorySlidingWindowRateLimiter,
    InMemoryTokenRevocationStore, PasswordHasher, PostgresSlidingWindowRateLimiter,
    PostgresTokenRevocationStore, PostgresUserStore, SqliteUserStore,
};
use security_application::{RateLimiter, SecretProvider, TokenRevocationStore, UserStore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{
    postgres::PgPoolOptions,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    PgPool, SqlitePool,
};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use crate::{config::AppConfig, query_handler::ProjectionPool};

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
    /// Durable, shared token/session revocation (audit finding H-02).
    /// Backed by Postgres in production (and any deployment with a
    /// governance or primary Postgres pool); an in-memory fallback only
    /// for a pure-SQLite, single-instance dev/test composition. See
    /// `security_application::ports::token_revocation`.
    pub token_revocation_store: Arc<dyn TokenRevocationStore>,
    pub secret_provider: Arc<dyn SecretProvider>,
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub audit_writer: Arc<dyn AuditWriter>,
    pub metrics: Metrics,
    /// Identity store backing `/api/auth/login` (audit finding H-01).
    /// Replaces the former `DEFAULT_USERNAME`/`DEFAULT_PASSWORD` constants.
    pub user_store: Arc<dyn UserStore>,
    /// Optional Clerk identity boundary. Clerk is never the ONYX authorization store;
    /// this adapter only verifies the external identity and exchanges it for an
    /// already-provisioned ONYX principal.
    pub clerk_auth: Option<Arc<clerk::ClerkAuth>>,
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
    /// Content-addressed file storage backing `GET /api/files/:content_hash`
    /// (MIGRATION_PLAN Phase 1.2). Rooted at `ONYX_BLOB_STORE_ROOT` when set,
    /// else a per-host temp directory — the same `BlobStore` port
    /// desktop-shell's `FileUploadCoordinator` writes through.
    pub blob_store: Arc<dyn BlobStore>,
    /// Explicit CORS origin allow-list (audit finding H-03 / hardening
    /// track H4(a)), parsed once at startup from
    /// `ONYX_CORS_ALLOWED_ORIGINS`. `None` means "reflect any origin"
    /// (`tower_http::cors::Any`) — the unchanged, permissive default
    /// outside production, so every existing local dev/test workflow
    /// (web-ui's and admin-shell's Vite dev servers, desktop-shell's
    /// webview, mobile emulators) keeps working exactly as before.
    /// `ApiState::new` refuses to start in production without this being
    /// `Some` and non-empty — see its own comment for why a concrete
    /// default cannot be hardcoded here.
    pub cors_allowed_origins: Option<Vec<HeaderValue>>,
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
    /// Compatibility constructor retained for existing test harnesses and
    /// integration tests that explicitly provide a database URL.
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let config = AppConfig::for_database(database_url)?;
        Self::new_with_config(config).await
    }

    /// Authoritative API composition entry point. All environment-sensitive
    /// startup policy is parsed and validated by AppConfig before state is built.
    pub async fn new_with_config(config: AppConfig) -> anyhow::Result<Self> {
        let postgres_primary = config.database_is_postgres();

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
                .connect(config.database_url())
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
            let options = SqliteConnectOptions::from_str(config.database_url())?
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
                .max_connections(if config.database_url().contains(":memory:") {
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
            // Production was rejected by AppConfig when the signing key was
            // absent. Non-production compositions retain the deterministic
            // local key that existing tests and local development rely on.
            if config.is_production() {
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
            let pool = sqlite_pool
                .clone()
                .ok_or_else(|| anyhow::anyhow!("no primary database pool available"))?;
            Arc::new(SqliteUserStore::new(pool))
        };

        let governance_url = config.governance_database_url().map(str::to_owned);

        // H-03 / H4(a): explicit CORS origin allow-list, driven by config
        // rather than hardcoded. A concrete origin list cannot honestly be
        // hardcoded here: confirmed by reading this repo's actual
        // deployment config (deploy/helm/, deploy/docker/) that neither
        // `web-ui` nor `admin-shell` has a Dockerfile, Helm chart, or
        // ingress entry at all today — both currently exist only as local
        // Vite dev servers (ports 5173/5174 respectively; see their
        // vite.config.ts). Only `api-server` itself is deployed
        // (`deploy/helm/onyx-api`, host `api.onyx.example.com`). Since no
        // real production origin for either browser client has been
        // decided or deployed, this is properly a deployment-time config
        // value, not something this code can guess — set
        // `ONYX_CORS_ALLOWED_ORIGINS` (comma-separated) to whatever origin(s)
        // web-ui/admin-shell actually get deployed under once that happens.
        let cors_allowed_origins = config.cors_allowed_origins().map(ToOwned::to_owned);
        let (rate_limiter, audit_writer, token_revocation_store): (
            Arc<dyn RateLimiter>,
            Arc<dyn AuditWriter>,
            Arc<dyn TokenRevocationStore>,
        ) = if let Some(url) = governance_url {
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
                Arc::new(HashChainAuditWriter::postgres(governance_pool.clone())),
                Arc::new(PostgresTokenRevocationStore::new(governance_pool)),
            )
        } else if let Some(pool) = primary_postgres_pool {
            (
                Arc::new(PostgresSlidingWindowRateLimiter::new(pool.clone())),
                Arc::new(HashChainAuditWriter::postgres(pool.clone())),
                Arc::new(PostgresTokenRevocationStore::new(pool)),
            )
        } else {
            let pool = sqlite_pool
                .ok_or_else(|| anyhow::anyhow!("SQLite composition must retain its pool"))?;
            (
                Arc::new(InMemorySlidingWindowRateLimiter::default()),
                Arc::new(HashChainAuditWriter::sqlite(pool)),
                Arc::new(InMemoryTokenRevocationStore::default()),
            )
        };

        let password_hasher = Arc::new(PasswordHasher::new());

        // Clerk is the external identity boundary. Allfather is permanently
        // bound to the designated verified email in clerk.rs; there is no
        // HTTP bootstrap and no self-registration flow. The canonical
        // Allfather principal is materialized once so the verified identity
        // can exchange its Clerk session for an ONYX administrator session.
        let clerk_auth = clerk::ClerkAuth::from_env()?.map(Arc::new);
        if clerk_auth.is_some() {
            match user_store
                .find_by_username(clerk::ALLFATHER_USERNAME)
                .await?
            {
                Some(existing) if !existing.is_admin || !existing.is_active => {
                    anyhow::bail!("configured Allfather principal is not backed by an active ONYX administrator");
                }
                Some(_) => {}
                None => {
                    let password_hash = password_hasher
                        .hash(&format!("onyx-clerk-{}", uuid::Uuid::new_v4()))
                        .map_err(|e| {
                            anyhow::anyhow!("failed to create unusable Allfather credential: {e}")
                        })?;
                    user_store
                        .create(security_application::NewUser {
                            user_id: uuid::Uuid::new_v4().to_string(),
                            username: clerk::ALLFATHER_USERNAME.to_owned(),
                            organization_id: ORGANIZATION_ID.to_owned(),
                            password_hash,
                            is_admin: true,
                            is_manager: false,
                            class: None,
                            parent_user_id: None,
                        })
                        .await?;
                    tracing::info!(
                        "provisioned the designated Allfather email identity as the ONYX master administrator"
                    );
                }
            }
        }

        let blob_store = crate::blob_storage::build(&config).await?;

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
            token_revocation_store,
            secret_provider,
            rate_limiter,
            audit_writer,
            metrics: Metrics::new("onyx_api_server")?,
            user_store,
            clerk_auth,
            password_hasher,
            relay_registry: relay::RelayRegistry::new(),
            blob_store,
            cors_allowed_origins,
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

    // H4(a): PUT added -- /api/admin/mobile-access and /api/admin/profiles
    // are both real, currently-registered PUT routes. DELETE is required for
    // /api/push/subscriptions/:subscription_id.
    let cors_methods = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
    ];
    let cors_layer = match state.cors_allowed_origins.clone() {
        Some(origins) => CorsLayer::new()
            .allow_origin(origins)
            .allow_headers(tower_http::cors::Any)
            .allow_methods(cors_methods),
        None => CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
            .allow_methods(cors_methods),
    };

    // Public bootstrap/authentication surface. Everything below is explicitly
    // placed in the protected router unless it has its own independent
    // credential (the relay ticket route).
    let protected_routes = Router::new()
        // Ordinary authenticated identity pickers.
        .route("/api/users", get(admin::list_picker_users))
        .route("/api/users/hierarchy", get(admin::list_hierarchy_users))
        // Admin/user-management surface.
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
        .route("/api/admin/users/:id/manager", post(admin::set_manager))
        .route("/api/admin/users/:id/class", post(admin::set_class))
        .route("/api/admin/users/:id/parent", post(admin::set_parent))
        .route("/api/admin/clerk-users", post(clerk::provision))
        .route(
            "/api/admin/mobile-access",
            get(admin::get_mobile_access).put(admin::set_mobile_access),
        )
        // Staff profiles.
        .route("/api/profiles", get(profiles::list_profiles))
        .route("/api/profiles/:owner_id", get(profiles::get_profile))
        .route("/api/admin/profiles", put(profiles::upsert_profile_route))
        .route(
            "/api/admin/profiles/import",
            post(profiles::batch::import_profiles),
        )
        .route(
            "/api/admin/profiles/export",
            get(profiles::batch::export_profiles),
        )
        // Policy and legal holds.
        .route("/api/admin/policies", post(policy_admin::create_policy))
        .route(
            "/api/admin/legal-holds",
            post(policy_admin::apply_legal_hold),
        )
        // Todo/Target/StaffLoan APIs.
        .route("/api/todo/lists", post(todo_admin::create_todo_list))
        .route("/api/todo/targets", post(todo_admin::create_target_list))
        .route(
            "/api/todo/staff-loans",
            post(todo_admin::request_staff_loan),
        )
        // Authenticated session and data APIs.
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/command", command_route)
        .route("/api/query", get(query::query_route))
        // Relay ticket issuance uses the caller's ordinary access token.
        .route("/api/relay-ticket", post(relay::issue_ticket))
        // File and Web Push APIs.
        .route("/api/files/:content_hash", get(files::download_file))
        .route("/api/push/subscriptions", post(push::register_subscription))
        .route(
            "/api/push/subscriptions/:subscription_id",
            delete(push::unregister_subscription),
        )
        // M-02: central route-layer authentication. This is the fail-safe
        // boundary for future protected routes. Existing handlers can retain
        // their narrower capability/tenant checks during the incremental
        // migration to Extension<AuthenticatedUser>.
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::authenticate_request,
        ));

    Router::new()
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/ready", get(readiness))
        // Ordinary Admin/Staff accounts authenticate directly against ONYX credentials.
        .route("/api/auth/login", post(auth::login))
        // Clerk/Google is reserved for the designated All-Father identity.
        .route("/api/auth/clerk", post(clerk::login))
        // Refresh consumes a refresh token rather than an access token, so it
        // intentionally remains outside the standard access-auth layer.
        .route("/api/auth/refresh", post(auth::refresh))
        // /api/events performs its own authentication from the
        // WebSocket subprotocol header; it therefore must not be wrapped
        // by the standard Authorization-header middleware above.
        .route("/api/events", get(events::websocket_route))
        .merge(protected_routes)
        // The relay WebSocket authenticates with its own short-lived,
        // single-use, target-scoped ticket; it must not require an access
        // bearer token as well.
        .route("/api/relay/:target_id", get(relay::relay_route))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::rate_limit::observe_request,
        ))
        .layer(cors_layer)
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
    /// Bound relay replica identity for `relay_ticket`-typed tokens only.
    /// Verified against `replica_ownership` at mint time (see relay.rs);
    /// `None` for every other token type. Optional/defaulted so existing
    /// access/refresh tokens without this field still deserialize.
    #[serde(default)]
    pub self_replica: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub username: String,
    pub organization_id: String,
    pub token_type: String,
    pub scope: TokenScope,
    /// Server-owned client classification (H10/P1.1), bound once at
    /// login and carried forward unchanged by every subsequent
    /// `refresh` for this session's lifetime -- see `auth::refresh`,
    /// which reads this field from the presented refresh token's own
    /// claims rather than re-deriving it, so an observer session cannot
    /// be "upgraded" to unrestricted merely by rotating its token.
    ///
    /// `#[serde(default = ...)]`: a token encoded before this field
    /// existed still decodes, resolving to
    /// [`client_type::ClientType::default_on_absence`] -- the same
    /// back-compat default a login request that omits `client_type`
    /// resolves to, so an in-flight session isn't retroactively
    /// misclassified by a field it predates.
    #[serde(default = "client_type::ClientType::default_on_absence")]
    pub client_type: client_type::ClientType,
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
    pub client_type: client_type::ClientType,
}

pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Hashes a raw bearer token for storage in `token_revocation_store`
/// (audit finding H-02) — the store must never hold raw, replayable
/// tokens, only a one-way digest of them.
pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
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
    client_type: client_type::ClientType,
) -> anyhow::Result<String> {
    let now = unix_seconds();
    let claims = TokenClaims {
        sub: user.user_id.clone(),
        username: user.username.clone(),
        organization_id: user.organization_id.clone(),
        token_type: token_type.to_string(),
        client_type,
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
                "staff_loan.EscalateStaffLoan".to_string(),
                "staff_loan.ExpireStaffLoan".to_string(),
            ],
            delegation_depth: 0,
            self_replica: None,
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
    // H-02: two independent revocation checks against the shared store —
    // see security_application::ports::token_revocation for why both
    // exist. Either one failing rejects the token.
    let revoked = state
        .token_revocation_store
        .is_token_revoked(&token_hash(token))
        .await
        .map_err(|_| ApiError::unauthorized(uuid::Uuid::new_v4().to_string()))?;
    if revoked {
        return Err(ApiError::unauthorized(uuid::Uuid::new_v4().to_string()));
    }
    if let Some(revoked_before) = state
        .token_revocation_store
        .user_revoked_before(&claims.sub)
        .await
        .map_err(|_| ApiError::unauthorized(uuid::Uuid::new_v4().to_string()))?
    {
        // `<=`, not `<`: `iat`/`revoked_before` share the same 1-second
        // resolution (`unix_seconds()`), so a token minted in the same
        // wall-clock second as a subsequent deactivation/password-reset
        // would otherwise tie with its own revocation watermark and be
        // treated as still valid — confirmed as a real, reproducing bug
        // via `tests/end-to-end/session_revocation.rs`'s cross-replica
        // deactivation test failing in real CI (left: 200, right: 401)
        // before this fix. A token legitimately reissued in that same
        // second after a reset is rejected too; the caller simply logs
        // in again, which is the correct fail-closed direction for a
        // revocation check.
        if claims.iat <= revoked_before {
            return Err(ApiError::unauthorized(uuid::Uuid::new_v4().to_string()));
        }
    }
    Ok(claims)
}

/// Deterministic fault injection used only by the explicit test-endpoints
/// feature. The non-feature implementation is a no-op, so production binaries
/// contain neither the environment switch nor the client-controlled header path.
#[cfg(feature = "test-endpoints")]
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

#[cfg(not(feature = "test-endpoints"))]
pub fn test_mode_error(_headers: &HeaderMap, _correlation_id: &str) -> Option<ApiError> {
    None
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
        client_type: claims.client_type,
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
    // ORGANIZATION_ID is the fixed development/test fixture UUID
    // 11111111-1111-1111-1111-111111111111, i.e. 16 bytes of 0x11.
    ObjectId([0x11; 16])
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
