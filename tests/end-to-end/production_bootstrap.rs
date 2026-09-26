//! H1 (audit finding H-01 follow-up): proves `ONYX_ENV=production` against
//! a genuinely empty database categorically refuses to create the known
//! seeded "All-Father" admin account, and that logging in with its known
//! credentials fails. This is the test that proves the fix -- not just
//! that the gating code changed.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

use super::test_harness::{PostgresHarness, TEST_ADMIN_PASSWORD, TEST_ADMIN_USERNAME};
use api_server::routes::{router, ApiState};

#[tokio::test(flavor = "current_thread")]
async fn production_env_never_seeds_the_known_admin_account() -> anyhow::Result<()> {
    let postgres = PostgresHarness::start().await?;

    // A full, real production boot: ONYX_ENV=production requires (and
    // this test genuinely provides) a Postgres primary, a real
    // ONYX_AUTHORITY_SIGNING_KEY, and a real ONYX_GOVERNANCE_DATABASE_URL
    // -- exercising the same top-of-function gates ApiState::new enforces
    // for any real production deployment, not a shortcut around them.
    std::env::set_var("ONYX_ENV", "production");
    std::env::set_var(
        "ONYX_AUTHORITY_SIGNING_KEY",
        "hex:4242424242424242424242424242424242424242424242424242424242424242",
    );
    std::env::set_var("ONYX_GOVERNANCE_DATABASE_URL", &postgres.database_url);
    // Required in production too (H4(a)), otherwise ApiState::new would
    // bail before ever reaching the seeded-admin gate this test targets.
    std::env::set_var(
        "ONYX_CORS_ALLOWED_ORIGINS",
        "https://admin.onyx.example.com",
    );

    let state_result = ApiState::new(&postgres.database_url).await;

    // Restore the environment immediately, win or lose, so a later test
    // in this same process (CI runs `--test-threads=1`, but this must
    // not depend on that) never inherits a stray ONYX_ENV=production.
    std::env::remove_var("ONYX_ENV");
    std::env::remove_var("ONYX_GOVERNANCE_DATABASE_URL");
    std::env::remove_var("ONYX_CORS_ALLOWED_ORIGINS");

    let state = state_result?;

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&postgres.pool)
        .await?;
    assert_eq!(
        user_count, 0,
        "ONYX_ENV=production against an empty database must never seed any account, \
         including the known 'All-Father' shortcut"
    );

    let app = router(state);
    let login_attempt = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"username": TEST_ADMIN_USERNAME, "password": TEST_ADMIN_PASSWORD})
                        .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(
        login_attempt.status(),
        StatusCode::UNAUTHORIZED,
        "login with the known seeded-admin credentials must fail in production, \
         since the account must never have been created"
    );

    postgres
        .record("h1-production-never-seeds-admin", "passed")
        .await?;
    Ok(())
}
