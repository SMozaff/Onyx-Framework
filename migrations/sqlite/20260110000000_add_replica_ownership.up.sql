-- See the matching Postgres migration's comment for why this table exists.
CREATE TABLE replica_ownership (
    replica_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    claimed_at INTEGER NOT NULL
);
CREATE INDEX idx_replica_ownership_user ON replica_ownership (user_id);
