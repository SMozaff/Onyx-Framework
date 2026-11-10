# ONYX Framework — Slint Migration Master Record

**Status:** Consolidated audit draft — owner review required  
**Audit date:** 2026-11-10  
**Repository:** [SMozaff/Onyx-Framework](https://github.com/SMozaff/Onyx-Framework)  
**Audited baseline:** `main` at `c447433a520a83ea9f3fb39e9d70f8740af8a98b`  
**Migration method:** BOUND  
**Current gate:** **Gate 0b — BLOCKED / NOT PASSED**

> This is the existing master document introduced by PR #443, now updated as the migration's central status and navigation record. It does not itself approve technical decisions, freeze contracts, satisfy Gate 0b, authorize production implementation or authorize cutover. The project owner remains the Gate 0b approver.

## 1. Executive overview

### Objective and scope

The migration moves the ONYX Desktop and Admin client experience toward native Slint hosts while reusing approved Rust application/domain components and preserving the Cloudflare Worker as the supported remote API for distributed clients. Work is governed by the BOUND method and must proceed through small, reviewable steps.

The migration must preserve:
- The existing ONYX domain and application behavior and API contracts.
- A clear separation between Desktop and Admin session policy.
- The Cloudflare Worker/D1 identity and authorization authority for remote clients.
- Cross-platform behavior, accessibility, Persian text shaping/IME/RTL behavior, localization consistency and secure session handling.
- Existing Tauri production behavior until the required gate and release approvals authorize a change.

Out of scope for this documentation audit: application implementation, database/schema changes, production deployments, production cutover, CI policy changes and technical decision implementation.

### Current status

- PR #443 merged the original draft master record on 2026-11-10 (merge commit `c447433a`). PR #442, which reconciled open owner items in the decision log, merged immediately before it.
- **Track A relay authentication:** recorded as implemented and merged in PR #407. Source/unit-test evidence covers fail-closed behavior and session handling. Two live relay-ticket checks still require an available deployed Worker and authorized test account.
- **Track B:** the shared `desktop-session` crate is in use by the Slint host. The Tauri-free dependency check is recorded as passing (zero Tauri-family crates in the normal dependency tree). Lifecycle tests pass in the documented environment. Real OS-keyring tests and two-host coexistence on physical devices remain unverified.
- **B-01:** Wave 0 / Gate 0b closure work only. The B-01 client implementation remains blocked.
- **Localization:** canonical EN/FA catalogs and generated resources exist; Persian-speaker review of newly added FA strings is not recorded.
- **Cross-platform verification:** CI evidence exists for Windows/Linux compile/test paths in specific spikes, but this does not establish the full manual/device verification matrix, macOS verification or OS-keyring round trips on all three desktop OSes.
- **Web UI:** postponed by owner decision dated 2026-11-08; do not resume or start the deferred Vite dependency work without owner approval.

### Major risks and critical blockers

1. Gate 0b has not been signed; production migration implementation is prohibited until the formal gate is satisfied.
2. B-01 C-01/C-03/C-09 artifacts remain drafts or incompletely approved/frozen.
3. Security/license CI failure recorded in B-01 evidence requires a security/policy-owner disposition and a fresh green required-check run; do not weaken the policy.
4. P-01 real keyring/device and two-host coexistence evidence is incomplete.
5. Persian translation quality review and manual IME/RTL/accessibility evidence are outstanding.
6. File-transfer route selection cadence is now clarified as per file, but the exact Local transport semantics remain unresolved across source documents.
7. Backend O-1–O-6 decisions are recorded under delegated authority; they are not six fresh owner questions, but they must not be misrepresented as direct owner-approved decisions.
8. The referenced governing BOUND plan and Phase 0.5 localization decision file are missing at their documented repository paths; their contents/authority cannot be independently checked.

### Next actions

1. Keep Gate 0b blocked; reconcile every gate blocker against current evidence and preserve the formal approval requirements.
2. Update stale backend and P-01 status statements with dated amendments.
3. Resolve the remaining Local route transport/relay/P2P scope from source evidence or an explicit owner decision.
4. Obtain Persian-speaker review and the required manual/device evidence.
5. Complete B-01 contract approvals, source baseline and CI/security disposition.
6. Obtain signed Contract Freeze v1 with exact baseline, accepted evidence, frozen contract versions, fallbacks, approver/date and scoped authorization before any production implementation.
7. Restore or locate the authoritative BOUND plan and Phase 0.5 localization decision record, or obtain approval to update their dangling references.
8. Continue to consult this record before asking the owner to repeat a decision.

## 2. Documentation governance and authority model

### Authoritative navigation

- **This master record:** current migration overview, consolidated decision index, blocker status, milestone/gate status and links to detailed records.
- **`docs/migrations/slint/decision-log.md`:** append-only migration decision history and source attribution. Do not erase or silently rewrite historical entries.
- **`reports/slint-migration/B-01/B-01_Blockers_and_Decisions.md`:** detailed B-01/Gate 0b blocker evidence and exit criteria. Its original table contains historical status text; use its dated reconciliation addendum together with this master.
- **Domain specifications:** remain authoritative for their domain within their stated scope (for example, file transfer, authentication, backend architecture and localization).
- **Evidence reports:** show what was inspected or tested at a particular commit/environment. They do not automatically approve a decision or prove deployment.
- **Historical snapshots:** remain in place and retain their original dates and claims. Corrections are dated amendments, not silent edits.

The complete inspected inventory and disposition is in [DOCUMENT-INVENTORY.md](DOCUMENT-INVENTORY.md).

### Decision statuses

Use only:
- **APPROVED:** explicit owner approval is evidenced.
- **RECORDED:** documented, but direct owner approval is not established; includes coordinator decisions made under delegated standing authority.
- **PROPOSED:** recommendation or draft only.
- **PENDING:** an owner or designated authority decision is required.
- **IMPLEMENTED:** source implementation exists; this does not imply tests or deployment.
- **SUPERSEDED:** a later decision explicitly replaces it.
- **REJECTED:** explicitly rejected.

Implementation and verification labels are separate: **implemented**, **tested**, **reviewed**, **merged**, and **deployed** are not interchangeable.

Never elevate an AI-generated recommendation to owner approval. A delegated coordinator decision must retain its attribution and be labelled RECORDED unless explicit owner approval of that exact decision is evidenced.

### Permanent rules

1. Every migration decision has a unique ID in the central decision history.
2. Before asking the owner a question, inspect this master, the decision log, domain specifications, linked evidence, relevant PRs and available owner instructions.
3. Ask again only when the issue is genuinely unresolved or the evidence cannot establish the decision.
4. Record exact wording, date, decision-maker, approval status, original evidence, source, implementation state, affected documents, related blockers and superseding decision.
5. Preserve original decision text and history. Changes create a dated correction or superseding record.
6. Record owner decisions promptly and reconcile every affected supporting document.
7. Close blockers only when their stated exit evidence exists and is recorded; a proposed solution is not closure.
8. Every contributor/agent reads this master before continuing work or requesting a decision.
9. Do not use this master to bypass review, approval, CI, security, release or production controls.

## 3. Consolidated decision register

This register summarizes the current repository evidence. Detailed wording and historical context remain in the linked decision log and domain specifications.

| ID | Subject and exact recorded decision | Date / decision authority | Status | Implementation / evidence state | Related blockers / source |
|---|---|---|---|---|---|
| MIG-APPROVAL-01 | Owner approval text “Approved for full migration” covering Admin and Desktop Slint migration scope; does not satisfy Gate 0b | Owner, 2026-10-29 | APPROVED (migration direction only) | Formal Gate 0b fields remain incomplete; no production implementation authorization follows from this entry alone | G0B-01; [decision log](decision-log.md) |
| P-01 | B1: separate native Slint Desktop host reusing approved Rust crates in-process; Track A first, Track B conditional on Track A | Owner approval recorded 2026-11-04 | APPROVED (conditional) | Track A merged #407; Track B partially implemented; lifecycle/device/coexistence proof incomplete | Gate 0b; [decision log](decision-log.md), [P-01 investigation](P-01-B1-investigation-2026-10-27.md) |
| TRACKB-01 | Move session authority (login, refresh rotation, logout, bearer provider and storage record) into shell-independent `crates/applications/desktop-session`; keyring adapter remains platform code | Owner decision 2026-11-07 | APPROVED | Shared crate exists; source tests recorded; device keyring/coexistence evidence remains open | G0B-04; [decision log](decision-log.md) |
| TEAM5-COMP-01 | `CommandRegistry`, `QueryRegistry`, `EventBus` and `SyncAgent` are client-side composition/wiring responsibilities, not delivered Increment 2 primitives | All-Father/orchestrator ruling in `docs/DECISIONS.md`; date not stated | RECORDED | Historical architecture ruling; relevant to shared composition, not a direct owner approval of the Slint host boundary | P-01; [project decisions](../../DECISIONS.md) |
| TRACKB-02 | Switch `desktop-slint` to shared `desktop-session`, removing its duplicate session service/authority | Coordinator minor decision, 2026-11-07 | RECORDED | Source adoption and build/clippy evidence recorded; real OS keyring unverified | G0B-04; [decision log](decision-log.md) |
| TRACKB-03 | Slint lifecycle: sign-in/restored session starts one outbox task; sign-out stops it and cancels relay work; Slint has no event subscription path in this scope | Coordinator minor decision, 2026-11-07 | RECORDED | Three lifecycle tests recorded; device/live login and coexistence evidence remains | P01-KEYRING-01; P01-COEX-01; [decision log](decision-log.md) |
| Q-01 | Admin tokens are memory-only; no disk, OS keyring or browser persistence | Decision log labels approved; owner decision | APPROVED | Contract obligations/tests remain; Admin app is distinct from Desktop session behavior | G0B-04; [decision log](decision-log.md), [Contract Freeze v1](../../slint/CONTRACT_FREEZE_V1.md) |
| AUTH-DESKTOP-01 | Desktop session persists across restarts; Admin remains memory-only (Q-01) | Owner decision recorded in Contract Freeze v1, 2026-10-28 | APPROVED | Existing shared session implementation stores refresh token via keyring; access token in memory. Real OS-store device verification remains open | P-01/G0B-04; [Contract Freeze v1](../../slint/CONTRACT_FREEZE_V1.md) |
| AUTH-DESKTOP-02 | Store refresh token only in OS keyring; restore by refresh at startup; no plaintext-file fallback; logout clears memory/keyring and revokes where Worker supports it; keyring failure is non-fatal for current launch | Coordinator under standing owner authority, 2026-11-04 | RECORDED | Shared session code exists; physical OS-keyring behavior and Worker logout/revocation contract require verification | G0B-04; [token policy](desktop-token-storage-policy.md) |
| SECURE-STORE-01 | Use the unified Rust `keyring` crate instead of three hand-written per-OS adapters for Desktop secure storage | Orchestrator ruling in `docs/DECISIONS.md` S2; date not stated | RECORDED | Historical Linux unknown-key read/delete tests passed; write round-trip was ignored due to interactive unlock; not Windows/macOS proof and not explicit owner approval | G0B-04; [project decisions](../../DECISIONS.md) |
| O-1 | Argon2id using existing Worker library `argon2-wasm-edge` | Coordinator under owner’s “decide and document” instruction, 2026-11-01 | RECORDED | Implemented in current Worker source; account test suite contains hash-only assertions; this audit did not run the suite or verify deployment | Backend account lane; [decision log](decision-log.md), [backend current-state reconciliation](backend-accounts-current-state.md) |
| O-2 | Cloudflare D1 is authoritative account store; do not write Rust `UserStore` for this lane | Coordinator under delegated authority, 2026-11-01 | RECORDED | Worker uses D1; `migrations/0009_public_ids.sql` is present and source ID allocation is implemented. Migration header calls O-2 an “owner decision”, but the decision log attributes it to the coordinator; use the latter. Production D1 application is unverified | Backend account lane; [decision log](decision-log.md), [backend architecture](../../BACKEND_ARCHITECTURE.md), [ID design](../../design/id-sequences.md) |
| O-3 | Onyx ID is primary login identifier; username and email remain accepted aliases during transition | Coordinator under delegated authority, 2026-11-01 | RECORDED | Implemented in current Worker login source; backend test suite covers Onyx ID login and aliases; production behavior unverified | Backend account lane; [API contract](../../production/api-contract.md), [backend current-state reconciliation](backend-accounts-current-state.md) |
| O-4 | `must_change_password` flag; while set, only password change and logout allowed; other session routes return `PASSWORD_CHANGE_REQUIRED` (409) | Coordinator under delegated authority, 2026-11-01 | RECORDED | Implemented in current Worker `requireSession`/password-change source; backend tests cover forced-change and session revocation; deployment unverified | Backend account lane; [API contract](../../production/api-contract.md), [backend current-state reconciliation](backend-accounts-current-state.md) |
| O-5 | Generate a new Onyx ID for every import row; ignore a supplied Onyx ID and report it in row result | Coordinator under delegated authority, 2026-11-01 | RECORDED | Implemented in Worker import source; backend tests cover generated IDs, ignored supplied IDs, and no ID gap on failed rows | Backend account lane; [decision log](decision-log.md), [ID design](../../design/id-sequences.md) |
| O-6 | Extend existing `POST /api/admin/profiles/import` to accept XLSX and create accounts per row | Coordinator under delegated authority, 2026-11-01 | RECORDED | Implemented in current Worker source for XLSX/CSV/JSON; backend account test suite contains import tests; audit did not run them and deployment is unverified | Backend account lane; [API contract](../../production/api-contract.md), [backend current-state reconciliation](backend-accounts-current-state.md) |
| STAFF-IMPORT-01 | Every staff import row and single admin creation creates an account with generated Onyx ID, generated one-time password and forced change at first login | Owner decision 2026-11-01 | APPROVED | Implemented in Worker source; account test suite covers batch and single creation, generated IDs and forced change; not independently run in this audit | Backend account lane; [staff/team rules](../../product/staff-and-team-rules.md), [backend current-state reconciliation](backend-accounts-current-state.md) |
| STAFF-REPORT-01 | Every batch import and single creation returns an Excel (.xlsx) creation report containing staff-sheet fields, assigned Onyx ID, one-time password and account status | Owner decision 2026-11-01 | APPROVED | Implemented as one-sheet XLSX response; tests cover single/batch reports, failed rows, no-store, formula-injection protection and no password persistence/logging. Two implementation fields (`source_row`, `row_notes`) and exact ordering were not separately owner-approved | [report spec](backend-accounts-creation-report-spec.md), [staff/team rules](../../product/staff-and-team-rules.md) |
| STAFF-EMAIL-01 | Require account email; if `contact_email` is absent, generate `<onyx_id>@accounts.invalid` and flag it in the report | Coordinator under delegated authority, 2026-11-01 | RECORDED | Implemented in Worker source; backend tests assert placeholder address and report note | Backend account lane; [decision log](decision-log.md) |
| STAFF-PASSWORD-01 | Password minimums remain 8 characters on create and 12 on reset; generated one-time passwords meet 12 characters | Coordinator under delegated authority, 2026-11-01 | RECORDED | Implemented in Worker source and covered by backend account tests; no plaintext password storage/logging is asserted by tests | Backend account lane; [decision log](decision-log.md) |
| STAFF-REPORT-OPEN-01 | Post-download retention/deletion policy for plaintext-password reports remains undecided; exact column ordering and the two implementation-added columns have no separate owner approval | Owner input required; current Worker source implements the report behavior | PENDING | Status values and failed-row same-sheet behavior are implemented; Worker sends an authenticated XLSX attachment with `Cache-Control: no-store` and does not store it. Client-side retention/deletion policy remains unspecified | STAFF-REPORT-01; [report spec](backend-accounts-creation-report-spec.md) |
| BACKEND-LANE-01 | Staff account/import lane may run in parallel with Slint migration | Owner allowed parallel lane 2026-11-01 | APPROVED | Handover exists; source implementation status must be evaluated separately | [backend handover](backend-accounts-handover.md) |
| Q-05 | Preserve list/API membership contract (`Vec<String>`) and semicolon-delimited editor behavior | Decision log's confirmed-decision record | APPROVED | Validator and unit tests recorded; route wiring/round-trip verification remains | G0B-05; [decision log](decision-log.md), [staff/team rules](../../product/staff-and-team-rules.md) |
| Q-05-DELIM | Reject semicolons in team names because semicolon is the list separator | Coordinator under standing owner instruction; owner informed 2026-10-31 | RECORDED | Validator and unit tests recorded; route wiring remains | G0B-05; [decision log](decision-log.md) |
| Q-14 | Persist theme and language preferences across launches; owner approved schema direction | Owner approval recorded 2026-10-29 | APPROVED | Required schema artifact (fields/defaults/fallback) is still missing; not implemented/verified | G0B-06; [decision log](decision-log.md) |
| Q-16 / U-05 | Display Latin digits for IDs only; canonical values and API data unchanged | Decision log records approved decision | APPROVED | Admin display helper and 8 tests recorded; helper not yet used on all screens or shared with Desktop | G0B-07; [decision log](decision-log.md) |
| U-04 | Policy lifecycle Active → Retired (terminal); versions Draft → Published (immutable); at most one pending draft; evaluation/violation require Active + published; expected version/epoch checks and safe retry semantics | Decision log records approved model | APPROVED | Rust/Worker behavior evidence and tests partly exist; parity, missing-key, concurrency and error mapping remain | G0B-08; [decision log](decision-log.md), [u04-policy-parity](u04-policy-parity.md) |
| U-04-ERR | Use `DRAFT_ALREADY_PENDING`, `NO_DRAFT_VERSION` and `NO_PUBLISHED_VERSION` for the documented policy precondition failures; do not use the generic unsupported-precondition code for these cases | Coordinator decision recorded 2026-10-31 | RECORDED | Worker implementation status must be verified against current source/tests | G0B-08; [decision log](decision-log.md) |
| U-03 | Bundle Vazirmatn with SIL OFL 1.1 | Owner confirmed 2026-10-29 | APPROVED | Per-OS rendering and glyph/shaping evidence incomplete | G0B-09; [decision log](decision-log.md), [spike results](spike-results.md) |
| U-10 | If OS motion preference is unknown/unreadable, resolve to reduced motion | Coordinator decision; owner confirmed 2026-10-31 | APPROVED | Windows reader implemented; Windows live toggle/device confirmation and other OS readers/evidence remain | G0B-09; [decision log](decision-log.md) |
| U-10-LIVE | Decide whether the app reacts to OS motion-setting changes while running or reads the setting only at startup | Open question OQ-1 in spike results | PENDING | Current spike reads at startup only; no approved live-update behavior found | G0B-09; [spike results](spike-results.md) |
| U-10-WIN-ACCEPT | Decide whether Windows reduced-motion fallback is acceptable if the Windows reader/manual verification is incomplete at Gate 0b | Open question OQ-2 in spike results | PENDING | Windows reader source/CI evidence exists; real Windows setting toggle still needs manual verification | G0B-01; G0B-09; [spike results](spike-results.md) |
| U-01-RTL | Decide Persian placeholder alignment and mixed-direction line/punctuation behavior | Open question OQ-3 in spike results | PENDING | Current Linux screenshot shows a possible punctuation/alignment issue; cursor, selection and copy/paste untested | G0B-09; [spike results](spike-results.md) |
| U-02-FOCUS | Decide whether to keep Slint std-widgets focus style or implement the custom C-04 focus ring | Open question OQ-4 in spike results | PENDING | Focus visibility in light/dark themes has not been human-reviewed | G0B-09; [spike results](spike-results.md) |
| U-TEST-OWNER | Assign who performs pending manual checks and the test dates for Windows, Persian IME, NVDA/Orca and theme focus | Open question OQ-5 in spike results | PENDING | No tester/date assignment found in accessible records | G0B-09; [spike results](spike-results.md) |
| C-01 | Re-pin Worker API contract to source baseline; decision log records 42 route-completeness tests passing on 2026-11-01 baseline | Recorded 2026-11-01 | RECORDED | Contract is source-level, not deployment proof; later B-01 draft/dispatch reconciliation still requires review/freeze | G0B-03; [API contract](../../production/api-contract.md), [C-01 reconciliation](../../../reports/slint-migration/B-01/B-01_C01_Dispatch_Reconciliation.md) |
| C-03 | Typed error mapping approval recorded 2026-10-29; concrete committed mapping remains required | Owner approval recorded in decision log | APPROVED (mapping direction) | Proposal remains DRAFT / not frozen | G0B-10; [C-03 proposal](../../../reports/slint-migration/B-01/B-01_C03_Error_Mapping_Proposal.md) |
| C-09 | Fixture schema approval recorded 2026-10-29; exact schema identity/provenance must be committed and verified | Owner approval recorded in decision log | APPROVED (schema direction); artifact identity unresolved | Provenance/validator/fixtures and exact schema reference remain outstanding | G0B-11; [fixture provenance](../../production/fixture-provenance.md), [decision log](decision-log.md) |
| FILE-ROUTE-01 | User chooses Local or Internet before each file send; Local/Internet selection is per file, not once for the whole session | Owner decision dated 2026-11-07; per-file scope explicitly clarified by owner in audit instruction 2026-11-10 | APPROVED | Product spec amended to state per-file selection. Exact Local transport (LAN direct vs relay/P2P/BLE) remains unresolved | [file-transfer spec](../../product/file-transfer.md), [decision log](decision-log.md) |
| P2P-1 | Android P2P platform integration direction is Kotlin→JNI; existing Rust P2P exports are placeholders, not final transport implementations | Technical ruling dated 2026-09-23 in `docs/DECISIONS.md` | RECORDED | Kotlin owns Android Wi-Fi P2P/Bluetooth platform integration; Rust exports do not implement transport | FILE-LOCAL-01; [project decisions](../../DECISIONS.md) |
| P2P-2 | Relay ticket minting requires `submit_domain_command`; `mobile_observer` sessions are excluded | Technical ruling dated 2026-09-23 in `docs/DECISIONS.md` | RECORDED | Source evidence records capability check before ticket minting; this is sync-transport authorization, not proof of file-route selection | [project decisions](../../DECISIONS.md) |
| P2P-4 | `GET /api/files/:content_hash` requires `can_download_files`; push subscription routes use `can_read_notifications` | Technical ruling dated 2026-09-24 in `docs/DECISIONS.md` | RECORDED | API route authorization is recorded; content-hash download is not a complete FileAsset listing or Local/Internet transfer policy | [project decisions](../../DECISIONS.md) |
| P2P-6 | FileList was deferred to Phase 3.1 because no FileAsset listing/query exists; only content-addressed download was available | Technical ruling dated 2026-09-24 in `docs/DECISIONS.md` | RECORDED | FileList was not implemented by the recorded Phase 2 PWA scope; check current source before asserting later implementation | [project decisions](../../DECISIONS.md) |
| TEAM4-DISCOVERY-01 | When the same peer is found locally and via cloud discovery, cloud entry overwrites the local entry so resolvable endpoints are retained | Team 4 ruling T4-9 in `docs/DECISIONS.md`; date not stated | RECORDED | Historical `CompositeDiscovery` implementation decision; does not determine file-transfer Local route | [project decisions](../../DECISIONS.md) |
| TEAM4-QUIC-01 | QUIC NAT rebinding is handled by the QUIC connection ID; rebind the client Endpoint, not the Connection | Team 4 ruling T4-5/T4-6 in `docs/DECISIONS.md`; date not stated | RECORDED | Real ignored-by-default integration test was explicitly run and passed per the historical record; not a file-transfer acceptance test | [project decisions](../../DECISIONS.md) |
| TEAM4-RELAY-01 | Keep Tokio-backed relay socket/TLS implementation behind the `RelaySocket`/`RelaySocketFactory` seam at the composition root; sync-transport itself depends on the runtime-neutral seam | Team 4 rulings T4-11/T4-12 in `docs/DECISIONS.md`; date not stated | RECORDED | Unit test proves binary wire encoding, not a live TLS handshake or deployed relay path | P01-LIVE-01; [project decisions](../../DECISIONS.md) |
| WEB-UI-01 | Web UI work postponed; no resume or Vite upgrades #424/#429 without owner approval | Owner decision 2026-11-08 | APPROVED | Postponed; no current web file-transfer page | [web UI postponed](../../product/web-ui-postponed.md) |
| CI-WEB-01 | Remove `web` and `mobile-pwa` jobs from CI and frontend security matrix while Web UI is postponed; retain shared-catalogue i18n audit | Owner decision recorded 2026-11-09 | APPROVED | CI/security workflow change merged in PR #441; web/mobile source and Dependabot entries retained | [decision log](decision-log.md), [web UI postponed](../../product/web-ui-postponed.md) |
| DIAG-POST-01 | Defer diagnostic event collection until after the Slint migration; Cloudflare logging approach is only a candidate, not decided | Recorded 2026-10-31 | RECORDED | Not started; owner/implementation approach to be assigned after migration | [decision log](decision-log.md) |
| Q-05 name delimiter | Reject semicolon in team names; preserve semicolon list serialization | Coordinator under standing authority; owner informed, 2026-10-31 | RECORDED | Validator implemented with unit tests; route wiring remains | [decision log](decision-log.md) |
| U-04 error codes | Use `DRAFT_ALREADY_PENDING`, `NO_DRAFT_VERSION`, `NO_PUBLISHED_VERSION`; do not use generic unsupported precondition code for these cases | Coordinator decision recorded 2026-10-31 | RECORDED | Worker implementation status must be verified | [decision log](decision-log.md) |
| U-10 fallback | Reduced motion is fallback when OS preference unavailable | Coordinator decision; owner confirmed 2026-10-31 | APPROVED | Platform readers/wiring still need evidence | [decision log](decision-log.md) |
| C-01 baseline | `docs/production/api-contract.md` re-pinned to `d330451` on 2026-11-01; 42 route-completeness tests recorded passing | Decision log | RECORDED | Does not establish deployed Worker revision; later source revision must be reconciled | [API contract](../../production/api-contract.md) |
| C-09 provenance | Fixture inventory recorded in `docs/production/fixture-provenance.md`; exact approved schema identity remains to be linked | Decision log | PENDING artifact/evidence | Gate evidence remains incomplete | [fixture provenance](../../production/fixture-provenance.md) |

### Important approval interpretation

The O-1–O-6 entries are **not six owner questions**: the decision log records them as coordinator decisions made under the owner's standing “decide and document” instruction. They are therefore retained as **RECORDED**, not relabelled as direct owner approvals. The owner may override them through a new, traceable decision. The earlier backend handover/current-state labels saying “not decided” are stale and receive dated amendments.

## 4. Technical architecture and supporting specifications

### Desktop and Admin clients

- Desktop direction: separate native Slint host under P-01/B1, reusing approved Rust crates in-process. Tauri remains the production default until release/gate authorization.
- Admin Slint scaffold exists under `crates/bins/admin-slint/`; the B-01 client production modules remain blocked by Gate 0b.
- Desktop and Admin are separate applications with different session persistence policies.
- Shared session authority is in `crates/applications/desktop-session/`; keyring adapter implementation is source evidence, not proof of device-level OS-store behavior.
- Desktop host must not depend on the Tauri shell as its runtime boundary. Track B dependency proof is recorded as Tauri-free for normal dependencies, but lifecycle/coexistence/device checks remain.

### Backend and identity

- Cloudflare Worker (`deploy/cloudflare/edge-gateway`) is the supported remote API for Admin, Staff, web and mobile clients; D1 is authoritative for Worker-managed accounts, organizations, sessions/revocations and authorization. Durable Objects back event/relay hubs; optional Hugging Face S3-compatible bindings are used where configured. The Worker does not proxy to Axum.
- Native Axum API/background worker is a separate runtime with SQLx-backed PostgreSQL/SQLite adapters. Worker does not proxy to Axum. Do not infer shared database, transaction, replication or authorization guarantees.
- See [Backend Architecture](../../BACKEND_ARCHITECTURE.md), [Authentication Model](../../SECURITY_AUTHENTICATION_MODEL.md), and [Worker API Contract](../../production/api-contract.md).

The project-wide `docs/DECISIONS.md` also records an S2 ruling to use the unified Rust `keyring` crate instead of three hand-written per-OS adapters. The inspected text attributes this to the orchestrator, not an explicit owner approval; classify it as RECORDED implementation precedent, not an owner-approved product policy. Its historical Linux Secret Service tests passed unknown-key read/delete, but the write/read/delete round-trip was ignored because the sandbox could not unlock the keyring interactively. This does not close current OS-keyring device verification.

### Authentication, authorization and session management

- Worker issues access/refresh token pairs; protected requests re-check current principal, organization, role, active status and revocation state from D1.
- Admin: access/refresh tokens are memory-only (Q-01); closing the Admin process discards them, so the user signs in again. No disk, OS keyring or browser persistence.
- Desktop: refresh token in OS keyring; access token memory-only; startup restore exchanges refresh token; no plaintext-file fallback. On supported platforms the keyring maps to Windows Credential Manager, macOS Keychain and Linux Secret Service. Desktop session persists across application/OS restarts. This detailed refresh-only rule is a delegated coordinator decision under standing authority.
- Logout revocation support and exact live Worker behavior must be verified before claiming end-to-end completion.
- Tokens/passwords must not appear in logs, diagnostics, crash reports, Slint properties, event payloads or persisted non-keyring stores.

### File transfer, relay and P2P

- Internet file transfer is in scope, with a 100 MB per-attempt limit.
- LAN transfer is in scope in the product specification; current product text describes direct same-LAN transfer and server-side peer registry discovery.
- User route selection is per file: Local or Internet.
- The decision log describes Local as BLE + LAN using a relay/P2P path. The product specification describes LAN as routine direct transfer and BLE/Wi-Fi Direct as a separate deferred emergency module. These statements do not establish one consistent Local transport contract.
- `docs/DECISIONS.md` records adjacent sync/P2P infrastructure: `CloudRelayTransport`, `QuicCrossNetworkTransport` (QUIC NAT-rebinding test recorded), and `WifiDirectTransport`/`BluetoothLETransport` platform stubs; Android P2P direction is Kotlin→JNI and Rust P2P exports are placeholders. It also records relay-ticket capability gating and a content-hash download route. These are sync/API architecture records, **not** proof that the Local file-transfer route is implemented or that the sync relay is the file-transfer relay. The relay socket/TLS implementation is intentionally supplied behind a runtime-neutral `RelaySocketFactory` seam at the composition root; the recorded unit test checks binary encoding, not a real TLS handshake.
- **Unresolved:** whether Local uses relay, direct LAN, BLE, or multiple transports; what “P2P connections, etc.” includes; and the precise routing/fallback behavior. Do not implement or reinterpret this from assumptions.
- Web UI is postponed and, when resumed with approval, the recorded scope is Internet-only; LAN/emergency P2P placeholders are disabled.

### Localization, accessibility and cross-platform requirements

- Canonical localization source: `shared/i18n/`; English is canonical, Persian is RTL. Generator/schema/key parity/placeholder validation is CI-enforced. The localization README points to missing `docs/i18n/PHASE-0.5-DECISIONS.md`; its contents/authority cannot be verified until restored or replaced.
- `fa.json` exists and was regenerated with new keys, but Persian-speaker review of new strings is not recorded. CI validity is not linguistic approval.
- Required Slint evidence includes Windows/Linux sorting/paging, multiline editor, Persian IME composition and mixed RTL/LTR cursor/selection; Windows UI Automation and Linux AT-SPI/screen reader; font source/license/glyph/shaping/render evidence; reduced-motion behavior; dark-theme contrast.
- Windows and Linux spike CI results exist for limited checks. Manual and device evidence must be separately recorded for each platform. No blanket “Windows/macOS/Linux verified” claim is supported.

### Deployment and release boundaries

- No production Worker deployment, D1 mutation or cutover is authorized by this record. The repository contains a deployment-capable `.github/workflows/deploy-cloudflare-edge.yml`; it must not be dispatched without explicit owner authorization.
- `wrangler.dev.toml` is fixed to local-only D1. The production `wrangler.toml` and owner-controlled `provision:allfather-password` procedure are documented in the Worker README; neither was executed or inspected against production in this audit.
- Production D1 migration state, deployed Worker revision, All-Father provisioning and live smoke remain unverified. Worker source contract is not proof of deployed Worker revision.
- B-01 implementation cannot begin until signed Gate 0b/Contract Freeze v1 and all required approvals/evidence are present.
- Release/cutover must preserve existing Tauri production default until an explicit release approval authorizes replacement.

## 5. Unified blocker register

Statuses describe the evidence available at the audited baseline. **OPEN** means exit evidence is missing; **IN PROGRESS** means work exists but acceptance criteria are not met; **PENDING OWNER** means a decision or formal approval is required. No blocker is closed merely because a proposal or source implementation exists.

| ID | Type | Blocker / why it blocks | Owner / responsible party | Dependencies | Required resolution and closure evidence | Status / last reviewed |
|---|---|---|---|---|---|---|
| G0B-01 | Owner approval / governance | Formal Gate 0b and signed Contract Freeze v1 prohibit production client work until satisfied | Project owner / integration coordinator | All gate evidence and contract approvals | Signed record with exact baseline, accepted evidence, frozen contract versions, approved fallbacks, approver/date and scoped authorization | OPEN / 2026-11-10 |
| G0B-02 | Documentation / architecture | Confirm crate-relative production client path and file ownership | Repository owner / integration coordinator | Governing BOUND plan and B-01 ownership packet | Explicit path/ownership recorded in canonical log and accepted by gate approver | OPEN / 2026-11-10 |
| G0B-03 | Technical contract | C-01 route/handler contract must match selected Worker source revision | B-01 + Worker/API contract owner | Exact source baseline, handler/DTO/error review | Source-pinned route-by-route reconciliation and reviewed contract; deployed revision limitation stated | IN PROGRESS / 2026-11-10 |
| G0B-04 | Security / authentication | Q-01 token lifecycle, Desktop keyring restore, Admin memory-only contract, refresh/logout/revocation and tests | Product/security owner + B-02 | C-07/C-08 contracts, Worker behavior, device keyring tests | Approved contracts; tests for refresh/expiry/logout/revocation/redaction; real keyring tests on Windows/macOS/Linux; two-app separation evidence | IN PROGRESS / 2026-11-10 |
| G0B-05 | Technical contract / testing | Profile membership wire representation and validation/round-trip behavior | Product owner + B-08, B-01 consulted | Existing `Vec<String>` and semicolon contract | Read/write fixtures, server validation and route-level evidence | IN PROGRESS / 2026-11-10 |
| G0B-06 | Owner decision / documentation | Q-14 preference persistence schema is approved in direction but schema fields/defaults/fallback are absent | Product owner + B-02/B-04 | Schema and storage decision artifact | Commit exact schema, defaults, storage location, versioning, corruption fallback and tests | OPEN / 2026-11-10 |
| G0B-07 | Testing / localization | Q-16/U-05 Latin-digit display-only behavior is not used consistently across screens/clients | Product owner + B-04/Desktop owner | Shared formatting and RTL/LTR tests | Use helper on required surfaces; mixed RTL/LTR tests prove canonical values unchanged | IN PROGRESS / 2026-11-10 |
| G0B-08 | Technical contract / testing | U-04 Worker/Rust parity, published-version preconditions, missing-key semantics and concurrency/error behavior | Product/policy owner + B-09 | Exact Worker/Rust handlers and version/epoch contracts | Parity matrix/tests and reviewed state diagram/error mapping | IN PROGRESS / 2026-11-10 |
| G0B-09 | Testing / accessibility | Slint feasibility U-01/U-02/U-03/U-10/U-12 lacks complete per-platform/manual evidence | B-13 + B-03/B-04/B-12 + designated human testers | Windows/macOS/Linux test devices, IME, assistive tech, font and contrast reports | Resolve pending OQ-1–OQ-5 (motion live-update/fallback, RTL punctuation/alignment, focus-ring choice and tester/date ownership); provide reproducible per-OS reports with versions, commands/manual steps, artifacts and accepted pass/fallback/block disposition | OPEN / 2026-11-10 |
| G0B-10 | Localization / technical contract | C-03 typed error taxonomy is a proposal, not a frozen approved map | B-01 + B-04 + B-02 | Worker error envelope and localization contract | Versioned mapping and tests; no raw body/secret exposure; approval recorded | OPEN / 2026-11-10 |
| G0B-11 | Testing / evidence | C-09 fixture schema/version/provenance must be identified and validated | B-13 + B-01 | Owner-approved schema direction; fixture provenance | Exact schema path/version, provenance, validator, valid/invalid fixtures, route/error coverage and approval | OPEN / 2026-11-10 |
| G0B-12 | Technical contract | Organization-admin create DTO mismatch (Worker response vs UI expected timestamps/role fields) | B-01 + B-10 + Worker/API owner | Source-level response and UI model review | Canonical DTO decision plus regression test against verified response shape | OPEN / 2026-11-10 |
| G0B-13 | Security / CI | Recorded audit run 40426925219 passed advisories, bans and sources but failed the license-policy check; policy disposition required | Security/CI maintainer + repository owner | License metadata and policy review | Security/CI owner reviews rejected license metadata (including Slint LicenseRef values) and records a policy disposition without unapproved allowlist weakening/suppression, then a fresh green required-check run on intended head | OPEN / 2026-11-10 |
| G0B-14 | CI / release evidence | B-01 docs PR CI run 40426925219 failed because security/license job failed | CI maintainers / PR owner | G0B-13 and current branch/head | All required checks green on current intended head; old run is historical, not current status | OPEN / 2026-11-10 |
| DEP-01 | Testing / environment | Earlier local Rust environment lacked pinned Rust 1.97.1; stable-toolchain results are not equivalent | B-13/CI owner | Pinned CI toolchain | Approved Rust 1.97.1 CI results for the relevant implementation commit | OPEN / 2026-11-10 |
| DEP-02 | Documentation / review | B-01 Wave 0 packet exists and PR #329 is merged (merge commit `09c7ba0e`); however its recorded follow-up run 40428710390 had security audit jobs cancelled and `merge-gate` failed | B-01 / integration coordinator | G0B-13/G0B-14 | Preserve the merged packet; verify current required checks and complete any missing handover review without treating the old merge as Gate 0b approval | IN PROGRESS / 2026-11-10 |
| P01-LIVE-01 | Security / integration test | Live relay-ticket success/rejection checks not run against deployed Worker | Desktop/Worker owners | Deployed Worker and authorized test account | Recorded live positive and negative relay-ticket checks, with exact deployed revision and redacted evidence | OPEN / 2026-11-10 |
| P01-KEYRING-01 | Platform testing | Real OS keyring round trip is not verified on Windows/macOS/Linux; historical Linux Secret Service evidence only covers unknown-key read/delete, while the write round-trip was ignored | Desktop owner / platform testers | Unlocked OS credential store and test devices | Successful device round-trip evidence per OS (store, retrieve, delete) with app/session isolation; mock-store tests and historical read/delete-only checks do not substitute | OPEN / 2026-11-10 |
| P01-COEX-01 | Platform testing | Desktop Tauri and Slint hosts need installed coexistence/session-isolation verification | Desktop owner / platform testers | Installable builds and test devices | Install both, authenticate independently, prove keyring service/data-directory separation and uninstall/upgrade behavior | OPEN / 2026-11-10 |
| I18N-FA-01 | Localization review | New Persian strings lack recorded human Persian-speaker review | Localization owner / Persian reviewer | Exact catalog revision and review scope | Review record identifying reviewer, catalog commit, corrections and approval | OPEN / 2026-11-10 |
| FILE-LOCAL-01 | Product decision / technical contract | Local route differs between decision log (relay/P2P/BLE+LAN) and product spec (direct LAN; emergency BLE/Wi-Fi Direct deferred) | Product owner + file-transfer technical owner | Existing owner instruction and detailed transport evidence | Reconciled written Local route contract; distinguish LAN, relay and emergency P2P; record fallback and per-file routing behavior | PENDING OWNER / 2026-11-10 |
| DOC-BOUND-01 | Documentation / governance | The decision log cites `common/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md`, but that path returned 404; the governing plan's full text could not be checked in-repository | Project owner / documentation coordinator | External or historical BOUND plan source | Restore the authoritative plan in the intended repository path or record its stable external location and version; verify Gate 0b requirements against it | OPEN / 2026-11-10 |
| DEPLOY-01 | Deployment / security | Production D1 migration state, deployed Worker revision and owner-controlled All-Father provisioning/live smoke are not verified | Project owner + Cloudflare deployment operator | Gate 0b, explicit production authorization, Cloudflare credentials and secret-safe smoke | Only after owner authorization, verify remote D1 migrations and deployed revision; run approved provisioning and live smoke with secrets kept out of logs; record exact evidence | OPEN / 2026-11-10 |
| I18N-DOC-01 | Documentation / localization governance | `shared/i18n/README.md` declares `docs/i18n/PHASE-0.5-DECISIONS.md` authoritative, but that path returned 404 | Localization/documentation owner | Current i18n README and existing decision records | Locate the approved Phase 0.5 decision record in history or replace the dangling reference with an owner-approved existing authority; do not infer its contents | OPEN / 2026-11-10 |
| STAFF-REPORT-BLK-01 | Owner decision / security / product | Post-download retention/deletion policy for a plaintext-password creation report is unspecified; exact column order/additional implementation fields lack separate owner approval | Product owner + backend owner | Implemented XLSX response and source-level no-store/no-persistence behavior | Decide whether downloaded report files need a defined retention/deletion window and whether the 17-column layout (including `source_row`/`row_notes`) is accepted; update spec/tests if changed | PENDING OWNER / 2026-11-10 |

### Blocker closure rule

A blocker is closed only when its own closure evidence is available, reviewed by the responsible party, linked in the register, and accepted by the relevant approver. A decision being recorded does not close an implementation/test blocker. A passing unit test does not close a physical-device blocker. A successful build does not prove deployment. A proposal does not close an approval blocker.

## 6. Implementation and verification status

| Requirement / area | Required behavior | Current status supported by evidence | Source / PR | Remaining acceptance criteria |
|---|---|---|---|---|
| B1 Desktop architecture | Separate native Slint host, shared Rust crates, shell-independent session authority | Conditionally approved; Track B work present | [P-01 investigation](P-01-B1-investigation-2026-10-27.md), decision log | Complete lifecycle/coexistence/platform evidence and gate acceptance |
| Track A relay auth | Replace test-only authority with session-aware fail-closed provider | Implemented and merged PR #407; unit tests recorded | [decision log](decision-log.md), PR #407 | Live Worker relay-ticket positive/negative checks |
| Track B shared session | One shared session owner in `desktop-session` | Implemented in source; tests recorded | `crates/applications/desktop-session/` | Real keyring device checks and two-host coexistence |
| Track B dependency proof | Slint host free of Tauri-family runtime dependencies | Normal dependency tree recorded with zero Tauri-family crates | Decision log | Preserve exact command/commit evidence and repeat at relevant head |
| Slint lifecycle | Start/stop outbox and preserve replica identity on restart | Three lifecycle tests recorded passing; clippy/fmt clean in recorded run | Decision log | Successful online login/refresh and device coexistence |
| Admin Slint B-01 client | Production client modules per approved contract | NOT IMPLEMENTED; Gate 0b prohibits starting | B-01 handover/final report | Signed Gate 0b, frozen C-01/C-03/C-09 and file ownership |
| Desktop/Admin token policy | Desktop refresh-only keyring; Admin memory-only | Policy recorded; shared implementation exists | Token policy and session source | Real OS keyring on all target OSes, logout/revocation and separation tests |
| Backend accounts O-1–O-6 | Argon2id, D1, Onyx ID aliases, forced change, generated IDs, XLSX account import | Decisions RECORDED; all six behaviors implemented in current Worker source and a dedicated test suite exists; test execution and production deployment are not independently verified here | Worker source, `test/backend-accounts.test.ts`, API contract, decision log | Verify CI on the current Worker revision and production D1/Worker deployment separately; do not infer deployment from source |
| Staff creation report | Each import/single create returns .xlsx with staff fields, generated ID, one-time password and account status | Implemented in source as one-sheet 17-column attachment; test coverage exists for report generation, failure rows, no-store and secret handling; current tests not rerun by this audit | Worker source, `src/xlsx.ts`, `test/backend-accounts.test.ts`, report spec | Only post-download retention/deletion and separate approval of exact column order/additional fields remain open |
| Q-05 team validation | Preserve list contract; reject semicolon in names | Validator and unit tests recorded; route wiring not established | Decision log | Route integration and round-trip tests |
| Q-14 preferences | Persist theme/language | Owner-approved direction; schema absent | Decision log | Schema, fallback, implementation and tests |
| Q-16/U-05 display digits | Latin digits for display only | Admin helper + 8 tests recorded | Decision log | Use on all intended screens and Desktop; RTL tests |
| U-04 policy lifecycle | Active/Retired and Draft/Published rules | Rust/Worker evidence and tests partly recorded | Decision log, parity docs | Full parity/concurrency/error tests |
| U-03 font | Vazirmatn OFL 1.1 | Owner confirmed font/license; per-OS render unverified | Decision log, spike results | Per-platform shaping/glyph evidence |
| U-10 motion | Reduced motion when OS preference unknown | Windows reader implemented; device toggle pending | Decision log | Live Windows check and other platform readers/evidence |
| Persian localization | Valid EN/FA catalog parity and Persian copy | Catalog and generator exist; CI structural validation; human review absent | `shared/i18n/fa.json`, shared i18n README | Persian-speaker review and RTL/IME tests |
| File transfer | Internet 100 MB, LAN, per-file Local/Internet selection; emergency P2P deferred | Product requirements recorded; UI placeholders exist on Desktop; implementation not complete; related sync/P2P infrastructure is not equivalent to file-transfer implementation | [file-transfer spec](../../product/file-transfer.md), [project decisions](../../DECISIONS.md) | Resolve Local transport contract; implement only after authorized gate |
| Web UI | Postponed; Internet-only file route if resumed | Owner decision recorded; no Files page | [web UI postponed](../../product/web-ui-postponed.md) | New owner approval before resuming |

### Verification vocabulary

For every requirement, report separately:
- **Implemented:** source code contains the behavior.
- **Tested:** a named test ran and passed against a stated commit/environment.
- **Reviewed:** a responsible reviewer examined the evidence.
- **Merged:** the change is in the target branch.
- **Deployed:** the intended revision is confirmed running in the target environment.
- **Device-verified:** the behavior was tested on the named physical OS/device configuration.

Do not collapse these into “done”.

## 7. Milestones and gates

### Completed or partly completed

- **PR #442:** merged; decision-log reconciliation changes.
- **PR #443:** merged; original master record.
- **Track A:** implemented and merged PR #407; live relay checks pending.
- **Track B steps 1–5:** session authority extraction, Slint shared-session adoption, lifecycle work, keyring adapter hermetic tests and dependency proof are recorded as partly/completed in source/test evidence. Device-level checks remain.
- **Q-16/U-05 helper:** Admin display helper and tests recorded; adoption across all screens remains.
- **File-route placeholders:** Desktop UI shows Internet enabled and LAN/emergency options disabled; this is not transfer implementation.
- **Localization generation:** new EN/FA keys added and generated; Persian human review remains.

### Current milestone

**Wave 0 / Gate 0b closure.** B-01 production client implementation is not authorized until the gate is formally signed.

### Remaining milestones

1. Close or formally disposition all G0B and dependency blockers with accepted evidence.
2. Freeze C-01, C-03 and C-09 with exact versions and approvals.
3. Complete platform, accessibility, localization, security and live integration evidence.
4. Sign Contract Freeze v1 with exact baseline and scoped authorization.
5. Only after Gate 0b: implement the explicitly approved B-01 production client scope in reviewable PRs.
6. Complete end-to-end, platform, security and release verification.
7. Obtain separate owner authorization for production cutover/release.

### Gate 0b non-bypass conditions

Gate 0b remains blocked until all mandatory evidence, decisions, approvals and accepted fallbacks/dispositions are recorded and the owner signs the formal Contract Freeze v1 record. The sign-off must identify exact baseline, accepted evidence, frozen contract versions, approved fallbacks, approver/date and implementation scope. Neither PR #443 nor this master record is a gate sign-off.

## 8. Historical decisions, changes and supersession

Preserve the original dated entries in the decision log and evidence reports. Important history:
- 2026-10-28: owner decision recorded for Desktop session persistence across restarts and Admin memory-only policy in Contract Freeze v1.
- 2026-10-29: “Approved for full migration” owner approval recorded; this is not Gate 0b sign-off because required evidence, contract freeze fields and scoped approval remained incomplete.
- 2026-10-29: owner approvals recorded for P-01 boundary, Q-14 schema direction, C-09 schema direction and C-03 mapping direction; artifacts/evidence remained outstanding.
- 2026-10-29: Vazirmatn/OFL 1.1 confirmed; coordinator named to coordinate evidence, owner remains gate approver.
- 2026-10-31: coordinator decisions and owner-confirmed reduced-motion fallback; C-01 re-pin and Q-05 delimiter handling recorded.
- 2026-11-01: backend O-1–O-6 recorded under delegated “decide and document” authority. Earlier postponed-lane entry is superseded by the later parallel-lane authorization.
- 2026-11-04: owner conditionally approved B1; Track A first, Track B only after Track A passes. Detailed refresh-token-only policy separately recorded by coordinator under standing authority.
- 2026-11-07: owner file-route decision recorded; current owner clarification establishes Local/Internet choice before each file. Local transport semantics remain unresolved.
- 2026-11-07 to 2026-11-08: Track A/Track B implementation and test evidence updated; P-01 evidence summary dated 2026-11-01 is stale.
- 2026-11-08: owner postponed Web UI work and removed web/mobile-PWA CI checks; standalone i18n check remains.
- 2026-11-09: decision-log open-item reconciliation merged by PR #442.
- 2026-11-10: original master record merged by PR #443; this audit update continues that record.

Never delete the old decision or overwrite the original source evidence. Add dated corrections and link superseding decisions.

## 9. Document disposition and navigation

See [DOCUMENT-INVENTORY.md](DOCUMENT-INVENTORY.md) for the path-by-path inventory, classification, conflicts and disposition.

No files are deleted, renamed or retired by this audit. Historical reports remain in place. The master links to detailed supporting material rather than copying full specifications.

## 10. Audit coverage, limitations and validation

### Verified in this audit

- Inspected existing master record and the existing migration decision log.
- Inspected Desktop token policy, backend current-state and handover, file-transfer specification, P-01 evidence summary, B-01 blocker register, B-01 final report and B-01 README.
- Inspected Contract Freeze v1, Worker API contract, staff/team rules, backend architecture, authentication model, ID sequence design, web UI postponement, shared localization README and Persian catalogue.
- Inspected root README and repository structure document.
- Checked root `AGENTS.md` and `CONTRIBUTING.md`; both returned not found.
- Inspected PR #443: merged; only changed file was this master record; no PR conversation comments, submitted reviews or inline review threads were returned.
- Inspected recent commit history including merged PR #442 and #443 and relevant migration commits.
- Confirmed current branch baseline `main` at `c447433a520a83ea9f3fb39e9d70f8740af8a98b`.

### Not verified / limitations

- No complete recursive repository tree listing was exposed in this session; therefore the inventory is scoped and transparent, not exhaustive.
- All branches and all historical PR discussions/commits were not exhaustively inspected.
- External BOUND plan files referenced by the decision log were not present at the checked paths in the repository. The localization README's referenced Phase 0.5 decision file was also not present at the checked path.
- No physical device was available; no new Windows/macOS/Linux manual tests were run.
- No live Worker or production D1 was accessed or modified.
- No production deployment, release, cutover or application-code change was performed.
- Current GitHub CI results for the audit branch will be reported in the PR; this document does not claim them before they run.

### Validation checklist for this documentation PR

- [x] Every internal link resolves on the branch.
- [x] Decision IDs and blocker IDs are unique.
- [x] Every status has an evidence source or is explicitly unresolved.
- [x] No delegated coordinator decision is represented as direct owner approval.
- [x] No proposal is upgraded to approved.
- [x] No owner-approved decision is downgraded.
- [x] Stale backend/P-01/B-01 statements receive dated amendments.
- [x] Gate 0b remains blocked.
- [x] No application source, schema, CI policy or deployment file is changed.
- Required PR checks must be read from the latest PR head; no CI success is claimed by this audit record before those checks complete.
- [ ] Required PR checks are reviewed before merge.

---

**Authority reminder:** This master record consolidates and points to evidence. It does not create approval, implementation, verification, merge, deployment or cutover authority that the underlying records do not provide.
