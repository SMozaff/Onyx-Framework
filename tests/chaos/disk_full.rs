use super::chaos_runtime::{assert_recovered, run_scenario};
use std::{io::Write, time::Duration};

#[tokio::test]
async fn disk_full_fails_closed_then_recovers() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let mut audit_spool = std::fs::File::create(directory.path().join("audit-spool.jsonl"))?;
    audit_spool.write_all(b"{\"outcome\":\"buffered\"}\n")?;
    audit_spool.sync_all()?;
    let report = run_scenario(
        "disk-full",
        Duration::from_secs(100),
        Duration::from_secs(180),
    )
    .await;
    assert_recovered(&report);
    assert!(std::fs::metadata(directory.path().join("audit-spool.jsonl"))?.len() > 0);
    Ok(())
}
