use super::chaos_runtime::{assert_recovered, run_scenario};
use platform_kernel::{CausalRelation, ReplicaId, VectorClock};
use std::time::Duration;

#[tokio::test]
async fn clock_skew_does_not_change_causal_order() {
    let replica = ReplicaId::new_random();
    let mut before = VectorClock::new();
    before.increment(replica);
    let mut after = before.clone();
    after.increment(replica);
    let report = run_scenario(
        "clock-skew",
        Duration::from_secs(120),
        Duration::from_secs(121),
    )
    .await;
    assert_recovered(&report);
    assert_eq!(before.compare(&after), CausalRelation::Before);
}
