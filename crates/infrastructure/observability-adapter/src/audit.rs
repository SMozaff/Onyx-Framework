use async_trait::async_trait;
use audit_application::{
    AuditError, AuditRecord, AuditWriteReceipt, AuditWriter, IntegrityDigest, IntegrityVerification,
};
use platform_kernel::OrganizationId;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row, SqlitePool};

#[derive(Clone)]
pub enum HashChainAuditWriter {
    Postgres { pool: PgPool },
    Sqlite { pool: SqlitePool },
}

impl HashChainAuditWriter {
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres { pool }
    }

    pub fn sqlite(pool: SqlitePool) -> Self {
        Self::Sqlite { pool }
    }
}

fn canonical_record(record: &AuditRecord) -> Result<Vec<u8>, AuditError> {
    let value = serde_json::to_value(record)
        .map_err(|error| AuditError::Serialization(error.to_string()))?;
    canonical_value(&value)
}

fn canonical_value(value: &Value) -> Result<Vec<u8>, AuditError> {
    fn normalize(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut keys = map.keys().cloned().collect::<Vec<_>>();
                keys.sort();
                let mut output = serde_json::Map::new();
                for key in keys {
                    output.insert(key.clone(), normalize(&map[&key]));
                }
                Value::Object(output)
            }
            Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_vec(&normalize(value))
        .map_err(|error| AuditError::Serialization(error.to_string()))
}

fn calculate_hash(previous: &[u8], record: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(previous);
    hasher.update(record);
    hasher.finalize().to_vec()
}

fn zero_hash() -> Vec<u8> {
    vec![0_u8; 32]
}

fn organization_uuid(organization_id: OrganizationId) -> String {
    uuid::Uuid::from_bytes(organization_id.0).to_string()
}

