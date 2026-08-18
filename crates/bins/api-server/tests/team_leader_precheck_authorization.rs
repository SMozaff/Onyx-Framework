//! End-to-end proof that `RecordTeamLeaderPreCheck` is actually gated
//! to Team Leaders (or Admins) over real HTTP — not just that the code
//! compiles. Mirrors `user_hierarchy_admin_routes.rs`'s server-bootstrap
//! shape and "compiling is not evidence" philosophy.
//!
//! # Provenance
//! Added 2026-08-16 alongside `require_team_leader_or_admin`
//! (`routes::command`), which closed a real gap: `RecordTeamLeaderPreCheck`
//! previously had no class-based authorization check at all — any
//! authenticated user could record a pre-check on any list, despite
//! `UserClass::TeamLeader`'s own doc comment describing this as "real,
//! standing... authority" specific to that class. See `DECISIONS.md`
//! for the full account.

use std::net::SocketAddr;

const BOOTSTRAP_TOKEN: &str = "team-leader-precheck-test-bootstrap";

/// Boots an api-server on an ephemeral port against a throwaway SQLite
/// file, bootstraps an admin, and returns the address plus a valid
/// access token. Same shape as `user_hierarchy_admin_routes.rs`'s
/// helper of the same name — kept as a separate copy since these are
/// independent test binaries, matching that file's own precedent.
async fn start_server(db_label: &str) -> (SocketAddr, String) {
    std::env::set_var("ONYX_BOOTSTRAP_TOKEN", BOOTSTRAP_TOKEN);

    let db_path =
        std::env::temp_dir().join(format!("onyx-team-leader-precheck-test-{db_label}.db"));
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
        .json(&serde_json::json!({"username": "precheck-admin", "password": "precheck-test-password"}))
        .send()
        .await
        .expect("bootstrap request");

    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "precheck-admin", "password": "precheck-test-password"}))
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

/// Creates a user via the admin API (optionally with a `class`) and
/// returns `(id, access_token)`.
async fn create_user_with_token(
    http: &reqwest::Client,
    base: &str,
    admin_token: &str,
    username: &str,
    class: Option<&str>,
) -> (String, String) {
    let mut payload = serde_json::json!({"username": username, "password": "a-fine-password-1"});
    if let Some(class) = class {
        payload["class"] = serde_json::json!(class);
    }
    let created: serde_json::Value = http
        .post(format!("{base}/api/admin/users"))
        .bearer_auth(admin_token)
        .json(&payload)
        .send()
        .await
        .expect("create user request")
        .json()
        .await
        .expect("create user body");
    let id = created["id"].as_str().expect("user id").to_string();

    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": username, "password": "a-fine-password-1"}))
        .send()
        .await
        .expect("login request")
        .json()
        .await
        .expect("login body");
    let token = login["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();

    (id, token)
}

const ORG_ID: &str = "11111111-1111-1111-1111-111111111111";

async fn create_and_submit_todo_list(
    http: &reqwest::Client,
    base: &str,
    admin_token: &str,
    owner_id: &str,
) -> String {
    let created: serde_json::Value = http
        .post(format!("{base}/api/todo/lists"))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "owner": owner_id,
            "origin": "staff_authored",
            "items": [{"description": "Do the thing"}],
        }))
        .send()
        .await
        .expect("create todo list request")
        .json()
        .await
        .expect("create todo list body");
    let todo_list_id = created["todo_list_id"]
        .as_str()
        .expect("todo_list_id")
        .to_string();

    let submit_payload = serde_json::json!({
        "command_id": uuid_v4(),
        "operation_id": uuid_v4(),
        "command_type": "todo_list.SubmitTodoList",
        "schema_version": "1.0",
        "target": {"id": todo_list_id, "type": "todo_list", "organization_id": ORG_ID},
        "expected_version": 0,
        "expected_lifecycle_epoch": 0,
        "expected_authority_epoch": 0,
        "issued_at": "2026-08-16T00:00:00Z",
        "correlation_id": uuid_v4(),
        "payload": "SubmitTodoList",
    });
    let response = http
        .post(format!("{base}/api/command"))
        .bearer_auth(admin_token)
        .json(&submit_payload)
        .send()
        .await
        .expect("submit request");
    assert!(
        response.status().is_success(),
        "submit must succeed to set up this test"
    );

    todo_list_id
}

fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn precheck_payload(todo_list_id: &str, notes: &str) -> serde_json::Value {
    serde_json::json!({
        "command_id": uuid_v4(),
        "operation_id": uuid_v4(),
        "command_type": "todo_list.RecordTeamLeaderPreCheck",
        "schema_version": "1.0",
        "target": {"id": todo_list_id, "type": "todo_list", "organization_id": ORG_ID},
        "expected_version": 1,
        "expected_lifecycle_epoch": 0,
        "expected_authority_epoch": 0,
        "issued_at": "2026-08-16T00:00:00Z",
        "correlation_id": uuid_v4(),
        "payload": {"RecordTeamLeaderPreCheck": {"notes": notes}},
    })
}

#[tokio::test]
async fn user_with_no_class_cannot_record_a_pre_check() {
    let (addr, admin_token) = start_server("no-class-rejected").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let (staff_id, _staff_token) =
        create_user_with_token(&http, &base, &admin_token, "staff-a", None).await;
    let (_, rando_token) =
        create_user_with_token(&http, &base, &admin_token, "rando-a", None).await;

    let todo_list_id = create_and_submit_todo_list(&http, &base, &admin_token, &staff_id).await;

    let response = http
        .post(format!("{base}/api/command"))
        .bearer_auth(&rando_token)
        .json(&precheck_payload(&todo_list_id, "sneaky notes"))
        .send()
        .await
        .expect("precheck request");

    let body: serde_json::Value = response.json().await.expect("response body");
    assert_eq!(
        body["error"]["safe_details"]["message"],
        serde_json::json!("only a Team Leader (or Admin) may record a pre-check")
    );
}

#[tokio::test]
async fn team_leader_can_record_a_pre_check() {
    let (addr, admin_token) = start_server("team-leader-accepted").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let (staff_id, _staff_token) =
        create_user_with_token(&http, &base, &admin_token, "staff-b", None).await;
    let (_, leader_token) =
        create_user_with_token(&http, &base, &admin_token, "leader-b", Some("team_leader")).await;

    let todo_list_id = create_and_submit_todo_list(&http, &base, &admin_token, &staff_id).await;

    let response = http
        .post(format!("{base}/api/command"))
        .bearer_auth(&leader_token)
        .json(&precheck_payload(&todo_list_id, "looks fine"))
        .send()
        .await
        .expect("precheck request");

    assert!(
        response.status().is_success(),
        "a real Team Leader must be able to record a pre-check"
    );
    let body: serde_json::Value = response.json().await.expect("response body");
    assert_eq!(body["success"], serde_json::json!(true));
}

#[tokio::test]
async fn admin_can_record_a_pre_check_without_a_team_leader_class() {
    // Admin is a practical superset for this gate too, matching every
    // other class-based check in this codebase (require_class's own
    // "Admin always passes regardless of `allowed`" rule).
    let (addr, admin_token) = start_server("admin-accepted").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let (staff_id, _staff_token) =
        create_user_with_token(&http, &base, &admin_token, "staff-c", None).await;
    let todo_list_id = create_and_submit_todo_list(&http, &base, &admin_token, &staff_id).await;

    let response = http
        .post(format!("{base}/api/command"))
        .bearer_auth(&admin_token)
        .json(&precheck_payload(&todo_list_id, "admin override"))
        .send()
        .await
        .expect("precheck request");

    assert!(
        response.status().is_success(),
        "Admin must be able to record a pre-check"
    );
}
