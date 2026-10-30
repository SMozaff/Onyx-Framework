# Storage for generated IDs (staff and team)

**Status:** Design draft, 2026-10-30. Not implemented. Two decisions are marked **OPEN**.

## Context

- Production store: Cloudflare D1 (`deploy/cloudflare/edge-gateway`).
- Staff profiles live in `aggregates` with `aggregate_type = 'staff_profile'`, keyed by a UUID (`id`), with the profile as JSON in `state`.
- The UUID stays the internal key. The generated ID (`st26sm0035`, `tm261030sm0007`) is a separate, shown staff/team number.
- Generation rules: `crates/bins/api-server/src/routes/profiles/id_generator.rs`.

## Requirements

1. Order numbers are per year, start at 1, and are never reused, even after a person or team is removed.
2. Two records can never get the same public ID.
3. A failed import leaves no gaps in the numbering and no half-written rows.
4. The sequence is atomic under concurrent imports.

## Proposed tables (migration `0009_public_ids.sql`)

```sql
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
```

- `public_ids` is the uniqueness guarantee. The primary key rejects any duplicate.
- Removal sets `retired_at`. It never deletes, so a number can't be issued again.

## Allocation

One statement per ID, inside a D1 batch together with the `public_ids` insert:

```sql
INSERT INTO id_sequences (organization_id, kind, year, last_value)
VALUES (?, ?, ?, 1)
ON CONFLICT (organization_id, kind, year)
DO UPDATE SET last_value = last_value + 1
RETURNING last_value;
```

- The returned value is the order. If it is above 9999, the batch fails and nothing is written.
- The public ID is built with `staff_id(...)` or `team_id(...)`, then inserted into `public_ids` in the same batch.
- A batch is all-or-nothing, so a failed row leaves no gap. The import should commit one batch per row, or one batch for the whole file, and say which.

## Import flow

1. Parse the sheet (`staff_sheet.rs`). Rows with a blank `onyx_id` need a new ID.
2. For each new row, allocate and write in one batch, with the profile `aggregates` row and its `domain_events` row.
3. Return each row's outcome, including its new `onyx_id`.

## Open decisions

1. **Year boundary:** The year comes from creation time. Is that UTC, or Iran local time (Asia/Tehran, UTC+03:30)? A record created on 31 December near midnight can fall into a different year, so this matters.
2. **Scope:** Are numbers counted per organization, or across the whole deployment? One device serves one organization, but the database may hold several.

## Not in this design

- The team aggregate itself (team table and routes). Only the ID storage is covered here.
- Changing the existing UUID-based routes.
