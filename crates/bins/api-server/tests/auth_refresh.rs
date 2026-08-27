//! Real, end-to-end proof that `POST /api/auth/refresh` actually works
//! over real HTTP (same harness style as `mobile_access_gate.rs`):
//! redeems a valid refresh token for a new access token, rotates the
//! refresh token (the old one can never be redeemed twice), rejects a
//! bogus/already-used refresh token outright, and — the scenario this
//! route exists for — an access token that has genuinely reached its
//! real expiry is rejected, while the still-valid refresh token issued
//! alongside it can still redeem a working replacement.

use std::net::SocketAddr;

use api_server::routes::TokenClaims;
use security_adapter::Ed25519JwtCodec;
use security_application::SecretProvider;

async fn start_server(db_label: &str) -> (SocketAddr, reqwest::Client) {
    let db_path = std::env::temp_dir().join(format!("onyx-auth-refresh-test-{db_label}.db"));
    let _ = std::fs::remove_file(&db_path);
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let state = api_server::routes::ApiState::new(&database_url)
        .await
        .expect("api state");
    let app = api_server::routes::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (addr, reqwest::Client::new())
}

#[tokio::test]
async fn refresh_token_yields_a_working_new_access_token_and_rotates() {
    let (addr, http) = start_server("basic").await;
    let base = format!("http://{addr}");

    // ApiState::new seeds a fixed admin account ("All-Father" /
    // "passvord0000") on first startup against an empty store -- see
    // routes/mod.rs's own doc comment; used here the same way
    // mobile_access_gate.rs's tests already do, to avoid the
    // token-gated /api/admin/bootstrap flow this seed leaves
    // permanently closed.
    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "All-Father", "password": "passvord0000"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let original_access_token = login["access_token"].as_str().unwrap().to_string();
    let original_refresh_token = login["refresh_token"].as_str().unwrap().to_string();

    // Redeem the refresh token for a new access token.
    let refresh_response = http
        .post(format!("{base}/api/auth/refresh"))
        .json(&serde_json::json!({"refresh_token": original_refresh_token}))
        .send()
        .await
        .unwrap();
    assert_eq!(refresh_response.status(), 200);
    let refreshed: serde_json::Value = refresh_response.json().await.unwrap();
    let new_access_token = refreshed["access_token"].as_str().unwrap().to_string();
    let new_refresh_token = refreshed["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(
        new_access_token, original_access_token,
        "refresh must issue a genuinely new access token"
    );
    assert_ne!(
        new_refresh_token, original_refresh_token,
        "refresh must rotate to a genuinely new refresh token"
    );

    // The new access token actually works against a real authenticated
    // endpoint (not just a well-formed-looking string).
    let hierarchy_response = http
        .get(format!("{base}/api/users/hierarchy"))
        .bearer_auth(&new_access_token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        hierarchy_response.status(),
        200,
        "the refreshed access token must be usable for a real authenticated request"
    );

    // The original (now-rotated) refresh token can never be redeemed
    // again.
    let reuse_attempt = http
        .post(format!("{base}/api/auth/refresh"))
        .json(&serde_json::json!({"refresh_token": original_refresh_token}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        reuse_attempt.status(),
        401,
        "a rotated-out refresh token must never be redeemable again"
    );

    // The new refresh token, however, still works (rotation swaps which
    // token is valid; it does not brick the session).
    let second_refresh = http
        .post(format!("{base}/api/auth/refresh"))
        .json(&serde_json::json!({"refresh_token": new_refresh_token}))
        .send()
        .await
        .unwrap();
    assert_eq!(second_refresh.status(), 200);
}

#[tokio::test]
async fn refresh_rejects_a_bogus_token_and_an_access_token_used_as_a_refresh_token() {
    let (addr, http) = start_server("bogus").await;
    let base = format!("http://{addr}");

    let bogus = http
        .post(format!("{base}/api/auth/refresh"))
        .json(&serde_json::json!({"refresh_token": "not-a-real-token"}))
        .send()
        .await
        .unwrap();
    assert_eq!(bogus.status(), 401);

    // A real access token, presented where a refresh token belongs,
    // must be rejected -- `validate_token`'s `expected_type` check is
    // what `authenticate_headers` relies on elsewhere in this file, and
    // this confirms `refresh` applies the identical check rather than
    // accepting any well-signed token regardless of its `token_type`.
    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "All-Father", "password": "passvord0000"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let access_token = login["access_token"].as_str().unwrap().to_string();

    let wrong_type = http
        .post(format!("{base}/api/auth/refresh"))
        .json(&serde_json::json!({"refresh_token": access_token}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        wrong_type.status(),
        401,
        "an access token must not be usable as a refresh token"
    );
}

/// Proves the actual scenario this route exists for, deterministically
/// rather than by waiting a real hour: an access token that has
/// genuinely reached its real, signed `exp` claim is rejected by a real
/// authenticated endpoint, and the refresh token issued alongside it —
/// still within its own, much longer, validity window — successfully
/// redeems a fresh access token that immediately works again against
/// that same endpoint. This is the exact `GET /api/users/hierarchy`
/// call `mobile/lib/net/auth.dart`'s `fetchHierarchyJson` makes to
/// populate the local Task/Mission approval-authority cache
/// (`mobile_core_set_hierarchy`) — proving this endpoint keeps working
/// across a refresh is the server-side half of proving that cache
/// keeps working across one too; the Dart/FFI half cannot be exercised
/// in this sandbox (no Flutter/Dart toolchain — see `DECISIONS.md`).
///
/// Constructs the expired token itself (same signing key `ApiState::new`
/// defaults `ONYX_AUTHORITY_SIGNING_KEY` to outside production — see
/// `routes/mod.rs`'s own doc comment on that default) by decoding a
/// real, freshly-issued access token's real claims, rewriting only
/// `exp`/`iat` to the past, and re-signing with the same codec
/// `api-server` itself uses — so this is a genuinely valid signature
/// over a genuinely expired token, not a forged/malformed one, and not
/// a mock that could pass for a reason unrelated to expiry.
#[tokio::test]
async fn access_token_that_has_actually_expired_is_rejected_and_refresh_recovers() {
    const SIGNING_KEY: &str =
        "hex:4242424242424242424242424242424242424242424242424242424242424242";
    std::env::set_var("ONYX_AUTHORITY_SIGNING_KEY", SIGNING_KEY);

    let (addr, http) = start_server("expiry").await;
    let base = format!("http://{addr}");

    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "All-Father", "password": "passvord0000"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fresh_access_token = login["access_token"].as_str().unwrap().to_string();
    let refresh_token = login["refresh_token"].as_str().unwrap().to_string();

    // Confirm the fresh token actually works first, so the later 401 is
    // proven to be *because* of expiry, not some unrelated auth failure.
    let sanity_check = http
        .get(format!("{base}/api/users/hierarchy"))
        .bearer_auth(&fresh_access_token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        sanity_check.status(),
        200,
        "the freshly-issued access token must work before we deliberately expire it"
    );

    // Decode the real token's real claims, and re-sign an
    // otherwise-identical token whose `exp` (and `iat`, so `iat > now`
    // is never separately tripped) sit safely in the past.
    let secret_provider = security_adapter::EnvironmentSecretProvider;
    let secret = secret_provider
        .get("ONYX_AUTHORITY_SIGNING_KEY")
        .await
        .unwrap();
    let codec = Ed25519JwtCodec::from_rotating_secret(&secret).unwrap();
    let mut claims: TokenClaims = codec.decode(&fresh_access_token).unwrap();
    claims.iat = 0;
    claims.exp = 1; // 1970-01-01T00:00:01Z -- unambiguously, genuinely expired.
    let expired_access_token = codec.encode(&claims).unwrap();

    let expired_attempt = http
        .get(format!("{base}/api/users/hierarchy"))
        .bearer_auth(&expired_access_token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        expired_attempt.status(),
        401,
        "a genuinely expired access token must be rejected"
    );

    // The refresh token issued alongside it (7-day TTL, untouched by
    // the access token's much shorter 1-hour one) redeems a working
    // replacement.
    let refresh_response = http
        .post(format!("{base}/api/auth/refresh"))
        .json(&serde_json::json!({"refresh_token": refresh_token}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        refresh_response.status(),
        200,
        "the still-valid refresh token must successfully redeem a new access token"
    );
    let refreshed: serde_json::Value = refresh_response.json().await.unwrap();
    let new_access_token = refreshed["access_token"].as_str().unwrap();

    let recovered = http
        .get(format!("{base}/api/users/hierarchy"))
        .bearer_auth(new_access_token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        recovered.status(),
        200,
        "the refreshed access token must immediately work again against the same real endpoint mobile's hierarchy fetch uses"
    );
}
