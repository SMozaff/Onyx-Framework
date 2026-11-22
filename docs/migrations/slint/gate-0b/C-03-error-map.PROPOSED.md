# C-03 typed error map — PROPOSED draft (2026-11-23)

**Status: PROPOSED — requires B-01 + B-04 + B-02 and contract-owner approval.** G0B-10 stays OPEN.
The mapping direction was owner-approved 2026-10-29 but the versioned committed map is not frozen.

## Source

- `docs/production/error-mapping.md`: 88 distinct Worker codes; each response carries `error`,
  `category`, `retryability`. Worker source: `deploy/cloudflare/edge-gateway/src/index.ts`.
- `reports/slint-migration/B-01/B-01_C03_Error_Mapping_Proposal.md`: DRAFT, normalization layers
  and generic categories (network/timeout, auth, forbidden, not-found, conflict, validation,
  rate-limited, server, protocol/unknown).
- Owner decisions 2026-11-15: four stable concurrency identifiers; UI maps codes and never infers
  from error text.
- Implemented Slint mapping (source evidence): `admin-slint/src/policy.rs::concurrency_code` reads
  `error.code` from `{"error":{"code":"…"}}` and plain `error` strings from other shapes;
  `admin-slint/src/main.rs::concurrency_text` selects catalog text.

## Proposed stable map

| Stable code / category | Slint text key | Source evidence |
|---|---|---|
| `LIFECYCLE_EPOCH_REQUIRED` (400, CONCURRENCY) | `settings.concurrencyLifecycleEpochRequired` | Worker `commandRoute` ~1178; decision 2026-11-15; implemented in `policy.rs` |
| `AUTHORITY_EPOCH_REQUIRED` (400, CONCURRENCY) | `settings.concurrencyAuthorityEpochRequired` | Worker ~1179; implemented |
| `EPOCH_MISMATCH` (409, CONCURRENCY) | `settings.concurrencyEpochMismatch` | Worker ~1181; implemented |
| `VERSION_CONFLICT` (409, CONCURRENCY) | `settings.concurrencyVersionConflict` | Worker ~1183 and other routes; implemented |
| Missing projection epochs (client-side) | `settings.policyMissingConcurrencyEpochs`, `settings.legalHoldMissingConcurrencyEpochs` | Slint `PolicyDetail::MissingConcurrency` / `HoldDetail::MissingConcurrency`; no command sent |
| `FORBIDDEN` | `auth.adminAccessRequired` | Admin UI uses existing key |
| Transport/timeout/server failures | `common.connectionLost` / `status.failed` | `admin-slint/src/auth.rs`, `main.rs::error_message` |
| Unknown code | generic `status.failed`; never raw body or error text | Proposed rule |

## Frozen-map requirements (not met)

- Versioned mapping approved by owner; keys approved in EN **and** FA (D-01/D-03 currently open —
  the four concurrency keys are English placeholders in `fa.json`).
- Tests that every code in C-01 has a message key in C-06, and that unknown codes show generic text.
- No raw error body, password, token or secret may be logged or displayed.
- A regression test that the four concurrency codes are matched by identifier only, never by text.

This draft records the current source implementation; it is **not** an approval or a closure
of G0B-10.
