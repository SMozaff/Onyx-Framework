# Test Suite Audit (2026-10-10)

## Keep
- Keep workspace unit/integration tests as the default correctness and security gate.
- Keep tests/end-to-end/session_revocation.rs: its cross-replica logout and deactivation assertions cover a real security boundary. Repair fixture setup rather than deleting the assertions.
- Keep tests/end-to-end/approval_workflow.rs: its 401 is a test-auth fixture/configuration defect, not a reason to remove the API assertion.
- Keep tests/end-to-end/production_bootstrap.rs: it verifies production initialization does not create a known administrator.
- Keep mission/task/conflict state-transition assertions, but later remove their unnecessary Postgres container startup and result-recording calls: they currently test in-memory domain objects rather than database persistence or HTTP E2E.

## Remove from mandatory execution
- notification_sync.rs, p2p_sync.rs, and background_sync.rs are ignored placeholders, not executable tests. Remove them from the journey runner until native-client integration exists.
- The Team 8 chaos suite did not inject faults. chaos_runtime.rs advances a counter using yield_now (or sleeps for 15 minutes in real-time mode); companion tests assert constants, a local temp file, a vector-clock relation, or a fabricated lag value. They do not partition a network, crash a process, fill a filesystem, skew a clock, or fail over a database. Remove this fake suite from mandatory CI until tests exercise real services.

## Current failures to fix, not delete
- Approval workflow login returns 401 because the harness assumes a seeded All-Father account, while API initialization seeds a test administrator only when ONYX_TEST_USERNAME and ONYX_TEST_PASSWORD are supplied. Make test auth setup explicit before constructing ApiState.
- Session revocation makes the same hard-coded test-admin assumption. Provision a deterministic test account and retain the cross-replica assertions.
- Replace unchecked unwrap calls in security setup with contextual errors/assertions.

## CI cost and redundancy
- scripts/ci-pipeline.sh excludes e2e and chaos from the broad workspace test command, then runs E2E separately. The exclusion prevents duplicate execution; it is not itself redundant.
- Keep migration idempotency/rollback checks because they verify a separate database contract.
- Do not remove security tests merely because they fail.

## Out of scope
The 100-VU/60-second k6 gate is outside this change. If it is too expensive at this stage, change its workflow trigger or make it scheduled/manual separately. This audit does not claim the remaining suite is green; GitHub Actions must confirm results.