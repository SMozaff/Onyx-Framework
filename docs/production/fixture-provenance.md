# Fixture inventory and provenance (C-09)

**Status:** Inventory recorded 2026-10-31. **OPEN:** The owner approved "the fixture schema" on 2026-10-29, but the approved schema is not identified in the repo. See "Open" below.

## Fixture files

| File | Lines | First added | Origin | Purpose |
|---|---|---|---|---|
| `deploy/cloudflare/edge-gateway/test/fixtures/original-0002_onyx_identity_model.sql` | 35 | c97501d, 2026-10-24 (PR #294) | Copy of the original D1 migration `0002_onyx_identity_model`, kept for test reference | Identity model: Supabase for All-Father only; Admin and Staff credentials in D1 |
| `deploy/cloudflare/edge-gateway/test/fixtures/original-0003_supabase_identity.sql` | 6 | c97501d, 2026-10-24 (PR #294) | Copy of the original D1 migration `0003_supabase_identity` | Supabase Auth linkage |
| `tests/fixtures/seed.sql` | 129 | c97501d, 2026-10-24 (PR #294) | Deterministic SQLite seed (org `11111111-…`), "ONYX Team 6" | Test database seed |
| `shared/diagnostics/diagnostic-event.schema.json` | — | c97501d, 2026-10-24 (PR #294) | Versioned contract `urn:onyx:diagnostic-event:1.0` with `shared/diagnostics/README.md` | Diagnostic event format (not a test fixture) |

Provenance is taken from git history: all four files first appear in c97501d (merge of PR #294). Their content before that commit is not recorded here.

## Open

1. **Which schema did you approve?** The only versioned schema in `shared/` is the diagnostic event contract (1.0). It is the likely candidate, but I have not assumed that. Please confirm, or point to the schema file.
2. Once confirmed, the approval is recorded in the decision log with the schema's version and path.
