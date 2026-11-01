-- DRAFT ONLY. NOT APPLIED. NOT IN migrations/ (Wrangler reads migrations/, not this folder).
-- Source: docs/design/id-sequences.md (proposed tables, migration 0009_public_ids.sql).
-- Blocked by: O-2 (confirm D1 as account store before any migration is applied).
-- Moving this file into migrations/ and applying it is an owner decision.
-- Allocation statement and import flow are in docs/design/id-sequences.md, not here.

CREATE TABLE IF NOT EXISTS id_sequences (
  organization_id TEXT    NOT NULL,
  kind            TEXT    NOT NULL CHECK (kind IN ('staff','team')),
  year            INTEGER NOT NULL,
  last_value      INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (organization_id, kind, year)
);

CREATE TABLE IF NOT EXISTS public_ids (
  organization_id TEXT    NOT NULL,
  public_id       TEXT    NOT NULL,
  kind            TEXT    NOT NULL CHECK (kind IN ('staff','team')),
  aggregate_id    TEXT    NOT NULL,
  issued_at       INTEGER NOT NULL,
  retired_at      INTEGER NULL,            -- set on removal; row is never deleted
  PRIMARY KEY (organization_id, public_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_public_ids_aggregate
  ON public_ids (organization_id, aggregate_id);
