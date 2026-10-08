-- SQLite (pre-3.35) cannot DROP COLUMN in all deployed versions; the
-- project's own convention elsewhere in this migrations directory is
-- checked below before assuming a plain DROP COLUMN is safe.
ALTER TABLE users DROP COLUMN is_manager;
