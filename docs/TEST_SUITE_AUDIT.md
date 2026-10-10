# Test Suite Audit (2026-10-10)

## Authentication model applied
The ONYX authentication system document supplied for this project is the basis for this audit: All-Father signs in through Google via Supabase Auth; Admin and Staff use ONYX-managed username/email + password; D1 is authoritative for ONYX users, roles, organization membership and authorization. There is no first-user/bootstrap login flow and no fixed test administrator account.

## Removed from the Rust journey runner
- approval_workflow.rs: depended on bootstrap_and_login and a seeded hard-coded All-Father username/password. It tests the retired Rust/Axum authentication path rather than the current Cloudflare Worker/D1/Supabase model.
- session_revocation.rs: depended on the seeded test administrator credentials and legacy Axum/PostgreSQL user routes. The security behavior is valuable, but this implementation is not a valid test of the current target architecture; reimplement it against Worker/D1 session revocation when that path exists.
- production_bootstrap.rs: tests a first-admin bootstrap/seed behavior that is not part of the authoritative authentication model.
- The harness's TEST_ADMIN_USERNAME, TEST_ADMIN_PASSWORD and bootstrap_and_login helper were removed. No fixed test credentials remain in this E2E harness.

## Keep
- mission_lifecycle.rs, task_workflow.rs and conflict_resolution.rs remain in the runner as domain journey tests. They still start Postgres despite primarily exercising in-memory domain objects; removing that container setup is a separate performance cleanup, and should only happen after checking whether their harness fixtures are needed.
- Workspace unit and integration tests remain enabled. This change does not remove them.
- Migration idempotency and rollback checks remain because they test a separate database contract.
- The fake chaos suite and empty client journey placeholders were already removed from mandatory execution because they did not exercise real failure/client behavior.

## Follow-up required
- Add focused Cloudflare Worker tests for the actual target auth flow: All-Father Supabase identity verification; Admin/Staff password verification; D1 role and tenant authorization; staff provisioning restrictions; and session revocation if implemented.
- Do not restore bootstrap endpoints or fixed test credentials to make CI pass.
- This pruning does not claim GitHub Actions is green. CI must confirm that the remaining suite compiles and runs.