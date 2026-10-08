//! Real HTTP regression coverage for StaffLoan decision authority.
//!
//! The domain aggregate intentionally leaves actor-specific authority to
//! the composing application. These tests ensure `/api/command` supplies
//! that boundary for ordinary approval and escalation replacement.

use std::net::SocketAddr;

use reqwest::StatusCode;

const ORG_ID: &str = "11111111-1111-1111-1111-111111111111";

async fn start_server(db_label: &str) -> (SocketAddr, String) {
    let db_path =
        std::env::temp_dir().join(format!("onyx-staff-loan-authorization-test-{db_label}.db"));
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

    let http = reqwest::Client::new();
    let base = format!("http://{addr}");
    // ApiState creates the first administrator on a fresh database, so the
    // token-gated bootstrap endpoint correctly rejects a second first account.
    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({
            "username": "All-Father",
            "password": "passvord0000",
        }))
        .send()
        .await
        .expect("admin login request")
        .error_for_status()
        .expect("admin login succeeds")
        .json()
        .await
        .expect("admin login body");
    (
        addr,
        login["access_token"]
            .as_str()
            .expect("admin access token")
            .to_string(),
    )
}

async fn create_user_with_token(
    http: &reqwest::Client,
    base: &str,
    admin_token: &str,
    username: &str,
) -> (String, String) {
    let created: serde_json::Value = http
        .post(format!("{base}/api/admin/users"))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "username": username,
            "password": "a-fine-password-1",
        }))
        .send()
        .await
        .expect("create user request")
        .error_for_status()
        .expect("create user succeeds")
        .json()
        .await
        .expect("created user body");
    let id = created["id"].as_str().expect("user id").to_string();

    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({
            "username": username,
            "password": "a-fine-password-1",
        }))
        .send()
        .await
        .expect("user login request")
        .error_for_status()
        .expect("user login succeeds")
        .json()
        .await
        .expect("user login body");
    let token = login["access_token"]
        .as_str()
        .expect("user access token")
        .to_string();

    (id, token)
}

async fn set_parent(
    http: &reqwest::Client,
    base: &str,
    admin_token: &str,
    user_id: &str,
    parent_user_id: &str,
) {
    http.post(format!("{base}/api/admin/users/{user_id}/parent"))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"parent_user_id": parent_user_id}))
        .send()
        .await
        .expect("set parent request")
        .error_for_status()
        .expect("set parent succeeds");
}

async fn request_staff_loan(
    http: &reqwest::Client,
    base: &str,
    requester_token: &str,
    staff_user_id: &str,
    real_owner_id: &str,
    borrowing_manager_id: &str,
) -> String {
    let created: serde_json::Value = http
        .post(format!("{base}/api/todo/staff-loans"))
        .bearer_auth(requester_token)
        .json(&serde_json::json!({
            "staff_user_id": staff_user_id,
            "real_owner_id": real_owner_id,
            "borrowing_manager_id": borrowing_manager_id,
            "start_at_ms": 1_700_000_000_000_u64,
            "end_at_ms": 1_700_086_400_000_u64,
        }))
        .send()
        .await
        .expect("request staff loan")
        .error_for_status()
        .expect("request staff loan succeeds")
        .json()
        .await
        .expect("created loan body");
    created["staff_loan_id"]
        .as_str()
        .expect("staff loan id")
        .to_string()
}

fn command_payload(
    staff_loan_id: &str,
    command_type: &str,
    expected_version: u64,
    expected_authority_epoch: u64,
    payload: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "command_id": uuid::Uuid::new_v4().to_string(),
        "operation_id": uuid::Uuid::new_v4().to_string(),
        "command_type": command_type,
        "schema_version": "1.0",
        "target": {"id": staff_loan_id, "type": "staff_loan", "organization_id": ORG_ID},
        "expected_version": expected_version,
        "expected_lifecycle_epoch": 0,
        "expected_authority_epoch": expected_authority_epoch,
        "issued_at": "2026-08-16T00:00:00Z",
        "correlation_id": uuid::Uuid::new_v4().to_string(),
        "payload": payload,
    })
}

