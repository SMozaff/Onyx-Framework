use audit_application::{AuditRecord, AuditWriter};
use observability_adapter::HashChainAuditWriter;
use platform_kernel::{ObjectId, Timestamp};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn audit_hash_chain_detects_tampering() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../../migrations/sqlite")
        .run(&pool)
        .await
        .unwrap();
    let writer = HashChainAuditWriter::sqlite(pool.clone());
    let org = ObjectId([4; 16]);
    for action in ["mission.Activate", "task.Close"] {
        writer
            .append(&AuditRecord {
                organization_id: org,
                actor_id: Some(ObjectId([5; 16])),
                operation_id: None,
                correlation_id: None,
                category: "command".to_string(),
                action: action.to_string(),
                outcome: "accepted".to_string(),
                occurred_at: Timestamp::now(),
                safe_details: serde_json::json!({"safe":true}),
            })
            .await
            .unwrap();
    }
    assert!(writer.verify(org).await.unwrap().valid);
    sqlx::query("UPDATE audit_entries SET record = ? WHERE sequence = 1")
        .bind("{\"tampered\":true}")
        .execute(&pool)
        .await
        .unwrap();
    let verification = writer.verify(org).await.unwrap();
    assert!(!verification.valid);
    assert_eq!(verification.first_invalid_sequence, Some(1));
}
