# B-01 — BOUND Blocker and Decision Register

**Current phase: Wave 0 / Gate 0b closure. Gate 0b remains BLOCKED pending mandatory evidence, decisions, approvals, and signed Contract Freeze v1. The project owner has authorized the assistant to pursue closure and then implement the approved B-01 scope once the gate criteria are satisfied; this is not a substitute for the gate artifacts.

| ID | Requirement / blocker | Evidence and impact | Responsible owner(s) | Exit evidence |
|---|---|---|---|---|
| G0B-01 | Gate 0b / Contract Freeze v1 | Governing BOUND plan explicitly prohibits production migration implementation before signed Gate 0b. Blocks all production `src/client/*.rs` modules. | Project owner / integration coordinator | Decision log records approver, date, baseline, accepted evidence, frozen contracts and explicit scoped authorization |
| G0B-02 | Crate/path confirmation | BOUND plan and B-01 ownership packet identify crate-relative `src/client/` under the Slint Admin lane; existing crate is `crates/bins/admin-slint/`. Gate remains closed; no production files created. | Repository owner / integration coordinator | Path and ownership recorded in canonical decision log |
| G0B-03 | C-01 source baseline | `main` SHA `b056c6ef204080446e84b31f3d35a226e6a40867`; Worker source blob `f587f240c51ce84ec6d19db3b4de5f1a944e2e3f`; `docs/production/api-contract.md` embeds `38c8975fd6616376d51e016b3e718b7736888514`. Contract is source-level only and does not prove deployed revision. | B-01 + Worker/API contract owner | Route-by-route reconciliation pinned to selected source revision, explicit deployed-revision limitation, reviewed C-01 draft |
| G0B-04 | Q-01 token persistence/session lifecycle | Open in the governing packet; B-01 cannot select keyring vs in-memory or define refresh/logout lifecycle. | Product/security owner + B-02 | Approved Q-01 decision, C-07/C-08 contract and tests |
| G0B-05 | Q-05 profile membership/wire representation | Open; profile lane must not choose UI/wire semantics independently. | Product owner + B-08, consulted by B-01 | Recorded decision and read/write round-trip fixtures |
| G0B-06 | Q-14 preference persistence | Open; affects session/localization contract. | Product owner + B-02/B-04 | Recorded decision/fallback with tests |
| G0B-07 | Q-16/U-05 ID display digits | Open; display policy must not change canonical wire IDs. | Product owner + B-04/Desktop owner | Recorded decision and locale serialization tests |
| G0B-08 | U-04 policy transitions | Open; blocks policy state-changing UI/client behavior that would depend on transition semantics. Independent non-policy client work is not blocked by U-04 once Gate 0b opens. | Product/policy owner + B-09, consulted by B-01 | Approved state diagram/table with concurrency and retry semantics |
| G0B-09 | Slint feasibility evidence U-01/U-02/U-03/U-10/U-12 | Required spikes are still pending in the execution packet; Gate 0b cannot pass without evidence and accepted fallback/block dispositions. | B-13 + B-03/B-04/B-12 and designated testers | Reproducible per-platform reports with versions, commands/manual steps, evidence and approved disposition |
| G0B-10 | C-03 error taxonomy | Worker responses have heterogeneous error envelopes; no approved shared code-to-localization map was found in the accessible packet. | B-01 + B-04 + B-02 | Versioned C-03 map and tests; no raw body/secret exposure |
| G0B-11 | C-09 fixture schema/version | Synthetic candidate cases are not authoritative until B-13 approves schema/version and provenance. | B-13 with B-01 | Schema, validator, valid/invalid fixtures, route/error coverage and recorded approval |
| G0B-12 | Organization-admin create DTO | Worker response and current UI row expectation reportedly disagree on timestamps vs `is_admin`/`role`; no approved resolution found. | B-01 + B-10 + Worker/API owner | Canonical DTO decision and regression test against verified response shape |
| G0B-13 | Security/license CI failure | Run [40336191173](https://github.com/SMozaff/Onyx-Framework/actions/runs/40336191173), job “Security audits / rust-advisories”, step “Advisory and source-policy audit”: `cargo deny check advisories bans licenses sources`; summary `advisories ok, bans ok, licenses FAILED, sources ok`. Rejected license metadata includes BSL-1.0, GPL-3.0-only, Unlicense and Slint custom LicenseRef identifiers. Do not change allowlist or suppress the check without policy-owner approval. | Security/CI maintainer + repository owner | Reviewed license policy disposition and fresh green required-check run |
| G0B-14 | B-01 docs PR CI | Latest verified PR #329 head `32d7dae99971d7b7cc70331298f62d4d844cfa05`, run [40426925219](https://github.com/SMozaff/Onyx-Framework/actions/runs/40426925219) **FAILED**: `Security audits / rust-advisories` → `Advisory and source-policy audit` failed on license policy; `merge-gate` failed as a consequence. The other reported jobs in this run passed. | CI maintainers / PR owner | Required checks green on current PR head |
| DEP-01 | Rust implementation verification environment | Local Rust/Cargo unavailable in earlier environment; no B-01 Rust tests executed. | B-13/CI owner | Approved Rust 1.97.1 CI results for the eventual implementation commit |
| DEP-02 | Documentation handover | User approved `reports/slint-migration/B-01/`; PR #329 contains the draft report packet but is still open and failing CI. | B-01 / integration coordinator | Reviewed/merged docs PR after required checks pass |

## Parallel work that remains authorized

- Inspect/refresh source-derived C-01 draft and identify exact DTO/source mismatches. The new `B-01_C01_Dispatch_Reconciliation.md` records dispatch predicates and specific path/query contract gaps.
- Prepare deterministic contract-test cases and test-harness proposals without treating them as approved C-09 fixtures.
- Support CI failure diagnosis with evidence; do not change shared license policy without owner approval.
- Coordinate C-03/C-09 reviews and update the existing handover as actual decisions arrive.

## Explicitly blocked

- Creating production Worker client modules or wiring them into the Slint shell.
- Freezing C-01/C-03/C-09 or claiming Contract Freeze v1.
- Choosing token persistence/session behavior.
- Implementing policy state transitions before U-04.
- Fabricating organization-admin timestamps, adding unverified routes, or using unchecked casts.
- Modifying shared license policy or required CI conditions without the responsible owner's review.

**The project owner's authorization to proceed is recorded, but no technical decision, feasibility report, cross-workstream approval, license-policy disposition, or Contract Freeze v1 is inferred from that authorization. No item is resolved by this register alone; closure requires the evidence/approval stated in the exit column.**

## Dated reconciliation addendum — 2026-11-10

The table above is retained as the original B-01 blocker snapshot. Several rows describe earlier open questions and must not be used as the current decision register. This addendum does not close any Gate 0b blocker.

| Existing ID / area | Later evidence or decision | Current interpretation | Closure status |
|---|---|---|---|
| G0B-01 | Owner's “Approved for full migration” approval is recorded in the decision log, but exact accepted evidence, frozen contract versions, fallbacks and scoped Gate 0b sign-off fields remain incomplete | Migration direction approval is not Gate 0b approval | OPEN / BLOCKED |
| G0B-02 | B-01 packet identifies `crates/bins/admin-slint/src/client/` as the crate-relative destination | Path is documented in packet; record/accept it in the canonical gate record before implementation | OPEN pending gate acceptance |
| G0B-03 | `docs/production/api-contract.md` was re-pinned to a 2026-11-01 source baseline; B-01 dispatch reconciliation later adds source-pinned findings | Source-level contract still requires current-baseline review and DTO/handler reconciliation; it does not prove deployed Worker behavior | IN PROGRESS |
| G0B-04 | Q-01 Admin memory-only rule is owner-approved; Desktop persistence distinction is owner-recorded in Contract Freeze v1; detailed refresh-token-only policy is coordinator-recorded under standing authority | No longer an unanswered choice of Admin vs Desktop persistence; lifecycle contracts, tests and real OS-keyring verification remain | IN PROGRESS |
| G0B-05 | Q-05 membership representation is recorded in decision log; validator work is documented | Decision exists; route-level validation and read/write round-trip evidence remain | IN PROGRESS |
| G0B-06 | Q-14 preference schema direction was owner-approved 2026-10-29 | Decision exists; committed schema, defaults, fallback and tests remain missing | OPEN |
| G0B-07 | Q-16/U-05 display-only Latin digit policy is recorded; Admin helper and tests exist | Policy is not an unanswered question; cross-screen/Desktop use and locale tests remain | IN PROGRESS |
| G0B-08 | U-04 lifecycle decision and related Worker/Rust evidence are in decision log | Decision exists; full parity/concurrency/error tests remain | IN PROGRESS |
| G0B-09 | Spike results and later Windows/Slint evidence exist | Original “spikes pending” label is historical; manual/device/platform evidence and accepted dispositions remain incomplete | IN PROGRESS |
| G0B-10 | C-03 mapping direction approval is recorded; proposal file remains draft | Approval of direction does not freeze a concrete mapping artifact | OPEN |
| G0B-11 | C-09 schema direction approval is recorded 2026-10-29; provenance inventory exists | Exact schema identity/version, validator, fixtures and provenance acceptance remain | OPEN |
| G0B-12 | B-01 final report and dispatch reconciliation retain DTO mismatch | No canonical DTO decision/test found in reviewed evidence | OPEN |
| G0B-13 | Recorded CI runs failed the license audit | Policy-owner disposition and fresh green required-check run remain required | OPEN |
| G0B-14 | Run 40426925219 is a historical failing docs-PR run, not a current-head status | Re-run required checks on the intended current PR head | OPEN |
| DEP-01 | Earlier local toolchain limitation was environment-specific | Require pinned Rust 1.97.1 CI evidence for the relevant implementation commit | OPEN |
| DEP-02 | B-01 packet PR #329 was merged on 2026-10-27 (merge commit `09c7ba0e691cf0061ec5f9c898e27ecc8b881018`). Follow-up run 40428710390 completed with security audit jobs cancelled and `merge-gate` failed. | Handover packet is merged; the required-check evidence is not green. Keep distinct from Gate 0b approval. | IN PROGRESS; current required checks and any missing review evidence must be verified |

For current consolidated status, dependencies, responsible parties and explicit closure evidence, see [MASTER-MIGRATION-RECORD.md](../../../docs/migrations/slint/MASTER-MIGRATION-RECORD.md). The project remains in Wave 0 / Gate 0b closure. No production B-01 client modules, deployment or cutover are authorized by this addendum.
