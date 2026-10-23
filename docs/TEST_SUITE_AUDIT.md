# Test Suite Audit

## Authentication model applied

The current remote-client authentication path is the Cloudflare Worker at `deploy/cloudflare/edge-gateway`. All-Father uses ONYX-managed username/password credentials, and the Worker uses D1 for the canonical principal, account and organization state, roles, and session revocation. Admin and Staff use ONYX username/email plus password. Google/Supabase All-Father identity exchange is retired and must not be described as the current model.

The native Rust/Axum API is a separate backend implementation. Its login route reserves the `allfather` username; it is not an alternate All-Father login path. Worker and native API tests must be assessed against the backend they actually exercise.

See [the authoritative authentication model](SECURITY_AUTHENTICATION_MODEL.md), [the Worker README](../deploy/cloudflare/edge-gateway/README.md), and [the historical Google/Supabase flow](authentication/GOOGLE_OAUTH_FLOW.md).

## Removed from the Rust journey runner

- `approval_workflow.rs`, `session_revocation.rs`, and `production_bootstrap.rs` were removed from the legacy Rust journey runner because they depended on bootstrap/seeded administrator assumptions or legacy Axum/PostgreSQL user routes that did not test the current Worker/D1 All-Father authentication path.
- The harness's fixed test administrator credentials and `bootstrap_and_login` helper were removed. Do not restore fixed credentials or bootstrap endpoints merely to make CI pass.

These removals do not imply that the underlying security behaviors are unnecessary. Where behavior is required in the current product, it needs tests against the actual authoritative implementation.

## Existing native tests

- Domain journey tests such as mission lifecycle, task workflow, and conflict resolution exercise native domain/application behavior and should be described as such.
- Workspace unit and integration tests remain separate from Worker tests.
- Migration idempotency and rollback checks validate their respective database contracts; they do not prove Worker/D1 behavior.
- Native API tests do not, by themselves, establish Worker route parity or deployed behavior.

## Required Worker verification

Worker tests should cover the actual supported path:

- Successful and failed ONYX credential login for the canonical All-Father principal.
- Invalid/missing credentials and reserved-principal behavior.
- D1-backed role, active-account, active-organization, and tenant authorization.
- All-Father-only organization/admin capabilities and rejection of forged client role/admin metadata.
- Access/refresh token rotation and revocation.
- Retired exchange route rejection.
- Local-only provisioning/smoke behavior without sending credential tests to production.

## Evidence status

This document describes test scope and architecture; it does **not** claim that all GitHub Actions checks pass, that the production account has been provisioned, or that live login has succeeded. Those claims require current workflow and deployment evidence tied to a specific commit.
