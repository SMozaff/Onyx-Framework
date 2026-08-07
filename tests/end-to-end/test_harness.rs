//! Shared Team 8 E2E test harness.

use std::time::Duration;

use anyhow::Context;
use sqlx::{postgres::PgPoolOptions, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{runners::SyncRunner, Container},
};

pub const ORGANIZATION_ID: &str = "11111111-1111-1111-1111-111111111111";

pub struct PostgresHarness {
    _container: Container<Postgres>,
    pub pool: PgPool,
    pub database_url: String,
}

impl PostgresHarness {
    pub async fn start() -> anyhow::Result<Self> {
        let container = Postgres::default().start()?;
        let port = container.get_host_port_ipv4(5432)?;
        let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

        let mut last_error = None;
        let mut connected = None;
        for _ in 0..80 {
            match PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await
            {
                Ok(pool) => {
                    connected = Some(pool);
                    break;
                }
                Err(error) => {
                    last_error = Some(error);
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }
        }
        let pool = connected
            .ok_or_else(|| anyhow::anyhow!("PostgreSQL did not become ready: {:?}", last_error))?;

        sqlx::migrate!("../../migrations/postgres")
            .run(&pool)
            .await
            .context("failed to migrate E2E PostgreSQL")?;
        seed_from_team6_fixture(&pool).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS e2e_journey_results (journey TEXT PRIMARY KEY, outcome TEXT NOT NULL, completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW())",
        )
        .execute(&pool)
        .await?;

        Ok(Self {
            _container: container,
            pool,
            database_url,
        })
    }

    pub async fn record(&self, journey: &str, outcome: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO e2e_journey_results (journey, outcome) VALUES ($1, $2) ON CONFLICT (journey) DO UPDATE SET outcome = EXCLUDED.outcome, completed_at = NOW()",
        )
        .bind(journey)
        .bind(outcome)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

async fn seed_from_team6_fixture(pool: &PgPool) -> anyhow::Result<()> {
    let fixture = include_str!("../fixtures/seed.sql");
    anyhow::ensure!(
        fixture.contains(ORGANIZATION_ID),
        "seed fixture tenant changed"
    );

    // Team 6 froze the fixture in SQLite syntax. The E2E harness reads that
    // source directly, extracts its projection JSON, and inserts the same
    // logical records into PostgreSQL using typed binds.
    for block in fixture.split("INSERT INTO aggregates").skip(1) {
        let values = match block.split_once("VALUES") {
            Some((_, values)) => values,
            None => continue,
        };
        let state_start = match values.find("'{") {
            Some(index) => index + 1,
            None => continue,
        };
        let state_tail = &values[state_start..];
        let state_end = match state_tail.find("}',") {
            Some(index) => index + 1,
            None => continue,
        };
        let state_text = &state_tail[..state_end];
        let state: serde_json::Value = serde_json::from_str(state_text)?;
        // Own the extracted id rather than borrowing from `state`: `state` is
        // moved into `.bind(state)` below, so a `&str` borrow of it cannot stay
        // alive across that move (E0505). `aggregate_type` is `&'static str`
        // and is unaffected.
        let public_id = state
            .get("public_id")
            .and_then(serde_json::Value::as_str)
            .context("seed aggregate missing public_id")?
            .to_owned();
        let aggregate_type = infer_aggregate_type(&state)?;
        let version = state
            .get("version")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        let lifecycle_epoch = state
            .get("lifecycle_epoch")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        let authority_epoch = state
            .get("authority_epoch")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        sqlx::query(
            "INSERT INTO aggregates (id, aggregate_type, version, lifecycle_epoch, authority_epoch, state, organization_id) VALUES ($1::uuid, $2, $3, $4, $5, $6, $7::uuid) ON CONFLICT (id) DO UPDATE SET state = EXCLUDED.state, version = EXCLUDED.version",
        )
        .bind(public_id)
        .bind(aggregate_type)
        .bind(version)
        .bind(lifecycle_epoch)
        .bind(authority_epoch)
        .bind(state)
        .bind(ORGANIZATION_ID)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn infer_aggregate_type(state: &serde_json::Value) -> anyhow::Result<&'static str> {
    if state.get("progress").is_some() {
        Ok("mission")
    } else if state.get("due_at").is_some() {
        Ok("task")
    } else if state.get("kind").is_some() {
        Ok("timeline")
    } else if state.get("priority").is_some() && state.get("message").is_some() {
        Ok("notification")
    } else if state.get("web_action_permitted").is_some() {
        Ok("approval")
    } else if state.get("evidence").is_some() {
        Ok("report")
    } else {
        anyhow::bail!("unknown seed projection shape")
    }
}

// ---------------------------------------------------------------------------
// Test authentication helpers
//
// Added by the production-readiness audit (finding H-01). The delivered tests
// logged in with the compile-time constants `operator`/`onyx`. Those constants
// were the vulnerability and have been removed, so tests must now provision a
// real user the same way an operator would: call the one-time bootstrap
// endpoint, then log in.
// ---------------------------------------------------------------------------

/// Bootstrap token used by the test suite.
///
/// This is a *test fixture*, not a credential with any production meaning: the
/// bootstrap endpoint reads `ONYX_BOOTSTRAP_TOKEN` from the environment, and
/// [`bootstrap_and_login`] sets it on the spot for the process under test.
pub const TEST_BOOTSTRAP_TOKEN: &str = "test-bootstrap-token";

/// Password used for the bootstrapped test admin.
///
/// Must satisfy the production policy (>= 12 characters), because the tests
/// exercise the real validation path rather than bypassing it.
pub const TEST_ADMIN_PASSWORD: &str = "test-admin-passphrase";

/// The username the test suite bootstraps.
pub const TEST_ADMIN_USERNAME: &str = "test-admin";

/// Provisions the first admin via `/api/admin/bootstrap`, logs in, and returns
/// a bearer access token.
///
/// Safe to call once per freshly-started harness. The bootstrap endpoint is
/// one-shot by design: a second call against a non-empty user table returns
/// 409, so this must not be invoked twice against the same database.
pub async fn bootstrap_and_login(app: axum::Router) -> anyhow::Result<String> {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    // The API server reads this at request time, so setting it here is
    // sufficient and keeps the token out of the committed environment.
    std::env::set_var("ONYX_BOOTSTRAP_TOKEN", TEST_BOOTSTRAP_TOKEN);

    let bootstrap = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/bootstrap")
                .header("content-type", "application/json")
                .header("x-onyx-bootstrap-token", TEST_BOOTSTRAP_TOKEN)
                .body(Body::from(
                    serde_json::json!({
                        "username": TEST_ADMIN_USERNAME,
                        "password": TEST_ADMIN_PASSWORD,
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    anyhow::ensure!(
        bootstrap.status() == StatusCode::CREATED,
        "bootstrap failed with status {}",
        bootstrap.status()
    );

    let login = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": TEST_ADMIN_USERNAME,
                        "password": TEST_ADMIN_PASSWORD,
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    anyhow::ensure!(
        login.status() == StatusCode::OK,
        "login failed with status {}",
        login.status()
    );

    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(login.into_body(), usize::MAX).await?)?;
    body["access_token"]
        .as_str()
        .map(str::to_owned)
        .context("login response contained no access_token")
}
