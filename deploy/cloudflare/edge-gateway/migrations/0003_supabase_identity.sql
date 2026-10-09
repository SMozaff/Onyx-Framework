-- Supabase Auth identity linkage.
-- Supabase is the authentication provider; Cloudflare D1 remains the ONYX authorization/system-of-record database.

ALTER TABLE users ADD COLUMN supabase_user_id TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_supabase_user_id
    ON users (supabase_user_id) WHERE supabase_user_id IS NOT NULL;
