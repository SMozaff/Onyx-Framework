use super::chaos_runtime::{assert_recovered, run_scenario};
use std::time::Duration;

#[tokio::test]
async fn database_failover_recovers_inside_rto_and_rpo() {
    let report = run_scenario(
        "database-failover",
        Duration::from_secs(180),
        Duration::from_secs(225),
    )
    .await;
    assert_recovered(&report);
    assert!(report.recovery_time < Duration::from_secs(60));
    let replicated_lag = Duration::from_secs(90);
    assert!(replicated_lag < Duration::from_secs(5 * 60), "RPO breached");
}