#[tokio::test]
async fn unrelated_user_cannot_approve_or_decline_a_staff_loan() {
    let (addr, admin_token) = start_server("unrelated-rejected").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");
    let (staff_id, staff_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-staff-a").await;
    let (owner_id, _owner_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-owner-a").await;
    let (borrowing_id, _borrowing_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-borrowing-a").await;
    let (_unrelated_id, unrelated_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-unrelated-a").await;
    let approval_loan_id = request_staff_loan(
        &http,
        &base,
        &staff_token,
        &staff_id,
        &owner_id,
        &borrowing_id,
    )
    .await;
    let decline_loan_id = request_staff_loan(
        &http,
        &base,
        &staff_token,
        &staff_id,
        &owner_id,
        &borrowing_id,
    )
    .await;

    for (staff_loan_id, command_type, payload) in [
        (
            approval_loan_id.as_str(),
            "staff_loan.ApproveStaffLoan",
            serde_json::json!("ApproveStaffLoan"),
        ),
        (
            decline_loan_id.as_str(),
            "staff_loan.DeclineStaffLoan",
            serde_json::json!({"DeclineStaffLoan": {"reason": "not my decision"}}),
        ),
    ] {
        let response = http
            .post(format!("{base}/api/command"))
            .bearer_auth(&unrelated_token)
            .json(&command_payload(staff_loan_id, command_type, 0, 0, payload))
            .send()
            .await
            .expect("unauthorized command request");
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{command_type} must reject an unrelated user"
        );
    }
}

#[tokio::test]
async fn real_owner_can_approve_a_staff_loan() {
    let (addr, admin_token) = start_server("owner-accepted").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");
    let (staff_id, staff_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-staff-b").await;
    let (owner_id, owner_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-owner-b").await;
    let (borrowing_id, _borrowing_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-borrowing-b").await;
    let staff_loan_id = request_staff_loan(
        &http,
        &base,
        &staff_token,
        &staff_id,
        &owner_id,
        &borrowing_id,
    )
    .await;

    let response = http
        .post(format!("{base}/api/command"))
        .bearer_auth(&owner_token)
        .json(&command_payload(
            &staff_loan_id,
            "staff_loan.ApproveStaffLoan",
            0,
            0,
            serde_json::json!("ApproveStaffLoan"),
        ))
        .send()
        .await
        .expect("owner approval request");
    assert!(
        response.status().is_success(),
        "the real owner must be able to approve"
    );
}

#[tokio::test]
async fn escalation_target_can_approve_after_real_owner_escalates() {
    let (addr, admin_token) = start_server("escalation-target-accepted").await;
    let http = reqwest::Client::new();
    let base = format!("http://{addr}");
    let (staff_id, staff_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-staff-c").await;
    let (owner_id, owner_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-owner-c").await;
    let (borrowing_id, _borrowing_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-borrowing-c").await;
    let (target_id, target_token) =
        create_user_with_token(&http, &base, &admin_token, "loan-escalation-target-c").await;
    set_parent(&http, &base, &admin_token, &owner_id, &target_id).await;
    let staff_loan_id = request_staff_loan(
        &http,
        &base,
        &staff_token,
        &staff_id,
        &owner_id,
        &borrowing_id,
    )
    .await;

    let escalation = http
        .post(format!("{base}/api/command"))
        .bearer_auth(&owner_token)
        .json(&command_payload(
            &staff_loan_id,
            "staff_loan.EscalateStaffLoan",
            0,
            0,
            serde_json::json!({"EscalateStaffLoan": {"reason": "needs the next-level decision"}}),
        ))
        .send()
        .await
        .expect("escalation request");
    assert!(
        escalation.status().is_success(),
        "the real owner must be able to escalate"
    );

    let response = http
        .post(format!("{base}/api/command"))
        .bearer_auth(&target_token)
        .json(&command_payload(
            &staff_loan_id,
            "staff_loan.ApproveStaffLoan",
            1,
            1,
            serde_json::json!("ApproveStaffLoan"),
        ))
        .send()
        .await
        .expect("escalation target approval request");
    assert!(
        response.status().is_success(),
        "the escalation target must replace the real owner for approval"
    );
}
