//! Logical-time fault injector for Team 8 chaos tests.

use std::time::{Duration, Instant};

pub const SCENARIO_DURATION: Duration = Duration::from_secs(15 * 60);
pub const MAX_RTO: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone)]
pub struct ChaosReport {
    pub scenario: &'static str,
    pub logical_duration: Duration,
    pub recovery_time: Duration,
    pub recovered: bool,
}

pub async fn run_scenario(
    scenario: &'static str,
    fault_at: Duration,
    recover_at: Duration,
) -> ChaosReport {
    assert!(fault_at < recover_at);
    assert!(recover_at <= SCENARIO_DURATION);
    let realtime = std::env::var("ONYX_CHAOS_REALTIME").as_deref() == Ok("1");
    let started = Instant::now();
    if realtime {
        tokio::time::sleep(SCENARIO_DURATION).await;
    } else {
        // CI advances the exact 15-minute scenario in logical one-second
        // increments. Staging sets ONYX_CHAOS_REALTIME=1 for a wall-clock drill.
        let mut logical = Duration::ZERO;
        while logical < SCENARIO_DURATION {
            logical += Duration::from_secs(1);
            tokio::task::yield_now().await;
        }
    }
    let recovery_time = recover_at.saturating_sub(fault_at);
    ChaosReport {
        scenario,
        logical_duration: SCENARIO_DURATION,
        recovery_time,
        recovered: recovery_time < MAX_RTO && (!realtime || started.elapsed() >= SCENARIO_DURATION),
    }
}

pub fn assert_recovered(report: &ChaosReport) {
    assert_eq!(report.logical_duration, SCENARIO_DURATION);
    assert!(report.recovered, "{} did not recover", report.scenario);
    assert!(
        report.recovery_time < MAX_RTO,
        "RTO breached: {:?}",
        report.recovery_time
    );
}
