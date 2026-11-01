-- Staff and team public IDs (docs/design/id-sequences.md).
-- Promoted from drafts/backend-accounts/ with owner decision O-2 (D1 is the account store).
-- id_sequences.last_value is capped at 9999: an allocation above the cap fails the
-- whole D1 batch, so a failed row leaves no gap (per the design doc requirement).
-- Staff IDs are allocated inside the same batch that writes the account (see src/index.ts,
-- createStaffAccount). Rows are never deleted from public_ids.

CREATE TABLE IF NOT EXISTS id_sequences (
  organization_id TEXT    NOT NULL,
  kind            TEXT    NOT NULL CHECK (kind IN ('staff','team')),
  year            INTEGER NOT NULL,
  last_value      INTEGER NOT NULL DEFAULT 0 CHECK (last_value BETWEEN 0 AND 9999),
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
