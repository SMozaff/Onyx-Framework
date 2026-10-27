# B-01 Worker Client — Handover

## Current state

Wave 0 is partial: source inventory and endpoint audit completed against `main` SHA `b056c6ef204080446e84b31f3d35a226e6a40867`; C-01 and C-03 drafts, test-gap plan and blockers prepared. Gate 0b is not approved. This handover contains documentation only.

## Do not assume

- Do not assume `src/client/*.rs` means repository-root `src`; no matching client module paths were found. The Slint crate is a candidate anchor only.
- Do not treat C-01/C-03/C-09 as frozen.
- Do not synthesize `created_at`/`updated_at` for organization-admin create.
- Do not apply generic retries to mutations. Evidence supports idempotency keys for policy/legal-hold creation and `expected_version` for mobile-access writes.
- Do not treat existing test files as executed tests or the Slint scaffold CI as a client pass.

## Baseline evidence

- Main HEAD: `b056c6ef204080446e84b31f3d35a226e6a40867`.
- Worker source blob: `f587f240c51ce84ec6d19db3b4de5f1a944e2e3f`.
- Root CI run 40336191173 failed the license audit and merge gate.
- Slint scaffold run 40336190967 passed Ubuntu/Windows scaffold jobs only.
- Local tests/builds were not run; no live/prod Worker calls were made.

## Next owner actions

1. Integration coordinator confirms path ownership and Gate 0b criteria.
2. B-01/B-10 settle organization-admin create response contract.
3. B-01/B-04 approve error mapping and localization keys.
4. B-13 approves C-09 schema/version and validates candidate fixtures.
5. Product owner/B-02 close Q-01; B-08 closes Q-05; B-09 closes U-04.
6. Security/CI maintainers resolve license audit and rerun required checks.
7. Only after authorization, implement the approved client and tests in a separate PR.

## Suggested first implementation validation

Use Worker CI's Node 24 environment and disposable local D1. Run `npm run check`, `npm test`, the Worker integration workflow, Admin UI tests, and Rust 1.97.1 formatting/build/tests for the approved crate. Report exact commit SHA, job URLs, failures and test counts. Never use production credentials/data for contract tests.