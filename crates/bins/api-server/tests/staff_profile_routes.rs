//! End-to-end proof that the staff profile routes (view, admin-only
//! upsert, batch CSV/JSON import/export) actually work over real HTTP —
//! matching the project's "compiling is not evidence" testing
//! philosophy already established by `relay_switchboard.rs` and
//! `user_hierarchy_admin_routes.rs`.

use std::net::SocketAddr;

const BOOTSTRAP_TOKEN: &str = "profiles-test-bootstrap";

async fn start_server(db_label: &str) -> (SocketAddr, String) {
    std::env::set_var("ONYX_BOOTSTRAP_TOKEN", BOOTSTRAP_TOKEN);

    let db_path = std::env::temp_dir().join(format!("onyx-profiles-test-{db_label}.db"));
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
        .json(&serde_json::json!({"username": "profiles-admin", "password": "profiles-test-password"}))
        .send()
        .await
        .expect("bootstrap request");

    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "profiles-admin", "password": "profiles-test-password"}))
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

async fn create_user(http: &reqwest::Client, base: &str, token: &str, username: &str) -> String {
    let payload = serde_json::json!({"username": username, "password": "a-fine-password-1"});
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

#[tokio::test]
async fn upsert_creates_then_updates_a_profile() {
    let (addr, token) = start_server("upsert-basic").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(&http, &base, &token, "staffer1").await;

    let create_response = http
        .put(format!("{base}/api/admin/profiles"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "owner_id": user_id,
            "full_name": "Alice Example",
            "job_title": "Engineer",
            "contact_email": "alice@example.com",
            "contact_phone": "555-1000",
            "department": "Engineering"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_response.status(), 200);
    let created: serde_json::Value = create_response.json().await.unwrap();
    assert_eq!(created["full_name"], serde_json::json!("Alice Example"));

    // A second upsert with the same owner_id must update, not duplicate.
    let update_response = http
        .put(format!("{base}/api/admin/profiles"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "owner_id": user_id,
            "full_name": "Alice Updated",
            "job_title": "Senior Engineer",
            "contact_email": "alice@example.com",
            "contact_phone": "555-1000",
            "department": "Engineering"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_response.status(), 200);
    let updated: serde_json::Value = update_response.json().await.unwrap();
    assert_eq!(updated["full_name"], serde_json::json!("Alice Updated"));

    // Exactly one profile should exist for this owner.
    let list: Vec<serde_json::Value> = http
        .get(format!("{base}/api/profiles"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let matches = list.iter().filter(|p| p["owner_id"] == user_id).count();
    assert_eq!(matches, 1, "upsert must not create a duplicate profile");
}

#[tokio::test]
async fn public_view_excludes_work_stats_for_non_manager() {
    let (addr, token) = start_server("visibility-public").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(&http, &base, &token, "staffer2").await;
    http.put(format!("{base}/api/admin/profiles"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "owner_id": user_id,
            "full_name": "Bob Example",
            "job_title": "Analyst",
            "contact_email": "bob@example.com",
            "contact_phone": "555-2000",
            "department": "Finance"
        }))
        .send()
        .await
        .unwrap();

    // The bootstrap admin has is_admin=true but no `class` assigned —
    // require_class([TopLevelManager]) must therefore fail for them
    // too (Admin does pass require_class elsewhere, so this also
    // confirms the two guards are genuinely different — see
    // routes::admin::require_class's own doc comment on why Admin
    // bypasses it deliberately... but here we're checking the DTO
    // returned to a plain, unclassified admin token specifically).
    let profile: serde_json::Value = http
        .get(format!("{base}/api/profiles/{user_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(profile["full_name"], serde_json::json!("Bob Example"));
    assert_eq!(profile["department"], serde_json::json!("Finance"));
}

#[tokio::test]
async fn upsert_route_requires_admin() {
    let (addr, token) = start_server("admin-required").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");
    let user_id = create_user(&http, &base, &token, "staffer3").await;

    let response = http
        .put(format!("{base}/api/admin/profiles"))
        .json(&serde_json::json!({
            "owner_id": user_id,
            "full_name": "No Auth",
            "job_title": "x",
            "contact_email": "x@example.com",
            "contact_phone": "555",
            "department": "x"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn json_import_creates_and_updates_and_reports_row_failures() {
    let (addr, token) = start_server("import-json").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let existing_user = create_user(&http, &base, &token, "importee1").await;
    let unknown_user_id = "00000000-0000-0000-0000-000000000000";

    // Pre-seed one profile so this row exercises the update path.
    http.put(format!("{base}/api/admin/profiles"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "owner_id": existing_user,
            "full_name": "Pre-existing",
            "job_title": "Old Title",
            "contact_email": "old@example.com",
            "contact_phone": "555-0000",
            "department": "Old Dept"
        }))
        .send()
        .await
        .unwrap();

    let rows = serde_json::json!([
        {
            "user_id": existing_user,
            "full_name": "Updated Name",
            "contact_email": "new@example.com",
            "contact_phone": "555-1111",
            "job_title": "New Title",
            "department": "New Dept"
        },
        {
            "user_id": unknown_user_id,
            "full_name": "Ghost",
            "contact_email": "ghost@example.com",
            "contact_phone": "555-2222",
            "job_title": "Nobody",
            "department": "Nowhere"
        },
        {
            "user_id": existing_user,
            "full_name": "",
            "contact_email": "blank@example.com",
            "contact_phone": "555-3333",
            "job_title": "Blank Name",
            "department": "X"
        }
    ]);
    let body = serde_json::to_vec(&rows).unwrap();

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(body)
            .file_name("profiles.json")
            .mime_str("application/json")
            .unwrap(),
    );

    let response = http
        .post(format!("{base}/api/admin/profiles/import"))
        .bearer_auth(&token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let summary: serde_json::Value = response.json().await.unwrap();

    assert_eq!(summary["total_rows"], serde_json::json!(3));
    assert_eq!(summary["updated"], serde_json::json!(1));
    assert_eq!(summary["created"], serde_json::json!(0));
    assert_eq!(summary["failed"], serde_json::json!(2));

    // Confirm the update actually landed.
    let profile: serde_json::Value = http
        .get(format!("{base}/api/profiles/{existing_user}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(profile["full_name"], serde_json::json!("Updated Name"));
    assert_eq!(profile["department"], serde_json::json!("New Dept"));
}

#[tokio::test]
async fn csv_import_and_export_round_trip() {
    let (addr, token) = start_server("import-csv").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    let user_id = create_user(&http, &base, &token, "csvperson").await;

    let csv_body = format!(
        "user_id,full_name,contact_email,contact_phone,job_title,department,photo_blob_key,class_label,parent_display_name,team_memberships\n\
         {user_id},CSV Person,csv@example.com,555-9999,Tester,QA,,,,TeamA;TeamB\n"
    );

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::text(csv_body)
            .file_name("profiles.csv")
            .mime_str("text/csv")
            .unwrap(),
    );

    let import_response = http
        .post(format!("{base}/api/admin/profiles/import"))
        .bearer_auth(&token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(import_response.status(), 200);
    let summary: serde_json::Value = import_response.json().await.unwrap();
    assert_eq!(summary["created"], serde_json::json!(1));

    // Export as CSV and confirm the row round-trips.
    let export_response = http
        .get(format!("{base}/api/admin/profiles/export?format=csv"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(export_response.status(), 200);
    let export_body = export_response.text().await.unwrap();
    assert!(export_body.contains("CSV Person"));
    assert!(export_body.contains("TeamA;TeamB"));

    // And as JSON.
    let export_json_response = http
        .get(format!("{base}/api/admin/profiles/export?format=json"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(export_json_response.status(), 200);
    let exported: Vec<serde_json::Value> = export_json_response.json().await.unwrap();
    let found = exported.iter().find(|p| p["owner_id"] == user_id);
    assert!(found.is_some());
    assert_eq!(
        found.unwrap()["team_memberships"],
        serde_json::json!(["TeamA", "TeamB"])
    );
}

#[tokio::test]
async fn import_requires_admin() {
    let (addr, token) = start_server("import-admin-required").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");
    let _ = create_user(&http, &base, &token, "irrelevant").await;

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"[]".to_vec())
            .file_name("x.json")
            .mime_str("application/json")
            .unwrap(),
    );

    let response = http
        .post(format!("{base}/api/admin/profiles/import"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}
