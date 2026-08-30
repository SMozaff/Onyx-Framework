//! H2 (audit finding H-02): proves the actual property that matters —
//! that revocation performed on one API replica is visible to a second,
//! independent replica that shares nothing in-process with the first,
//! only the same backing Postgres database. The former
//! `ApiState::revoked_tokens: Arc<RwLock<HashSet<String>>>` could never
//! pass this test: each `ApiState::new` call built its own empty
//! `HashSet` with no way for one instance to ever see what another wrote.

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

use super::test_harness::{PostgresHarness, TEST_ADMIN_PASSWORD, TEST_ADMIN_USERNAME};
use api_server::routes::{router, ApiState};

async fn json_body(response: axum::response::Response) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[tokio::test(flavor = "current_thread")]
async fn logout_on_one_replica_revokes_the_token_on_a_second_independent_replica(
) -> anyhow::Result<()> {
    let postgres = PostgresHarness::start().await?;
    std::env::set_var("ONYX_ENV", "test");

    // Two fully independent ApiState/Router instances -- each with its
    // own in-process rate limiter state, connection pool handle, etc. --
    // pointed at the exact same Postgres database. This is the real
    // production topology H2 is about: multiple horizontally-scaled
    // replicas sharing nothing except the database.
    let replica_a = router(ApiState::new(&postgres.database_url).await?);
    let replica_b = router(ApiState::new(&postgres.database_url).await?);

    // Log in via replica A.
    let login_response = replica_a
        .clone()
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
    assert_eq!(login_response.status(), StatusCode::OK);
    let login_json = json_body(login_response).await?;
    let access_token = login_json["access_token"].as_str().unwrap().to_string();
    let refresh_token = login_json["refresh_token"].as_str().unwrap().to_string();

    // Sanity check: the access token works against replica B *before*
    // revocation. This proves replica B genuinely shares the same real
    // user/session (via the shared database), not that it simply
    // rejects everything -- so the later 401 can only be attributed to
    // the revocation itself.
    let pre_revocation = replica_b
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users/hierarchy")
                .header("authorization", format!("Bearer {access_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        pre_revocation.status(),
        StatusCode::OK,
        "the access token must work against replica B before any revocation, \
         proving both replicas share the same real session"
    );

    // Log out via replica A only -- replica B never receives this
    // request and has no in-process knowledge that it happened.
    let logout_response = replica_a
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header("authorization", format!("Bearer {access_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"refresh_token": refresh_token}).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(logout_response.status(), StatusCode::OK);

    // The actual point of H2: replica B, which never saw the logout
    // call, must reject the same access token now -- because
    // revocation lives in the shared Postgres store both replicas read,
    // not in replica A's local memory.
    let post_revocation = replica_b
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users/hierarchy")
                .header("authorization", format!("Bearer {access_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        post_revocation.status(),
        StatusCode::UNAUTHORIZED,
        "a token revoked via one replica must be rejected by a second, independent \
         replica reading the same shared revocation store -- this is the exact \
         production-topology bug H-02 (H2) describes"
    );

    postgres
        .record("h2-cross-replica-revocation", "passed")
        .await?;
    Ok(())
}

/// Complements the logout test above with the other real gap H2 closes:
/// `deactivate_user`/`set_user_password` previously did not touch
/// revocation at all (see `admin.rs`'s own former doc comment on
/// `set_user_password`), so an already-issued token kept working for its
/// full TTL after either action. Proves the per-user watermark, not just
/// the single-token path, is genuinely shared cross-replica too.
#[tokio::test(flavor = "current_thread")]
async fn deactivating_a_user_on_one_replica_revokes_their_session_on_a_second_replica(
) -> anyhow::Result<()> {
    let postgres = PostgresHarness::start().await?;
    std::env::set_var("ONYX_ENV", "test");

    let replica_a = router(ApiState::new(&postgres.database_url).await?);
    let replica_b = router(ApiState::new(&postgres.database_url).await?);

    // Bootstrap the admin (seeded by ApiState::new in non-production
    // envs) and a second, ordinary user to deactivate.
    let admin_login = json_body(
        replica_a
            .clone()
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
            .await?,
    )
    .await?;
    let admin_token = admin_login["access_token"].as_str().unwrap().to_string();

    let create_response = replica_a
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"username": "h2-target-user", "password": "a-real-password-12"})
                        .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let target_login = json_body(
        replica_a
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"username": "h2-target-user", "password": "a-real-password-12"})
                            .to_string(),
                    ))?,
            )
            .await?,
    )
    .await?;
    let target_token = target_login["access_token"].as_str().unwrap().to_string();
    let target_id = target_login["user"]["id"].as_str().unwrap().to_string();

    // The target's token works against replica B before deactivation.
    let pre = replica_b
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users/hierarchy")
                .header("authorization", format!("Bearer {target_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(pre.status(), StatusCode::OK);

    // Deactivate via replica A only.
    let deactivate_response = replica_a
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/admin/users/{target_id}/deactivate"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(deactivate_response.status(), StatusCode::OK);

    // Replica B, which never saw the deactivation call, must now reject
    // the target's already-issued token -- proving the per-user
    // revoke-all watermark is read from the shared store, not
    // per-replica memory.
    let post = replica_b
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users/hierarchy")
                .header("authorization", format!("Bearer {target_token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        post.status(),
        StatusCode::UNAUTHORIZED,
        "deactivating a user on one replica must revoke their existing session \
         as seen by a second, independent replica"
    );

    postgres
        .record("h2-cross-replica-deactivation-revocation", "passed")
        .await?;
    Ok(())
}
