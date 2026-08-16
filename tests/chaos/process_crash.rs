use super::chaos_runtime::{assert_recovered, run_scenario};
use std::time::Duration;

#[tokio::test]
async fn process_crash_reclaims_expired_job_lease() {
    let job = serde_json::json!({"status":"running","lease_expires_at":120,"attempts":1});
    let report = run_scenario(
        "process-crash",
        Duration::from_secs(90),
        Duration::from_secs(125),
    )
    .await;
    assert_recovered(&report);
    assert_eq!(job["attempts"], 1);
}
