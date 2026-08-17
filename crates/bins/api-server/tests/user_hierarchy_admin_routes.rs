//! End-to-end proof that the Phase A (User Hierarchy) admin routes
//! actually work over real HTTP against a real server — not just that
//! they compile. Mirrors `relay_switchboard.rs`'s "compiling is not
//! evidence" philosophy and reuses its server-bootstrap shape.

use std::net::SocketAddr;

const BOOTSTRAP_TOKEN: &str = "user-hierarchy-test-bootstrap";

/// Boots an api-server on an ephemeral port against a throwaway SQLite
/// file, bootstraps an admin, and returns the address plus a valid
/// access token. Nearly identical to `relay_switchboard.rs`'s helper of
/// the same name — kept as a separate copy rather than shared, since
/// these are two independent test binaries (integration tests each
/// compile as their own crate) and the duplication is small.
async fn start_server(db_label: &str) -> (SocketAddr, String) {
    std::env::set_var("ONYX_BOOTSTRAP_TOKEN", BOOTSTRAP_TOKEN);

    let db_path = std::env::temp_dir().join(format!("onyx-user-hierarchy-test-{db_label}.db"));
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

    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    http.post(format!("{base}/api/admin/bootstrap"))
        .header("x-onyx-bootstrap-token", BOOTSTRAP_TOKEN)
        .json(&serde_json::json!({"username": "hierarchy-admin", "password": "hierarchy-test-password"}))
        .send()
        .await
        .expect("bootstrap request");

    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "hierarchy-admin", "password": "hierarchy-test-password"}))
        .send()
        .await
        .expect("login request")
        .json()
        .await
        .expect("login body");

    let token = login["access_token"]
        .as_str()
        .expect("access_token in login response")
        .to_string();

    (addr, token)
}

/// Creates a user via the admin API and returns its id.
async fn create_user(
    http: &reqwest::Client,
    base: &str,
    token: &str,
    username: &str,
    extra: serde_json::Value,
) -> String {
    let mut payload = serde_json::json!({"username": username, "password": "a-fine-password-1"});
    payload
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());

    let response: serde_json::Value = http
        .post(format!("{base}/api/admin/users"))
        .bearer_auth(token)
        .json(&payload)
        .send()
        .await
        .expect("create user request")
        .json()
        .await
        .expect("create user body");

    response["id"]
        .as_str()
        .unwrap_or_else(|| panic!("no id in create-user response: {response:?}"))
        .to_string()
}

async fn login_token(http: &reqwest::Client, base: &str, username: &str) -> String {
    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": username, "password": "a-fine-password-1"}))
        .send()
        .await
        .expect("login request")
        .json()
        .await
        .expect("login body");
    login["access_token"]
        .as_str()
        .expect("access token")
        .to_string()
}

