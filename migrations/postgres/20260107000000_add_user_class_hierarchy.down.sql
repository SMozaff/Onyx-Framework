DROP INDEX IF EXISTS idx_users_parent_user_id;
ALTER TABLE users DROP COLUMN IF EXISTS parent_user_id;
ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_users_class;
ALTER TABLE users DROP COLUMN IF EXISTS class;
