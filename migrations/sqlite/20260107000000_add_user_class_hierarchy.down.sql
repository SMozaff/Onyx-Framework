-- SQLite (pre-3.35) cannot DROP COLUMN in all deployed versions; this
-- mirrors the same convention already used by
-- 20260106000000_add_manager_role.down.sql for this project.
DROP INDEX IF EXISTS idx_users_parent_user_id;
ALTER TABLE users DROP COLUMN parent_user_id;
ALTER TABLE users DROP COLUMN class;
