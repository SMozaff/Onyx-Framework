# U-04: Policy Worker/Rust parity matrix

**Status:** Evidence gathered 2026-10-30 by reading both implementations. Behavior tests for the cases marked "Not yet verified" are still needed.

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

## Not yet verified

- Published versions are immutable (no command edits a Published version). Check both sides.
- Optimistic concurrency (`expected_version`). Check the Worker's version check against Rust's handling.
- Retire is terminal on both sides. Behavior is consistent in the code read so far; a test is still needed.

## Owner decision required

Rows 1, 2, 3, and 6: decide the error codes the API returns. Options:

1. **Keep the Rust names.** The Worker returns `DRAFT_ALREADY_PENDING`, `NO_DRAFT_VERSION`, and `NO_PUBLISHED_VERSION`. This changes Worker responses for rows 1 and 2.
2. **Keep the Worker's generic code.** Rust maps every precondition failure to `COMMAND_PRECONDITION_UNSUPPORTED`. This loses information in Rust.

Recommended: option 1, because it gives the Slint UI specific messages. Owner to confirm.

## Stale comment

The Worker test comments describing a Rust mismatch are stale, per the decision log. Correct the comments without changing behavior.
