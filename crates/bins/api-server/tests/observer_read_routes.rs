//! MIGRATION_PLAN Phase 1.2 — positive and error-path proof that the two
//! new ObserverClient read routes work over real HTTP with real auth:
//!
//! - `POST/DELETE /api/push/subscriptions` — push subscription
//!   registration, idempotent re-registration, ownership-scoped deletion,
//!   and the validation/error shapes.
//! - `GET /api/files/:content_hash` — content-addressed file download,
//!   subject to `can_download_files`.
//!
//! Both are read-class routes, so a `mobile_observer` session (which
//! grants every `can_read_*`/`can_download_files` flag per ONYX-MOB-01 §8)
//! must be able to use them — that is exactly what the negative-capability
//! matrix in `mobile_observer_capability.rs` does *not* cover. These tests
//! use the seeded "All-Father" admin deliberately, mirroring that file's
//! choice to prove the ceiling applies to a highly privileged account.

use std::net::SocketAddr;
use std::sync::Arc;

use api_server::routes::ApiState;
use query_application::BlobKey;

/// Starts a real HTTP server, returning `(addr, base, blob_store)` — the
/// blob store handle is kept so a test can seed content before downloading
/// it through the route.
async fn start_server(
    db_label: &str,
) -> (SocketAddr, String, Arc<dyn query_application::BlobStore>) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api_server=debug".into()),
        )
        .try_init();
    let db_path = std::env::temp_dir().join(format!("onyx-observer-read-routes-{db_label}.db"));
    let _ = std::fs::remove_file(&db_path);
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let state = ApiState::new(&database_url).await.expect("api state");
    let blob_store = state.blob_store.clone();
    let app = api_server::routes::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let addr = listener.local_addr().expect("listener address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (addr, format!("http://{addr}"), blob_store)
}

/// Logs in `username`/`password` with a given `client_type`, returning the
/// `access_token`. `verify` asserts the login actually succeeded — used by
/// callers that expect the account to be able to log in.
async fn login_as(
    http: &reqwest::Client,
    base: &str,
    client_type: &str,
    username: &str,
    password: &str,
) -> String {
    let response: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({
            "username": username,
            "password": password,
            "client_type": client_type,
        }))
        .send()
        .await
        .expect("login request")
        .json()
        .await
        .expect("login body");
    response["access_token"]
        .as_str()
        .expect("access_token")
        .to_string()
}

const VALID_P256DH: &str =
    "BNcRdreALRFXTkOOUHK1EtK2wtaz5Ry4YfYCA_0QTpQtUbVlUls0VJXg7A8u-Ts1XbjhazAkj7I99e8QcYP7DkM";
const VALID_AUTH: &str = "I-1jMf3W9VqkQ6BbVzY0Nw";

fn push_payload(endpoint: &str) -> serde_json::Value {
    serde_json::json!({
        "endpoint": endpoint,
        "p256dh": VALID_P256DH,
        "auth": VALID_AUTH,
        "platform": "pwa",
    })
}

