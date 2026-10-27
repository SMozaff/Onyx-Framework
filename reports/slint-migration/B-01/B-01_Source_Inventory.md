# B-01 Source Inventory — Wave 0

**Status: audit evidence only; no implementation authorized.**  
**Baseline:** `main` at `b056c6ef204080446e84b31f3d35a226e6a40867`.

## Scope and inspected sources

| Source | Audit purpose | Result |
|---|---|---|
| `deploy/cloudflare/edge-gateway/src/index.ts` | Worker route dispatch, handlers, request validation, authorization, DTOs and error envelopes | Current source audited; no changes |
| `docs/production/api-contract.md` | Existing contract/source stamp | Stamp is older than the audited Worker source; reconcile before freeze |
| `crates/bins/admin-shell/ui/src/api/client.ts`, `query.ts` | UI request wrappers and query behavior | Existing Tauri/React client, not a Slint Rust client |
| `crates/bins/admin-shell/ui/src/auth/credentials.ts` | Login/session client behavior | Reuse behavior only after B-02/B-05 contract approval |
| Admin UI pages: Login, Users, Profiles, Settings, Organizations and `organizationsApi.ts` | Current call sites and DTO expectations | Source of observed operation inventory |
| Worker tests: `test/route-completeness.test.ts`, `auth.test.ts`, authorization and regression tests | Existing coverage inventory | Files exist; not executed for this task |
| Admin UI tests: organization API, profile transfer, settings helpers, credentials and login diagnostics | Existing UI-side coverage | Files exist; not executed |
| `crates/bins/admin-slint/Cargo.toml`, `src/main.rs` | Potential future owner crate | Phase 0 shell only; no client module currently established |
| `.github/workflows/cloudflare-worker-check.yml`, root CI and Slint workflow | CI requirements/results | Worker workflow inspected; current-main Worker pass not verified |

## Proposed paths / ownership

The supplied paths `src/client/{mod.rs,login.rs,users.rs,profiles.rs,policy.rs,orgs.rs}` were checked at repository root and as exact suffixes elsewhere: none matched. Do not create them until the integration coordinator confirms the crate-relative anchor. `crates/bins/admin-slint/src/` is a candidate only, not an approved inference.

## Verified contract concerns

- Worker source SHA differs from the embedded source stamp in the existing API-contract document.
- Organization-admin create response has `is_admin` and `role` but no `created_at`/`updated_at`; the existing UI type expects timestamps and omits the returned fields.
- Error envelope shapes vary, including string errors, optional code/category/retryability, safe metadata and structured command errors.
- Mobile-access writes use `expected_version`; policy/legal-hold create uses idempotency keys. Do not generalize either mechanism to other mutations.

## Execution limits

Source inspection and GitHub Actions records only. No local checkout, production calls, or mutation tests. The local environment had Node 22/npm 10 and lacked Rust/Cargo; Worker workflow specifies Node 24 and repository Rust toolchain is 1.97.1. See the final report for CI evidence and blockers.