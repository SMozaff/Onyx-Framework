# B-01 Test Coverage and Gaps — Wave 0

**Execution status: local tests NOT RUN.** Source/test inventory only.  
**Baseline:** `b056c6ef204080446e84b31f3d35a226e6a40867`.

## Existing test sources observed

- Worker `test/route-completeness.test.ts`, `auth.test.ts`, authorization and auth regression tests.
- Admin UI organization API, profile transfer, settings helper, credential and login diagnostics tests.
- Worker package scripts: `npm test` runs `vitest run`; `npm run check` runs Wrangler deploy dry-run using `wrangler.dev.toml`.
- Existing test files were not executed in this task.

## CI evidence

- Root CI [40336191173](https://github.com/SMozaff/Onyx-Framework/actions/runs/40336191173): **FAIL** on security license/source-policy audit; merge gate failed as a consequence.
- Admin Slint scaffold [40336190967](https://github.com/SMozaff/Onyx-Framework/actions/runs/40336190967): **PASS** on Ubuntu/Windows for the scaffold only.
- Dedicated Worker CI at the inspected main SHA: no run verified.
- Local Worker/UI tests and Rust build/tests: **NOT RUN**. Runtime had Node 22/npm 10 and no Rust/Cargo; Worker workflow uses Node 24 and repo pins Rust 1.97.1.

## Required contract tests after approval

| ID | Scope | Minimum assertions |
|---|---|---|
| B01-T01 | Method/path | Every C-01 operation matches Worker dispatch and a real caller |
| B01-T02 | Request schema | Required/optional keys, nullability, enums, bounds and exact-key checks where enforced |
| B01-T03 | Success DTO | Required fields, conditional fields, content type, CSV/JSON export shape |
| B01-T04 | Error union | All observed envelope variants and malformed/unknown body handling |
| B01-T05 | Auth | Login/refresh schema, invalid session, replay/revocation; no credential leakage |
| B01-T06 | Authorization | Cross-tenant and role-boundary denial verified at Worker boundary |
| B01-T07 | Profile import/export | CSV/JSON, malformed rows, limits, partial row failures, filename/content type |
| B01-T08 | Version conflict | `expected_version`, 409 current version, no stale-body auto-retry |
| B01-T09 | Idempotency | Same key/payload replay; changed payload conflict; same key reused only for safe retry |
| B01-T10 | Commands | Required target/version/epoch, nested errors and `success:false` |
| B01-T11 | Transport | Timeout, no response, malformed JSON, wrong success DTO |
| B01-T12 | Admin create DTO | Block/fail until canonical response decision is recorded |
| B01-T13 | Secret hygiene | No password/token/raw-body leakage in logs, fixtures or diagnostics |
| B01-T14 | CI matrix | Exact SHA and required Windows/Linux jobs reported |

## Reproducible commands (not executed)

```bash
cd deploy/cloudflare/edge-gateway
npm install
npm run check
npm test
```

Use Node 24 to match Worker CI. Run Admin UI tests from `crates/bins/admin-shell/ui` using its checked-in `npm test` script. Rust tests require the repository-pinned Rust 1.97.1 toolchain and an approved crate/module path. Worker CI's disposable local D1 integration should be run through the workflow, never against production data.

## Fixture note

Candidate JSON fixtures are synthetic and token-free. They are not C-09 schema validated; B-13 must approve the fixture schema/version and validate complete per-route/per-error coverage. A fixture parse pass is artifact sanity only, not a contract test pass.