#[tokio::test]
async fn class_can_be_set_at_creation_and_read_back() {
    let (addr, token) = start_server("class-creation").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(
        &http,
        &base,
        &token,
        "leader1",
        serde_json::json!({"class": "team_leader"}),
    )
    .await;

    let users: Vec<serde_json::Value> = http
        .get(format!("{base}/api/admin/users"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let created = users
        .iter()
        .find(|u| u["id"] == user_id)
        .expect("created user present in listing");
    assert_eq!(created["class"], serde_json::json!("team_leader"));
}

#[tokio::test]
async fn set_class_updates_and_can_clear_to_null() {
    let (addr, token) = start_server("class-update").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(&http, &base, &token, "staffer1", serde_json::json!({})).await;

    let response = http
        .post(format!("{base}/api/admin/users/{user_id}/class"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"class": "senior_manager"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let users: Vec<serde_json::Value> = http
        .get(format!("{base}/api/admin/users"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let updated = users.iter().find(|u| u["id"] == user_id).unwrap();
    assert_eq!(updated["class"], serde_json::json!("senior_manager"));

    // Explicit null clears it — see SetClassRequest's doc comment on why
    // this is not the same as omitting the field.
    http.post(format!("{base}/api/admin/users/{user_id}/class"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"class": null}))
        .send()
        .await
        .unwrap();

    let users_after_clear: Vec<serde_json::Value> = http
        .get(format!("{base}/api/admin/users"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cleared = users_after_clear
        .iter()
        .find(|u| u["id"] == user_id)
        .unwrap();
    assert_eq!(cleared["class"], serde_json::Value::Null);
}

#[tokio::test]
async fn set_class_rejects_unrecognized_value() {
    let (addr, token) = start_server("class-invalid").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(&http, &base, &token, "staffer2", serde_json::json!({})).await;

    let response = http
        .post(format!("{base}/api/admin/users/{user_id}/class"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"class": "team-leader-but-with-a-typo"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn set_parent_links_and_reports_in_listing() {
    let (addr, token) = start_server("parent-link").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let manager_id = create_user(&http, &base, &token, "manager1", serde_json::json!({})).await;
    let staff_id = create_user(&http, &base, &token, "staff1", serde_json::json!({})).await;

    let response = http
        .post(format!("{base}/api/admin/users/{staff_id}/parent"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"parent_user_id": manager_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let users: Vec<serde_json::Value> = http
        .get(format!("{base}/api/admin/users"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let staff = users.iter().find(|u| u["id"] == staff_id).unwrap();
    assert_eq!(staff["parent_user_id"], serde_json::json!(manager_id));
}

#[tokio::test]
async fn set_parent_rejects_self_reference_over_http() {
    let (addr, token) = start_server("parent-self").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(&http, &base, &token, "loner", serde_json::json!({})).await;

    let response = http
        .post(format!("{base}/api/admin/users/{user_id}/parent"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"parent_user_id": user_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn set_parent_rejects_nonexistent_parent_over_http() {
    let (addr, token) = start_server("parent-missing").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(&http, &base, &token, "solo1", serde_json::json!({})).await;

    let response = http
        .post(format!("{base}/api/admin/users/{user_id}/parent"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"parent_user_id": "00000000-0000-0000-0000-000000000000"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn class_and_parent_routes_reject_unauthenticated_requests() {
    let (addr, token) = start_server("auth-required").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(&http, &base, &token, "target1", serde_json::json!({})).await;

    let class_response = http
        .post(format!("{base}/api/admin/users/{user_id}/class"))
        .json(&serde_json::json!({"class": "staff"}))
        .send()
        .await
        .unwrap();
    assert_eq!(class_response.status(), 401);

    let parent_response = http
        .post(format!("{base}/api/admin/users/{user_id}/parent"))
        .json(&serde_json::json!({"parent_user_id": null}))
        .send()
        .await
        .unwrap();
    assert_eq!(parent_response.status(), 401);
}

#[tokio::test]
async fn authenticated_user_can_list_reduced_active_picker_identities() {
    let (addr, admin_token) = start_server("picker-identities").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let picker_user_id = create_user(
        &http,
        &base,
        &admin_token,
        "picker-staff",
        serde_json::json!({}),
    )
    .await;
    let inactive_user_id = create_user(
        &http,
        &base,
        &admin_token,
        "picker-inactive",
        serde_json::json!({}),
    )
    .await;
    let foreign_user_id = create_user(
        &http,
        &base,
        &admin_token,
        "picker-foreign",
        serde_json::json!({"organization_id": "33333333-3333-3333-3333-333333333333"}),
    )
    .await;

    let deactivation = http
        .post(format!(
            "{base}/api/admin/users/{inactive_user_id}/deactivate"
        ))
        .bearer_auth(&admin_token)
        .send()
        .await
        .expect("deactivate request");
    assert!(deactivation.status().is_success());

    let staff_token = login_token(&http, &base, "picker-staff").await;
    let response = http
        .get(format!("{base}/api/users"))
        .bearer_auth(&staff_token)
        .send()
        .await
        .expect("picker users request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let users: Vec<serde_json::Value> = response.json().await.expect("picker users body");

    let picker = users
        .iter()
        .find(|user| user["id"] == picker_user_id)
        .expect("ordinary authenticated user is visible");
    assert_eq!(picker["username"], serde_json::json!("picker-staff"));
    assert!(picker.get("is_admin").is_none());
    assert!(picker.get("class").is_none());
    assert!(picker.get("parent_user_id").is_none());
    assert!(users.iter().all(|user| user["id"] != inactive_user_id));
    assert!(users.iter().all(|user| user["id"] != foreign_user_id));

    let unauthenticated = http
        .get(format!("{base}/api/users"))
        .send()
        .await
        .expect("unauthenticated picker request");
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
}
