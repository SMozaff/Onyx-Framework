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
