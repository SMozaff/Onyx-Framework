//! H10/P1.5 — real, end-to-end proof that the `mobile_observer`
//! capability ceiling is enforced server-side, over real HTTP (same
//! harness style as `relay_switchboard.rs`/`staff_loan_authorization.rs`),
//! not merely designed to be.
//!
//! This is the release gate ONYX-MOB-01 §28's negative capability
//! matrix and ONYX-MOB-00 §24's verification law both require:
//! "every prohibited operational mutation MUST remain prohibited when
//! attempted directly against the backend using a valid
//! `mobile_observer` session." The seeded test-drive admin
//! ("All-Father") is used deliberately for the mutation-denial tests
//! below, not a lesser-privileged user, to prove ONYX-MOB-00 §4's
//! strongest claim directly: "a highly privileged administrator using
//! the PWA still receives only observer capabilities through that
//! client class."

use std::net::SocketAddr;

async fn start_server(db_label: &str) -> (SocketAddr, String) {
    let db_path = std::env::temp_dir().join(format!("onyx-mobile-observer-test-{db_label}.db"));
    let _ = std::fs::remove_file(&db_path);
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let state = api_server::routes::ApiState::new(&database_url)
        .await
        .expect("api state");
    let app = api_server::routes::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let addr = listener.local_addr().expect("listener address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (addr, format!("http://{addr}"))
}

/// Logs in the seeded test-drive admin with a given `client_type` (or
/// none, when `client_type` is `None`), returning `(access_token,
/// refresh_token)`.
async fn login_as(
    http: &reqwest::Client,
    base: &str,
    client_type: Option<&str>,
) -> (String, String) {
    let mut body = serde_json::json!({"username": "All-Father", "password": "passvord0000"});
    if let Some(ct) = client_type {
        body["client_type"] = serde_json::json!(ct);
    }
    let response: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&body)
        .send()
        .await
        .expect("login request")
        .json()
        .await
        .expect("login body");
    (
        response["access_token"]
            .as_str()
            .expect("access_token")
            .to_string(),
        response["refresh_token"]
            .as_str()
            .expect("refresh_token")
            .to_string(),
    )
}

fn assert_capability_denied(body: &serde_json::Value, expected_capability: &str) {
    let error = &body["error"];
    assert_eq!(error["code"], "CLIENT_CAPABILITY_DENIED");
    assert_eq!(error["safe_details"]["client_type"], "mobile_observer");
    assert_eq!(
        error["safe_details"]["required_capability"],
        expected_capability
    );
}

fn command_envelope(
    target_id: &str,
    target_type: &str,
    organization_id: &str,
    command_type: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "command_id": uuid::Uuid::new_v4().to_string(),
        "operation_id": uuid::Uuid::new_v4().to_string(),
        "command_type": command_type,
        "schema_version": "1.0",
        "target": {"id": target_id, "type": target_type, "organization_id": organization_id},
        "expected_version": 0,
        "expected_lifecycle_epoch": 0,
        "expected_authority_epoch": 0,
        "issued_at": "2026-08-31T00:00:00Z",
        "correlation_id": uuid::Uuid::new_v4().to_string(),
        "payload": payload,
    })
}

fn org_id() -> String {
    api_server::routes::ORGANIZATION_ID.to_string()
}

