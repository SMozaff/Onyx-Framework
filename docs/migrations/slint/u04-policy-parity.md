# U-04: Policy Worker/Rust parity matrix

**Status:** Evidence gathered 2026-10-30. Worker behavior tests added 2026-11-01 in `deploy/cloudflare/edge-gateway/test/u04-parity-behavior.test.ts` (11 passing). Remaining gaps are listed below as todo.

Sources:
- Rust: `crates/domains/policy-domain/src/aggregate.rs` (`decide`), `error.rs`.
- Worker: `deploy/cloudflare/edge-gateway/src/command-semantics.ts` (`policy-*` special cases).

## Outcome matrix

"Accept/Reject" is the outcome both sides agree on. The error code column shows what each side returns on rejection.

| # | Case | Rust | Worker | Outcome | Error code (Rust / Worker) |
|---|---|---|---|---|---|
| 1 | Create version while a Draft exists | `DraftAlreadyPending` | `COMMAND_PRECONDITION_UNSUPPORTED` | Match: reject | **Mismatch** |
| 2 | Publish with no Draft | `NoDraftVersion` | `COMMAND_PRECONDITION_UNSUPPORTED` | Match: reject | **Mismatch** |
| 3 | Evaluate with no Published version | `NoPublishedVersion` | `NO_PUBLISHED_VERSION` | Match: reject | Names differ (same meaning) |
| 4 | Evaluate an unknown rule key | Accepts, result `None` | Accepts, result `null` | Match: accept | Match |
| 5 | Register violation with an unknown rule key | Accepts | Accepts | Match: accept | Match |
| 6 | Register violation with no Published version | `NoPublishedVersion` | `NO_PUBLISHED_VERSION` | Match: reject | Names differ |
| 7 | Evaluate or register on a Retired policy | `InvalidTransition` | Rejected by the Active-only stay rule | Match: reject | Not compared |
| 8 | Publish, create version, or retire on a Retired policy | `InvalidTransition` | Rejected by the Active-only stay rule | Match: reject | Not compared |
| 9 | Retire an already Retired policy | `InvalidTransition` | `from: ["Active"]` rejects | Match: reject | Not compared |
| 10 | Create a policy with an empty name | `MissingField` | Not checked here (create-scope check only) | Not verified | Not verified |

## Verification status

- Rows 1 to 9: covered by Worker tests (2026-11-01).
- Published versions are immutable on the Worker side: publish needs a Draft, and creating a version does not edit a Published one.
- Retire is terminal on the Worker side (rows 7 to 9).
- **Still todo:** row 10 (empty policy name) and optimistic concurrency (`expected_version`). Both need a decision on the Worker change, or a Rust-side check on the same case.
- Rust-side behavior tests for these rows are not in this lane.

## Decision (2026-10-31): specific error codes

Rows 1, 2, 3, and 6 use the Rust names on the API:

| Rust error | API code |
|---|---|
| `DraftAlreadyPending` | `DRAFT_ALREADY_PENDING` |
| `NoDraftVersion` | `NO_DRAFT_VERSION` |
| `NoPublishedVersion` | `NO_PUBLISHED_VERSION` |

Status: **decided, not yet implemented.** The Worker change alters production responses and needs a deploy through the normal gate. Implementation is a separate step.

## Stale comment

The Worker test comments describing a Rust mismatch are stale, per the decision log. Correct the comments without changing behavior.