/// An observer can register a push subscription (read-class), re-register
/// the same endpoint idempotently, and only ever delete its own
/// subscription. Validation and auth errors match the documented shapes.
#[tokio::test]
async fn observer_can_register_and_delete_a_push_subscription() {
    let (_addr, base, _blobs) = start_server("push").await;
    let http = reqwest::Client::new();
    let token = login_as(
        &http,
        &base,
        "mobile_observer",
        "All-Father",
        "passvord0000",
    )
    .await;

    // --- Register. ---
    let register = http
        .post(format!("{base}/api/push/subscriptions"))
        .bearer_auth(&token)
        .json(&push_payload(
            "https://fcm.googleapis.com/fcm/send/test-device-1",
        ))
        .send()
        .await
        .expect("register request");
    assert_eq!(register.status(), 201);
    let first: serde_json::Value = register.json().await.expect("register body");
    let id = first["id"].as_str().expect("id").to_string();
    assert_eq!(
        first["organization_id"],
        "11111111-1111-1111-1111-111111111111"
    );

    // --- Re-registering the same endpoint is idempotent: same stored id.
    let re_register = http
        .post(format!("{base}/api/push/subscriptions"))
        .bearer_auth(&token)
        .json(&push_payload(
            "https://fcm.googleapis.com/fcm/send/test-device-1",
        ))
        .send()
        .await
        .expect("re-register request");
    assert_eq!(re_register.status(), 201);
    let second: serde_json::Value = re_register.json().await.expect("re-register body");
    assert_eq!(
        second["id"].as_str().expect("id"),
        id,
        "re-registering the same endpoint must keep the stored id stable"
    );

    // --- Delete by that id. ---
    let delete = http
        .delete(format!("{base}/api/push/subscriptions/{id}"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("delete request");
    assert_eq!(delete.status(), 204);

    // --- Deleting again 404s. ---
    let delete_again = http
        .delete(format!("{base}/api/push/subscriptions/{id}"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("second delete request");
    assert_eq!(delete_again.status(), 404);

    // --- A different user cannot delete someone else's subscription. ---
    // The seeded DB has a single user; create a second one through the
    // admin route, register its subscription under its own (web) session,
    // then try to remove it with the original observer session.
    let web_admin = login_as(&http, &base, "web", "All-Father", "passvord0000").await;
    let created = http
        .post(format!("{base}/api/admin/users"))
        .bearer_auth(&web_admin)
        .json(&serde_json::json!({
            "username": "observer-two",
            "password": "otherpass123",
            "is_admin": false,
            "class": "top_level_manager",
        }))
        .send()
        .await
        .expect("create second user request");
    assert_eq!(created.status(), 201);

    // observer-two registers under its own web session.
    let two_token = login_as(&http, &base, "web", "observer-two", "otherpass123").await;
    let kept = http
        .post(format!("{base}/api/push/subscriptions"))
        .bearer_auth(&two_token)
        .json(&push_payload(
            "https://fcm.googleapis.com/fcm/send/kept-device",
        ))
        .send()
        .await
        .expect("register kept request");
    assert_eq!(kept.status(), 201);
    let kept_id = kept.json::<serde_json::Value>().await.expect("kept body")["id"]
        .as_str()
        .expect("kept id")
        .to_string();

    let cross_delete = http
        .delete(format!("{base}/api/push/subscriptions/{kept_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("cross-user delete request");
    assert_eq!(
        cross_delete.status(),
        404,
        "deleting another user's subscription must be refused"
    );

    // --- Validation and auth error shapes. ---
    let bad_endpoint = http
        .post(format!("{base}/api/push/subscriptions"))
        .bearer_auth(&token)
        .json(&push_payload("http://insecure.example.com/push"))
        .send()
        .await
        .expect("bad endpoint request");
    assert_eq!(bad_endpoint.status(), 400);

    let bad_key = http
        .post(format!("{base}/api/push/subscriptions"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "endpoint": "https://fcm.googleapis.com/fcm/send/x",
            "p256dh": "!!!not-base64url!!!",
            "auth": VALID_AUTH,
            "platform": "pwa",
        }))
        .send()
        .await
        .expect("bad key request");
    assert_eq!(bad_key.status(), 400);

    let unauth = http
        .post(format!("{base}/api/push/subscriptions"))
        .json(&push_payload("https://fcm.googleapis.com/fcm/send/y"))
        .send()
        .await
        .expect("unauthenticated request");
    assert_eq!(unauth.status(), 401);

    let bad_delete_id = http
        .delete(format!("{base}/api/push/subscriptions/not-a-uuid"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("bad delete id request");
    assert_eq!(bad_delete_id.status(), 400);
}

/// An observer can download stored file bytes by content hash
/// (`can_download_files` is granted to observers), with the documented
/// 400/404 and auth error shapes for malformed or absent hashes.
#[tokio::test]
async fn observer_can_download_stored_content_and_errors_match() {
    let (_addr, base, blobs) = start_server("files").await;
    let http = reqwest::Client::new();
    let token = login_as(
        &http,
        &base,
        "mobile_observer",
        "All-Father",
        "passvord0000",
    )
    .await;

    let content = b"the quick brown fox".to_vec();
    let hash = hex_sha256(&content);
    blobs
        .put(&BlobKey::new(hash.clone()), &content)
        .await
        .expect("seeding blob must succeed");

    // --- Download the stored hash. ---
    let download = http
        .get(format!("{base}/api/files/{hash}"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("download request");
    assert_eq!(download.status(), 200);
    assert_eq!(
        download.bytes().await.expect("download body"),
        content,
        "downloaded bytes must exactly match what was stored"
    );

    // --- A well-formed but absent hash 404s. ---
    let missing_hash = "0".repeat(64);
    let missing = http
        .get(format!("{base}/api/files/{missing_hash}"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("missing hash request");
    assert_eq!(missing.status(), 404);

    // --- A malformed hash 400s before any store access. ---
    let malformed = http
        .get(format!("{base}/api/files/NOT_HEX"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("malformed hash request");
    assert_eq!(malformed.status(), 400);

    // --- Unauthenticated download is refused. ---
    let unauth = http
        .get(format!("{base}/api/files/{hash}"))
        .send()
        .await
        .expect("unauthenticated download request");
    assert_eq!(unauth.status(), 401);
}

fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
