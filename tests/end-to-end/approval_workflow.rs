use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

use super::test_harness::{bootstrap_and_login, PostgresHarness, ORGANIZATION_ID};
use api_server::routes::{router, ApiState};

#[tokio::test(flavor = "current_thread")]
async fn journey_4_approval_workflow() -> anyhow::Result<()> {
    let postgres = PostgresHarness::start().await?;
    std::env::set_var("ONYX_ENV", "test");
    let app = router(ApiState::new(&postgres.database_url).await?);

    // Provision a real user through the bootstrap endpoint. The previous
    // hardcoded `operator`/`onyx` login was removed by audit fix H-01.
    let token = bootstrap_and_login(app.clone()).await?;

    let command = json!({
        "command_id": uuid::Uuid::new_v4(),
        "operation_id": uuid::Uuid::new_v4(),
        "command_type": "approval.Approve",
        "schema_version": "1.0",
        "target": {
            "id": "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee1",
            "type": "approval",
            "organization_id": ORGANIZATION_ID
        },
        "expected_version": 1,
        "expected_lifecycle_epoch": 0,
        "expected_authority_epoch": 0,
        "issued_at": "2026-08-05T12:00:00Z",
        "vector_clock": {"entries": {}},
        "correlation_id": uuid::Uuid::new_v4(),
        "causation_id": null,
        "payload": {"reason": "Team 8 release approval"}
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/command")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(command.to_string()))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    postgres
        .record("journey-4-approval-workflow", "passed")
        .await?;
    Ok(())
}
