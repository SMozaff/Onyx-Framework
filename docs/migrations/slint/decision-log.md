# ONYX Slint Migration — BOUND Decision Log

**Status:** Gate 0b OPEN — decisions below do not constitute Gate 0b sign-off.  
**Repository baseline inspected:** `913a36a5606c0241a15104cebae2c915997ea02b` (main, 2026-10-27).  
**Decision update:** 2026-10-27.  
**Authority:** User-confirmed decisions in the ONYX project conversation.  
**Rule:** Preserve unresolved items as blockers; do not infer approval from recommendations or investigations.

## Confirmed decisions

### P-01 — Desktop hosting architecture

- **Decision state:** Investigation authorized; final architecture not yet approved.
- **User decision:** Option B — separate native Slint host.
- **Authorized investigation:** B1 — separate executable reusing approved Rust backend crates in-process.
- **Boundary:** This authorizes feasibility investigation only. It does not authorize replacing the Tauri default, deleting Tauri/React, freezing the architecture, or claiming Gate 0b passed.
- **Next evidence required:** dependency graph and feature/build validation; shared composition/session boundary; authority and token ownership; event/subscription lifecycle; startup/shutdown/restart/error behavior; data-directory and packaging behavior; cross-platform build and smoke-test evidence.
- **Current finding:** `client-composition` is a reusable workspace library and `AppState` exposes command/query/event registries, but `desktop-shell` currently owns Tauri-bound session orchestration, secure storage wiring, and the composition setup. A Slint host should not depend on the Tauri shell as its runtime. Extracting a shell-independent runtime/session boundary is likely required. See [P-01 B1 investigation](P-01-B1-investigation-2026-10-27.md).

### Q-01 — Admin token persistence

- **Decision state:** Approved.
- **Decision:** Keep tokens in memory only; do not persist them to disk or an OS keyring.
- **Contract obligations:** tokens must not be exposed to Slint properties/UI state, logs, diagnostics, crash reports, or disk; define expiry, refresh, logout, revocation, and process-restart reauthentication behavior; add tests for all of these.

### Q-05 — Profile team-membership representation

- **Decision state:** Approved.
- **Decision:** Preserve the existing list/API contract (`Vec<String>` in the Rust DTO) and the existing semicolon-delimited editor behavior.
- **Constraints:** Do not invent a team picker or team catalog without a verified catalog and authorization contract. Test whitespace, empty and duplicate entries, semicolon-containing values, serialization round trips, and server validation/authorization.

### Q-14 — Theme and language preferences

- **Decision state:** Approved.
- **Decision:** Persist theme and language preferences across launches.
- **Remaining contract work:** define storage location, schema/versioning, defaults, corruption fallback, and tests.

### Q-16 / U-05 — ID digit display

- **Decision state:** Approved.
- **Decision:** Display Latin digits for IDs. This is display-only; canonical values, serialization, identifiers, and API data remain unchanged.
- **Remaining contract work:** UI formatting tests, including mixed RTL/LTR contexts.

### U-04 — Policy lifecycle and transitions

- **Decision state:** Approved, subject to implementation evidence and Worker/Rust parity tests.
- **Accepted model:** Policy lifecycle `Active -> Retired`, with Retired terminal. Policy versions move `Draft -> Published` and published versions are immutable; at most one pending draft. Evaluation and violation registration require an Active policy with a published version. An unknown rule key returns an explicit missing/null result, not a silently coerced false value. Mutations must use expected version/epoch checks; on conflict, reread and require fresh user intent. Do not blindly replay state-changing operations. Retry an ambiguous create only with the same idempotency key and payload when the endpoint explicitly supports idempotency.
- **Current evidence:** Rust policy aggregate rejects `EvaluatePolicy` and `RegisterViolation` on Retired policies. Worker command semantics require Active for both; existing Worker tests assert rejection. The Worker test's comments describing a Rust mismatch are stale and should be corrected without changing the behavior.
- **Remaining work:** parity tests for lifecycle, published-version requirement, missing rule keys, draft uniqueness, optimistic concurrency, and error mapping. Review exact endpoint behavior before changing production semantics.

## Gate 0b blockers — still open

These items are not closed by the decisions above:

1. **P-01:** complete B1 architecture evidence and approve the final boundary, state/session authority, typed interfaces, event behavior, failure/restart semantics, packaging, and exact implementation scope.
2. **Q-01:** implement and test the in-memory token contract and lifecycle behaviors.
3. **Q-14:** freeze persisted preference schema, defaults, fallback, and tests.
4. **Q-05:** implement contract-level validation and round-trip tests; confirm actual server validation behavior.
5. **Q-16/U-05:** verify display-only Latin-digit formatting without changing canonical data.
6. **U-01:** reproducible Windows/Linux table sorting/paging, multiline editor, Persian IME composition, and mixed RTL/LTR cursor/selection evidence.
7. **U-02:** Windows UI Automation and Linux AT-SPI/screen-reader evidence.
8. **U-03:** bundled font source/licensing, Latin/Persian glyph coverage, shaping samples, and per-OS render evidence.
9. **U-04:** complete Worker/Rust parity tests and resolve version/epoch/error behavior against actual command handlers.
10. **U-10:** reduced-motion source/fallback and per-OS evidence.
11. **U-12:** complete dark-theme tokens, WCAG AA contrast pairs, and automated report.
12. **C-01:** re-pin API contracts to current Worker routes/handlers and review; existing embedded baseline is stale.
13. **C-03:** review/version typed error mapping.
14. **C-09:** approve fixture schema and provenance.
15. **Governance:** identify integration coordinator; record exact baseline, accepted evidence, frozen contract versions, approved fallbacks, approver/date, and scoped authorization in the formal Gate 0b sign-off.

