-- SQLite mirror of the Postgres migration of the same name. See that
-- file's header comment for full rationale (supersedes `is_manager`
-- with a real class hierarchy + reporting line, per
-- IMPLEMENTATION_PLAN_User_Hierarchy.md §2).
--
-- SQLite's ALTER TABLE cannot add a CHECK constraint via ADD COLUMN in
-- all deployed versions the way Postgres can; the class value is
-- validated at the application layer (security-application's UserClass
-- enum + its FromStr/TryFrom parsing) rather than enforced here, unlike
-- the Postgres migration's CHECK constraint. This is a genuine
-- validation-strength difference between the two backends for this
-- column, flagged here rather than silently assumed equivalent.
ALTER TABLE users ADD COLUMN class TEXT NULL;

-- SQLite enforces foreign keys only when `PRAGMA foreign_keys = ON` is
-- set per-connection (not a database-wide default). Checked this
-- project's actual connection setup before writing this comment: no
-- code anywhere sets that pragma today, so this REFERENCES clause is
-- currently decorative/documentation-only on SQLite — a bad
-- parent_user_id value (pointing at a non-existent user) will not be
-- rejected by the database. This is an existing gap in this project's
-- SQLite setup, not something introduced by this migration, but it
-- means application-level validation of parent_user_id (does the
-- referenced user actually exist?) is not optional on SQLite the way
-- it might be assumed to be with an FK in place — flag this to
-- whoever implements the UserStore write path for this column.
ALTER TABLE users ADD COLUMN parent_user_id TEXT NULL
    REFERENCES users(id);

CREATE INDEX idx_users_parent_user_id ON users (parent_user_id);
