# C-09 fixture schema identity and provenance — PROPOSED draft (2026-11-23)

**Status: PROPOSED — artifact identity not confirmed.** G0B-11 stays OPEN.

## Source evidence

`docs/production/fixture-provenance.md` records four fixture/artifact files, all first appearing in
`c97501d` (merge of PR #294, 2026-10-24):

| File | Purpose |
|---|---|
| `deploy/cloudflare/edge-gateway/test/fixtures/original-0002_onyx_identity_model.sql` | Copy of original D1 migration `0002_onyx_identity_model` |
| `deploy/cloudflare/edge-gateway/test/fixtures/original-0003_supabase_identity.sql` | Copy of original D1 migration `0003_supabase_identity` |
| `tests/fixtures/seed.sql` | Deterministic SQLite seed (org `11111111-…`, "ONYX Team 6") |
| `shared/diagnostics/diagnostic-event.schema.json` | Versioned contract `urn:onyx:diagnostic-event:1.0` (not a test fixture) |

Decision log 2026-11-08 correction: the owner approved "the fixture schema" on 2026-10-29; what
is missing is committed **provenance** identifying which schema was approved. The only versioned
schema in `shared/` is the diagnostic-event contract; it is a candidate, not an assumption.

## PROPOSED identity (requires owner confirmation)

- Candidate schema: `shared/diagnostics/diagnostic-event.schema.json`, version
  `urn:onyx:diagnostic-event:1.0`. Confirmation is outstanding; do not record it as approved.
- Fixture provenance anchor: `c97501d` (PR #294), with content before that commit unrecorded.
- Validator: not yet identified. Requirement: validate each fixture against the schema in C-01.

## Acceptance (not met)

1. Owner-confirmed schema path and version recorded in the decision log.
2. Validator command/test committed and run against valid and invalid fixtures.
3. Route/error coverage per endpoint from C-01.
4. No blocker closed by this draft.