## Baseline and governance notes

- Main baseline inspected: `913a36a5606c0241a15104cebae2c915997ea02b`.
- The plan-referenced canonical decision log did not exist on this baseline; this file establishes the proposed canonical record in the expected `docs/migrations/slint/` area.
- The plan path `common/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md` returned 404 on the current main branch. The supplied planning artifact remains the governing source for this work, but its in-repository canonical location must be reconciled.
- Existing Slint feasibility CI is evidence for a spike only; it is not proof of full production UI, Persian IME, accessibility, font shaping, reduced motion, or theme-contrast readiness.
- Do not start B-01 production Worker-client implementation until Gate 0b's required evidence and formal approval are complete.

## Gate 0b formal sign-off record

**Status:** Owner approval recorded; Gate 0b NOT passed. Blockers 1–14 above remain open, and the required fields below are incomplete.

| Field | Value |
|---|---|
| Owner approval (verbatim) | "Approved for full migration" |
| Approver | Project owner (fanoos.media@gmail.com) |
| Approval date | 2026-10-29 |
| Scope | Full migration (Admin and Desktop, Slint), per the BOUND plan |
| Exact baseline | origin/main `9ac3964` (PR #342 and #343 merged) |
| Integration coordinator | **OWNER TO PROVIDE** |
| Accepted evidence (blockers 1–14) | **OWNER TO PROVIDE** — none accepted yet |
| Frozen contract versions | **OWNER TO PROVIDE** |
| Approved fallbacks | **OWNER TO PROVIDE** |

**Effect:** Tauri/React remains the production default. No production change, B-01 implementation, or Worker deploy follows from this record alone.

## Owner decisions recorded 2026-10-29

- **P-01 (desktop boundary):** Owner approved the boundary. Reference: [P-01 B1 investigation](P-01-B1-investigation-2026-10-27.md). The boundary text is not yet committed as its own file; it must be added before Gate 0b evidence review.
- **Q-14 (preference schema):** Owner approved the schema. The schema itself must be committed (fields, defaults, fallback) before it can be frozen.
- **C-09 (fixture schema):** Owner approved the fixture schema. Provenance must be committed alongside it.
- **C-03 (typed error mapping):** Owner approved the mapping. Reference the committed mapping file when it exists.

**Gate 0b status:** still OPEN. Approvals here do not close blockers 1–14 and do not pass the gate.

**Open:**

## Gate 0b coordination and font (2026-10-29)

- **Bundled font (U-03):** Vazirmatn, SIL Open Font License 1.1. Owner confirmed the font and its license.
- **Integration coordinator (blocker 15):** Claude (AI assistant), named by the owner. Coordinates evidence collection and keeps this record current. The owner remains the approver of record for Gate 0b sign-off.

## U-04 error codes (2026-10-31)

- **Decision:** Policy precondition failures use the specific codes `DRAFT_ALREADY_PENDING`, `NO_DRAFT_VERSION`, and `NO_PUBLISHED_VERSION` on the API. The Worker's generic `COMMAND_PRECONDITION_UNSUPPORTED` is not used for these cases.
- **Status:** Decided. Worker implementation pending.
- Source matrix: `docs/migrations/slint/u04-policy-parity.md`.
## C-09 fixture provenance (2026-10-31)

- **Recorded:** Inventory of fixtures with git provenance in `docs/production/fixture-provenance.md`.
- **OPEN:** The approved fixture schema is not identified in the repo. Owner to confirm which schema (candidate: `shared/diagnostics/diagnostic-event.schema.json`, version 1.0).
## C-01 API contract re-pin (2026-10-31)

- **Decision:** `docs/production/api-contract.md` re-pinned to main `98a2258`. Verified by the route-completeness test (42 passing).
- **Reason:** The contract's routes already matched the Worker. Only the baseline commit was stale.
- **Not changed:** Historical B-01 reports keep their original baseline. They record what was reviewed at that time, and rewriting them would misstate the history.

## Q-05 team names and semicolons (2026-10-31)

- **Decision:** Team names containing `;` are rejected at input. Team member lists keep the `;`-separated format.
- **Reason:** `;` is the separator in `team_ids` and the editor list. A name containing it would be split into two entries. Team IDs (`tm…`) never contain `;`, and no legitimate name needs it.
- **Decided by:** Coordinator, per the owner's standing instruction to decide minor matters and document them. Owner informed.

## Future priority: diagnostics collection (after migration, 2026-10-31)

- **Status:** Not started. Deliberately deferred until the Slint migration is complete.
- **Goal:** Store diagnostic events (`shared/diagnostics/diagnostic-event.schema.json`, v1.0) on Cloudflare so failures can be counted by `error_code` and component.
- **Current state:** Admin Shell login events go only to the browser console and a window event. Worker events are not collected.
- **Candidate approach (not decided):** Cloudflare-native logging. Volume is small and needs no computation. Options to evaluate: Workers Logs, Logpush, or a D1 table.
- **Owner:** To be assigned when the migration closes.

## U-10 motion preference fallback (2026-10-31)

- **Decision:** If the OS motion setting is unknown or unreadable, the app uses reduced motion.
- **Reason:** Reduced motion is the safer default for users who may need it. It is also the value `MotionPreference::resolve(None)` already returns in `crates/applications/client-composition/src/motion.rs`.
- **Decided by:** Coordinator, per the owner's standing instruction. Owner confirmed on 2026-10-31.
- **Still to do:** Per-OS readers and Slint wiring.
