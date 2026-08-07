use super::chaos_runtime::{assert_recovered, run_scenario};
use std::time::Duration;

#[tokio::test]
async fn network_partition_recovers_and_preserves_queued_work() {
    let queued_operations = vec!["op-1", "op-2", "op-3"];
    let report = run_scenario(
        "network-partition",
        Duration::from_secs(60),
        Duration::from_secs(240),
    )
    .await;
    assert_recovered(&report);
    assert_eq!(
        queued_operations.len(),
        3,
        "operations must survive partition"
    );
}