/// The single most important assertion in this file: `mobile_observer`
/// still reads normally, but every real mutation-class endpoint this
/// task audited denies it with the deterministic
/// `CLIENT_CAPABILITY_DENIED` shape — for the seeded *administrator*
/// account, proving the ceiling is not merely a non-admin restriction.
#[tokio::test]
async fn mobile_observer_reads_normally_but_every_mutation_endpoint_denies_it() {
    let (_addr, base) = start_server("matrix").await;
    let http = reqwest::Client::new();
    let (observer_token, _) = login_as(&http, &base, Some("mobile_observer")).await;

    // --- Read still works. ---
    let hierarchy = http
        .get(format!("{base}/api/users/hierarchy"))
        .bearer_auth(&observer_token)
        .send()
        .await
        .expect("hierarchy request");
    assert_eq!(
        hierarchy.status(),
        200,
        "an authorized read must still succeed under mobile_observer"
    );

    // --- /api/command: single gate covers notification/approval/policy/
    // legal_hold/todo_list/target_list/staff_loan dispatch. ---
    let command_response = http
        .post(format!("{base}/api/command"))
        .bearer_auth(&observer_token)
        .json(&command_envelope(
            &uuid::Uuid::new_v4().to_string(),
            "notification",
            &org_id(),
            "notification.Acknowledge",
            serde_json::json!({}),
        ))
        .send()
        .await
        .expect("command request");
    assert_eq!(command_response.status(), 403);
    assert_capability_denied(
        &command_response.json().await.expect("command body"),
        "submit_domain_command",
    );

    // --- /api/relay-ticket (P2P-2): minting requires `submit_domain_command`,
    // which the observer ceiling sets to false. ---
    let relay_ticket_response = http
        .post(format!("{base}/api/relay-ticket"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({
            "target_id": uuid::Uuid::new_v4().to_string(),
            "self_replica": uuid::Uuid::new_v4().to_string(),
        }))
        .send()
        .await
        .expect("relay ticket request");
    assert_eq!(relay_ticket_response.status(), 403);
    assert_capability_denied(
        &relay_ticket_response
            .json()
            .await
            .expect("relay ticket body"),
        "submit_domain_command",
    );

    // --- /api/todo/lists, /api/todo/targets, /api/todo/staff-loans:
    // create()-routed domain commands with their own REST entry points
    // (see routes::todo_admin's module doc comment for why). ---
    let todo_list_response = http
        .post(format!("{base}/api/todo/lists"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({
            "owner": uuid::Uuid::new_v4().to_string(),
            "origin": "staff_authored",
        }))
        .send()
        .await
        .expect("todo list request");
    assert_eq!(todo_list_response.status(), 403);
    assert_capability_denied(
        &todo_list_response.json().await.expect("todo list body"),
        "submit_domain_command",
    );

    let target_list_response = http
        .post(format!("{base}/api/todo/targets"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({
            "owner": uuid::Uuid::new_v4().to_string(),
            "origin": "staff_authored",
            "description": "denied",
            "time_window": {"start_at_ms": 0, "end_at_ms": 1},
        }))
        .send()
        .await
        .expect("target list request");
    assert_eq!(target_list_response.status(), 403);
    assert_capability_denied(
        &target_list_response.json().await.expect("target list body"),
        "submit_domain_command",
    );

    let staff_loan_response = http
        .post(format!("{base}/api/todo/staff-loans"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({
            "staff_user_id": uuid::Uuid::new_v4().to_string(),
            "real_owner_id": uuid::Uuid::new_v4().to_string(),
            "borrowing_manager_id": uuid::Uuid::new_v4().to_string(),
            "start_at_ms": 0,
            "end_at_ms": 1,
        }))
        .send()
        .await
        .expect("staff loan request");
    assert_eq!(staff_loan_response.status(), 403);
    assert_capability_denied(
        &staff_loan_response.json().await.expect("staff loan body"),
        "submit_domain_command",
    );

    // --- /api/admin/*: `can_administer`. Proven with the seeded
    // *admin* account's own observer session — ONYX-MOB-00 §4's
    // strongest claim. ---
    let create_user_response = http
        .post(format!("{base}/api/admin/users"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({"username": "should-not-exist", "password": "irrelevant-1"}))
        .send()
        .await
        .expect("create user request");
    assert_eq!(
        create_user_response.status(),
        403,
        "an administrator authenticated through mobile_observer must still be denied a mutation"
    );
    assert_capability_denied(
        &create_user_response.json().await.expect("create user body"),
        "administer",
    );

    let deactivate_response = http
        .post(format!(
            "{base}/api/admin/users/{}/deactivate",
            uuid::Uuid::new_v4()
        ))
        .bearer_auth(&observer_token)
        .send()
        .await
        .expect("deactivate request");
    assert_eq!(deactivate_response.status(), 403);
    assert_capability_denied(
        &deactivate_response.json().await.expect("deactivate body"),
        "administer",
    );

    let set_mobile_access_response = http
        .put(format!("{base}/api/admin/mobile-access"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({"allowed_classes": []}))
        .send()
        .await
        .expect("mobile-access request");
    assert_eq!(set_mobile_access_response.status(), 403);
    assert_capability_denied(
        &set_mobile_access_response
            .json()
            .await
            .expect("mobile-access body"),
        "administer",
    );

    let upsert_profile_response = http
        .put(format!("{base}/api/admin/profiles"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({
            "owner_id": uuid::Uuid::new_v4().to_string(),
            "full_name": "denied",
        }))
        .send()
        .await
        .expect("profile upsert request");
    assert_eq!(upsert_profile_response.status(), 403);
    assert_capability_denied(
        &upsert_profile_response
            .json()
            .await
            .expect("profile upsert body"),
        "administer",
    );

    let create_policy_response = http
        .post(format!("{base}/api/admin/policies"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({"name": "denied-policy"}))
        .send()
        .await
        .expect("create policy request");
    assert_eq!(create_policy_response.status(), 403);
    assert_capability_denied(
        &create_policy_response
            .json()
            .await
            .expect("create policy body"),
        "administer",
    );

    let legal_hold_response = http
        .post(format!("{base}/api/admin/legal-holds"))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({
            "target_id": uuid::Uuid::new_v4().to_string(),
            "target_type": "mission",
            "reason": "denied",
        }))
        .send()
        .await
        .expect("legal hold request");
    assert_eq!(legal_hold_response.status(), 403);
    assert_capability_denied(
        &legal_hold_response.json().await.expect("legal hold body"),
        "administer",
    );

    // --- Remaining §28 admin mutations (ONYX-MOB-01 §28, G001's
    // UserMutation/AdminCommand rows): activation, password reset,
    // manager/class/parent assignment, and batch profile import. Each is
    // a separate `require_admin_mutation` route, so each needs its own
    // denial assertion rather than being implied by one of the above.
    let activate_response = http
        .post(format!(
            "{base}/api/admin/users/{}/activate",
            uuid::Uuid::new_v4()
        ))
        .bearer_auth(&observer_token)
        .send()
        .await
        .expect("activate request");
    assert_eq!(activate_response.status(), 403);
    assert_capability_denied(
        &activate_response.json().await.expect("activate body"),
        "administer",
    );

    let password_response = http
        .post(format!(
            "{base}/api/admin/users/{}/password",
            uuid::Uuid::new_v4()
        ))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({"password": "irrelevant-1"}))
        .send()
        .await
        .expect("password reset request");
    assert_eq!(password_response.status(), 403);
    assert_capability_denied(
        &password_response.json().await.expect("password reset body"),
        "administer",
    );

    let manager_response = http
        .post(format!(
            "{base}/api/admin/users/{}/manager",
            uuid::Uuid::new_v4()
        ))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({"is_manager": false}))
        .send()
        .await
        .expect("manager assignment request");
    assert_eq!(manager_response.status(), 403);
    assert_capability_denied(
        &manager_response.json().await.expect("manager assignment body"),
        "administer",
    );

    let class_response = http
        .post(format!(
            "{base}/api/admin/users/{}/class",
            uuid::Uuid::new_v4()
        ))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({"class": "team_leader"}))
        .send()
        .await
        .expect("class assignment request");
    assert_eq!(class_response.status(), 403);
    assert_capability_denied(
        &class_response.json().await.expect("class assignment body"),
        "administer",
    );

    let parent_response = http
        .post(format!(
            "{base}/api/admin/users/{}/parent",
            uuid::Uuid::new_v4()
        ))
        .bearer_auth(&observer_token)
        .json(&serde_json::json!({"parent_user_id": uuid::Uuid::new_v4().to_string()}))
        .send()
        .await
        .expect("parent assignment request");
    assert_eq!(parent_response.status(), 403);
    assert_capability_denied(
        &parent_response.json().await.expect("parent assignment body"),
        "administer",
    );

    let import_response = http
        .post(format!("{base}/api/admin/profiles/import"))
        .bearer_auth(&observer_token)
        .multipart(
            reqwest::multipart::Form::new()
                .part("file", reqwest::multipart::Part::text("irrelevant").file_name("denied.csv")),
        )
        .send()
        .await
        .expect("profile import request");
    assert_eq!(import_response.status(), 403);
    assert_capability_denied(
        &import_response.json().await.expect("profile import body"),
        "administer",
    );
}

/// H10/P1.5 / ONYX-MOB-00 §24, MBP-012: refreshing an observer session
/// must not silently upgrade its classification. The refreshed access
/// token must still be denied on mutation and still permitted to read.
#[tokio::test]
async fn observer_session_refresh_preserves_the_capability_ceiling() {
    let (_addr, base) = start_server("refresh").await;
    let http = reqwest::Client::new();
    let (_, observer_refresh_token) = login_as(&http, &base, Some("mobile_observer")).await;

    let refreshed: serde_json::Value = http
        .post(format!("{base}/api/auth/refresh"))
        .json(&serde_json::json!({"refresh_token": observer_refresh_token}))
        .send()
        .await
        .expect("refresh request")
        .json()
        .await
        .expect("refresh body");
    let refreshed_access_token = refreshed["access_token"]
        .as_str()
        .expect("refreshed access_token")
        .to_string();

    let read_after_refresh = http
        .get(format!("{base}/api/users/hierarchy"))
        .bearer_auth(&refreshed_access_token)
        .send()
        .await
        .expect("read after refresh");
    assert_eq!(read_after_refresh.status(), 200);

    let mutation_after_refresh = http
        .post(format!("{base}/api/todo/lists"))
        .bearer_auth(&refreshed_access_token)
        .json(&serde_json::json!({
            "owner": uuid::Uuid::new_v4().to_string(),
            "origin": "staff_authored",
        }))
        .send()
        .await
        .expect("mutation after refresh");
    assert_eq!(
        mutation_after_refresh.status(),
        403,
        "a refreshed observer session must still be denied a mutation, not silently upgraded"
    );
    assert_capability_denied(
        &mutation_after_refresh
            .json()
            .await
            .expect("mutation after refresh body"),
        "submit_domain_command",
    );
}

/// Confirms the new capability ceiling and the pre-existing tenant
/// isolation check are orthogonal, layered checks, neither weakening
/// the other: a full-capability (`"web"`) session -- which passes the
/// new capability gate -- is still rejected by the untouched
/// `TENANT_MISMATCH` check when it targets a different organization.
#[tokio::test]
async fn cross_tenant_command_still_rejected_independent_of_client_capability() {
    let (_addr, base) = start_server("tenant").await;
    let http = reqwest::Client::new();
    let (web_token, _) = login_as(&http, &base, Some("web")).await;

    let other_org = uuid::Uuid::new_v4().to_string();
    let response = http
        .post(format!("{base}/api/command"))
        .bearer_auth(&web_token)
        .json(&command_envelope(
            &uuid::Uuid::new_v4().to_string(),
            "notification",
            &other_org,
            "notification.Acknowledge",
            serde_json::json!({}),
        ))
        .send()
        .await
        .expect("cross-tenant command request");
    assert_eq!(response.status(), 403);
    let body: serde_json::Value = response.json().await.expect("cross-tenant command body");
    assert_eq!(
        body["error"]["code"], "TENANT_MISMATCH",
        "a full-capability client must still be rejected by tenant isolation, unrelated to the new capability ceiling"
    );
}
