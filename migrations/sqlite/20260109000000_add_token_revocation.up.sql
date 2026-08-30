-- See the matching Postgres migration's comment for why these two tables
-- exist. This SQLite copy exists for schema parity with the Postgres
-- migration set (same convention as add_rate_limit); in practice a
-- pure-SQLite composition (no governance/primary Postgres configured)
-- uses InMemoryTokenRevocationStore instead of these tables -- see
-- ApiState::new's store-selection logic in api-server/src/routes/mod.rs.
CREATE TABLE revoked_tokens (
    token_hash TEXT PRIMARY KEY,
    revoked_at INTEGER NOT NULL
);

CREATE TABLE user_token_revocations (
    user_id TEXT PRIMARY KEY,
    revoked_before INTEGER NOT NULL
);