#[async_trait]
impl AuditWriter for HashChainAuditWriter {
    async fn append(&self, record: &AuditRecord) -> Result<AuditWriteReceipt, AuditError> {
        let encoded = canonical_record(record)?;
        match self {
            Self::Postgres { pool } => {
                let org = organization_uuid(record.organization_id);
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(|e| AuditError::Persistence(e.to_string()))?;
                sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                    .bind(format!("audit:{org}"))
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| AuditError::Persistence(e.to_string()))?;
                let previous: Option<Vec<u8>> = sqlx::query_scalar(
                    "SELECT current_hash FROM audit_entries WHERE organization_id = $1::uuid ORDER BY sequence DESC LIMIT 1",
                )
                .bind(&org)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| AuditError::Persistence(e.to_string()))?;
                let previous = previous.unwrap_or_else(zero_hash);
                let current = calculate_hash(&previous, &encoded);
                let sequence: i64 = sqlx::query_scalar(
                    "INSERT INTO audit_entries (organization_id, previous_hash, current_hash, record, occurred_at_ms) VALUES ($1::uuid, $2, $3, $4::jsonb, $5) RETURNING sequence",
                )
                .bind(&org)
                .bind(&previous)
                .bind(&current)
                .bind(String::from_utf8(encoded).map_err(|e| AuditError::Serialization(e.to_string()))?)
                .bind((record.occurred_at.0 / 1_000_000) as i64)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| AuditError::Persistence(e.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|e| AuditError::Persistence(e.to_string()))?;
                Ok(AuditWriteReceipt {
                    sequence,
                    previous_hash: IntegrityDigest(hex::encode(previous)),
                    current_hash: IntegrityDigest(hex::encode(current)),
                })
            }
            Self::Sqlite { pool } => {
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(|e| AuditError::Persistence(e.to_string()))?;
                let previous: Option<Vec<u8>> = sqlx::query_scalar(
                    "SELECT current_hash FROM audit_entries WHERE organization_id = ? ORDER BY sequence DESC LIMIT 1",
                )
                .bind(record.organization_id.0.to_vec())
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| AuditError::Persistence(e.to_string()))?;
                let previous = previous.unwrap_or_else(zero_hash);
                let current = calculate_hash(&previous, &encoded);
                let result = sqlx::query(
                    "INSERT INTO audit_entries (organization_id, previous_hash, current_hash, record, occurred_at_ms) VALUES (?, ?, ?, ?, ?)",
                )
                .bind(record.organization_id.0.to_vec())
                .bind(&previous)
                .bind(&current)
                .bind(String::from_utf8(encoded).map_err(|e| AuditError::Serialization(e.to_string()))?)
                .bind((record.occurred_at.0 / 1_000_000) as i64)
                .execute(&mut *tx)
                .await
                .map_err(|e| AuditError::Persistence(e.to_string()))?;
                let sequence = result.last_insert_rowid();
                tx.commit()
                    .await
                    .map_err(|e| AuditError::Persistence(e.to_string()))?;
                Ok(AuditWriteReceipt {
                    sequence,
                    previous_hash: IntegrityDigest(hex::encode(previous)),
                    current_hash: IntegrityDigest(hex::encode(current)),
                })
            }
        }
    }

    async fn verify(
        &self,
        organization_id: OrganizationId,
    ) -> Result<IntegrityVerification, AuditError> {
        let rows: Vec<(i64, Vec<u8>, Vec<u8>, Value)> = match self {
            Self::Postgres { pool } => {
                let org = organization_uuid(organization_id);
                let raw = sqlx::query(
                    "SELECT sequence, previous_hash, current_hash, record FROM audit_entries WHERE organization_id = $1::uuid ORDER BY sequence ASC",
                )
                .bind(org)
                .fetch_all(pool)
                .await
                .map_err(|e| AuditError::Persistence(e.to_string()))?;
                raw.into_iter()
                    .map(|row| {
                        Ok((
                            row.try_get("sequence")
                                .map_err(|e| AuditError::Persistence(e.to_string()))?,
                            row.try_get("previous_hash")
                                .map_err(|e| AuditError::Persistence(e.to_string()))?,
                            row.try_get("current_hash")
                                .map_err(|e| AuditError::Persistence(e.to_string()))?,
                            row.try_get("record")
                                .map_err(|e| AuditError::Persistence(e.to_string()))?,
                        ))
                    })
                    .collect::<Result<Vec<_>, AuditError>>()?
            }
            Self::Sqlite { pool } => {
                let raw = sqlx::query(
                    "SELECT sequence, previous_hash, current_hash, record FROM audit_entries WHERE organization_id = ? ORDER BY sequence ASC",
                )
                .bind(organization_id.0.to_vec())
                .fetch_all(pool)
                .await
                .map_err(|e| AuditError::Persistence(e.to_string()))?;
                raw.into_iter()
                    .map(|row| {
                        let record_raw: String = row
                            .try_get("record")
                            .map_err(|e| AuditError::Persistence(e.to_string()))?;
                        let record = serde_json::from_str(&record_raw)
                            .map_err(|e| AuditError::Serialization(e.to_string()))?;
                        Ok((
                            row.try_get("sequence")
                                .map_err(|e| AuditError::Persistence(e.to_string()))?,
                            row.try_get("previous_hash")
                                .map_err(|e| AuditError::Persistence(e.to_string()))?,
                            row.try_get("current_hash")
                                .map_err(|e| AuditError::Persistence(e.to_string()))?,
                            record,
                        ))
                    })
                    .collect::<Result<Vec<_>, AuditError>>()?
            }
        };

        let mut previous = zero_hash();
        for (index, (sequence, stored_previous, stored_current, record)) in rows.iter().enumerate()
        {
            let record_bytes = canonical_value(record)?;
            let expected = calculate_hash(&previous, &record_bytes);
            if stored_previous != &previous || stored_current != &expected {
                return Ok(IntegrityVerification {
                    valid: false,
                    checked_entries: index as u64,
                    first_invalid_sequence: Some(*sequence),
                    expected_hash: Some(IntegrityDigest(hex::encode(expected))),
                    actual_hash: Some(IntegrityDigest(hex::encode(stored_current))),
                });
            }
            previous = stored_current.clone();
        }
        Ok(IntegrityVerification {
            valid: true,
            checked_entries: rows.len() as u64,
            first_invalid_sequence: None,
            expected_hash: None,
            actual_hash: None,
        })
    }
}
