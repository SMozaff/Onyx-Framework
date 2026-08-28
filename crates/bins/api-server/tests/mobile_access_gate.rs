//! Real, end-to-end proof that class-based mobile access control
//! actually gates `/api/auth/login`, over real HTTP (same harness
//! style as `query_id_normalization.rs`): restrictive by default (no
//! `mobile_class_access` row denies mobile login for that class), an
//! Admin always bypasses it, and a grant via
//! `PUT /api/admin/mobile-access` actually lifts the restriction for
//! ordinary users of that class.

use std::net::SocketAddr;

async fn start_server(db_label: &str) -> (SocketAddr, reqwest::Client) {
    let db_path = std::env::temp_dir().join(format!("onyx-mobile-access-test-{db_label}.db"));
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

async fn login(
    http: &reqwest::Client,
    base: &str,
    username: &str,
    password: &str,
    client_type: Option<&str>,
) -> reqwest::Response {
    let mut body = serde_json::json!({"username": username, "password": password});
    if let Some(ct) = client_type {
        body["client_type"] = serde_json::json!(ct);
    }
    http.post(format!("{base}/api/auth/login"))
        .json(&body)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn mobile_login_is_denied_by_default_then_allowed_once_granted_admin_always_allowed() {
    let (addr, http) = start_server("gate").await;
    let base = format!("http://{addr}");

    // ApiState::new seeds a fixed admin account ("All-Father" /
    // "passvord0000", see that seed's own doc comment in routes/mod.rs)
    // the moment the users table is empty, which it is for this
    // fresh-per-test database -- use that rather than the token-gated
    // `/api/admin/bootstrap` flow, which the seed leaves permanently
    // closed (`BOOTSTRAP_ALREADY_COMPLETED`) from the instant the
    // server starts against an empty store.
    let admin_login_resp = login(&http, &base, "All-Father", "passvord0000", None).await;
    assert_eq!(
        admin_login_resp.status(),
        200,
        "seeded admin login must succeed"
    );
    let admin_login: serde_json::Value = admin_login_resp.json().await.unwrap();
    let admin_token = admin_login["access_token"].as_str().unwrap().to_string();

    // Admin logging in with client_type "mobile" must succeed even
    // though no mobile_class_access row exists yet -- Admin bypasses
    // this gate entirely.
    let admin_mobile_login =
        login(&http, &base, "All-Father", "passvord0000", Some("mobile")).await;
    assert_eq!(
        admin_mobile_login.status(),
        200,
        "Admin must always be allowed to log in from mobile"
    );

    // Create an ordinary Staff user via the admin route, then assign
    // its class.
    let create: serde_json::Value = http
        .post(format!("{base}/api/admin/users"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"username": "mobile-staffer", "password": "mobile-staffer-password"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let staff_id = create["id"].as_str().expect("created user id").to_string();

    http.post(format!("{base}/api/admin/users/{staff_id}/class"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"class": "staff"}))
        .send()
        .await
        .unwrap();

    // Restrictive default: no mobile_class_access row for "staff" yet
    // -- mobile login must be denied with the specific error code,
    // even though the password is correct.
    let denied = login(
        &http,
        &base,
        "mobile-staffer",
        "mobile-staffer-password",
        Some("mobile"),
    )
    .await;
    assert_eq!(denied.status(), 403);
    let denied_body: serde_json::Value = denied.json().await.unwrap();
    assert_eq!(
        denied_body["error"]["code"],
        serde_json::json!("MOBILE_ACCESS_RESTRICTED")
    );

    // The same credentials with client_type "desktop" (or omitted)
    // must succeed -- the gate only applies to client_type "mobile".
    let desktop_login = login(
        &http,
        &base,
        "mobile-staffer",
        "mobile-staffer-password",
        Some("desktop"),
    )
    .await;
    assert_eq!(
        desktop_login.status(),
        200,
        "non-mobile client_type must never be gated"
    );

    // Confirm GET /api/admin/mobile-access starts empty (restrictive
    // default is visible through the read endpoint too, not just
    // inferred from the login denial).
    let initial_access: serde_json::Value = http
        .get(format!("{base}/api/admin/mobile-access"))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        initial_access["allowed_classes"],
        serde_json::json!(Vec::<String>::new())
    );

    // Grant "staff" mobile access via the admin route.
    let grant = http
        .put(format!("{base}/api/admin/mobile-access"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"allowed_classes": ["staff"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(grant.status(), 200);

    // Mobile login for the same Staff user must now succeed.
    let allowed = login(
        &http,
        &base,
        "mobile-staffer",
        "mobile-staffer-password",
        Some("mobile"),
    )
    .await;
    assert_eq!(
        allowed.status(),
        200,
        "staff must be allowed to log in from mobile once granted"
    );

    // A non-admin cannot read or write the mobile-access grant list.
    let staff_token: serde_json::Value = allowed.json().await.unwrap();
    let staff_bearer = staff_token["access_token"].as_str().unwrap();
    let forbidden = http
        .get(format!("{base}/api/admin/mobile-access"))
        .bearer_auth(staff_bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403);
}

/// The exact scenario requested when this piece was verified: an org
/// with `Staff` excluded from mobile access (never granted a row) and
/// `Supervisor` granted from the outset. `Staff` must be denied on
/// `client_type: "mobile"` but allowed on `client_type: "desktop"`;
/// `Supervisor` must be allowed on both.
#[tokio::test]
async fn excluded_class_denied_on_mobile_allowed_on_desktop_granted_class_allowed_on_both() {
    let (addr, http) = start_server("gate-two-classes").await;
    let base = format!("http://{addr}");

    let admin_login: serde_json::Value = login(&http, &base, "All-Father", "passvord0000", None)
        .await
        .json()
        .await
        .unwrap();
    let admin_token = admin_login["access_token"].as_str().unwrap().to_string();

    // Grant only "supervisor" mobile access up front; "staff" is never
    // granted at all.
    let grant = http
        .put(format!("{base}/api/admin/mobile-access"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"allowed_classes": ["supervisor"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(grant.status(), 200);

    async fn create_user_with_class(
        http: &reqwest::Client,
        base: &str,
        admin_token: &str,
        username: &str,
        password: &str,
        class: &str,
    ) {
        let create: serde_json::Value = http
            .post(format!("{base}/api/admin/users"))
            .bearer_auth(admin_token)
            .json(&serde_json::json!({"username": username, "password": password}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let id = create["id"].as_str().expect("created user id").to_string();
        http.post(format!("{base}/api/admin/users/{id}/class"))
            .bearer_auth(admin_token)
            .json(&serde_json::json!({"class": class}))
            .send()
            .await
            .unwrap();
    }

    create_user_with_class(
        &http,
        &base,
        &admin_token,
        "two-class-staffer",
        "two-class-staffer-pw",
        "staff",
    )
    .await;
    create_user_with_class(
        &http,
        &base,
        &admin_token,
        "two-class-supervisor",
        "two-class-supervisor-pw",
        "supervisor",
    )
    .await;

    // Excluded class ("staff"): denied on mobile, allowed on desktop.
    let staff_mobile = login(
        &http,
        &base,
        "two-class-staffer",
        "two-class-staffer-pw",
        Some("mobile"),
    )
    .await;
    assert_eq!(
        staff_mobile.status(),
        403,
        "excluded class must be denied on client_type: mobile"
    );
    let staff_desktop = login(
        &http,
        &base,
        "two-class-staffer",
        "two-class-staffer-pw",
        Some("desktop"),
    )
    .await;
    assert_eq!(
        staff_desktop.status(),
        200,
        "excluded class must still succeed on client_type: desktop"
    );

    // Granted class ("supervisor"): allowed on both.
    let supervisor_mobile = login(
        &http,
        &base,
        "two-class-supervisor",
        "two-class-supervisor-pw",
        Some("mobile"),
    )
    .await;
    assert_eq!(
        supervisor_mobile.status(),
        200,
        "granted class must succeed on client_type: mobile"
    );
    let supervisor_desktop = login(
        &http,
        &base,
        "two-class-supervisor",
        "two-class-supervisor-pw",
        Some("desktop"),
    )
    .await;
    assert_eq!(
        supervisor_desktop.status(),
        200,
        "granted class must also succeed on client_type: desktop"
    );
}
