-- SQLite mirror of the Postgres migration of the same name. See that
-- file's header comment for full rationale.
ALTER TABLE users ADD COLUMN is_manager INTEGER NOT NULL DEFAULT 0;
