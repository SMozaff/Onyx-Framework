-- SQLite mirror of the Postgres users table (audit finding H-01).
-- Type mapping follows the convention already established by this project's
-- other SQLite migrations: UUID -> TEXT, BOOLEAN -> INTEGER, TIMESTAMPTZ ->
-- INTEGER epoch milliseconds.
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_users_username_lower ON users (LOWER(username));
CREATE INDEX idx_users_organization ON users (organization_id);
