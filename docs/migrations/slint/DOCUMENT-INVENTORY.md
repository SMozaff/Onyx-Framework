# ONYX Slint migration — documentation inventory and disposition

**Audit date:** 2026-11-10  
**Repository:** [SMozaff/Onyx-Framework](https://github.com/SMozaff/Onyx-Framework)  
**Inspected baseline:** `main` at `c447433a520a83ea9f3fb39e9d70f8740af8a98b`  
**Purpose:** Inventory of the migration's governing records, supporting specifications, implementation evidence and cross-cutting documents found during the audit.

## Scope and completeness statement

This is a repository-search-based inventory, not a claim that every file under every `docs/` subdirectory or every historical branch has been enumerated. The GitHub integration exposed file search and individual-file retrieval, but not a complete recursive repository-tree listing in this session. Searches covered migration/Slint, architecture, authentication, backend, file transfer, security, localization, testing, platform verification, deployment, product, README and governance terms. Known source paths cited by those documents were also checked where relevant.

The following could not be exhaustively inspected:
- Every file in every `docs/` subdirectory and every non-document source directory.
- All branches and all historical commits; recent commit history and directly relevant PRs were inspected.
- External planning artifacts referenced by the repository but absent from the checked-in tree.
- External owner conversations other than the current explicit clarification about per-file route selection.
- Physical-device test records not present in accessible repository evidence.

Do not describe this inventory as exhaustive beyond the inspected paths and searches.

## Classification and disposition meanings

- **MASTER:** central migration record and navigation authority after this audit is reviewed.
- **AUTHORITATIVE:** approved source for a specific domain policy or decision, subject to its stated scope.
- **SUPPORTING:** detailed technical specification, handover or implementation guidance.
- **EVIDENCE:** test results, source audits, approvals or implementation records.
- **HISTORICAL:** retained snapshot or superseded record preserved for traceability.
- **STALE:** contains statements requiring a dated correction; do not rely on stale status text.
- **UNRESOLVED:** authority, evidence or contract remains insufficient.
- **DUPLICATE:** repeated material that should be linked rather than maintained twice.

Disposition verbs:
- **KEEP:** retain as domain authority or useful evidence.
- **UPDATE:** retain and add a dated correction.
- **LINK:** retain as supporting material and link from the master.
- **ARCHIVE:** preserve as a dated historical snapshot.
- **MERGE:** move verified summary facts into the existing master, keeping detailed source intact.
- **RETIRE:** only after replacement and historical preservation are established. No retirement or deletion is authorized in this audit.

## Inventory

| Path | Subject / purpose | Status and decision authority | Classification | Duplicate / conflict notes | Relationship and disposition |
|---|---|---|---|---|---|
| `docs/migrations/slint/MASTER-MIGRATION-RECORD.md` | Central Slint migration status, decisions, blockers and navigation | Existing draft merged by PR #443; this audit branch updates it | MASTER | Earlier draft explicitly said first-pass inventory only | **UPDATE / KEEP**; central migration record |
| `docs/migrations/slint/decision-log.md` | BOUND decision history, owner instructions, delegated decisions, gate records | Primary migration decision history; includes owner-approved and coordinator-delegated entries | AUTHORITATIVE / EVIDENCE | Some older status summaries are stale; preserve historical entries | **UPDATE / KEEP**; append-only decision source |
| `docs/slint/CONTRACT_FREEZE_V1.md` | Contract freeze snapshot and outstanding freeze obligations | Contract reference; Gate 0b is not signed/passed | SUPPORTING / UNRESOLVED | Must not be mistaken for a signed final freeze | **UPDATE / KEEP / LINK** |
| `docs/migrations/slint/desktop-token-storage-policy.md` | Detailed Desktop refresh-token keyring and Admin memory-only policy | Detailed Desktop constraints recorded by coordinator under standing authority; Admin Q-01 separately owner-approved | AUTHORITATIVE for detailed policy / SUPPORTING | P-01 evidence summary still asks for the already-settled distinction | **UPDATE / KEEP / LINK**; preserve delegated attribution |
| `docs/migrations/slint/P-01-evidence-summary-2026-11-01.md` | Historical P-01 architecture evidence snapshot | Historical snapshot; Track A and token-policy prompts are stale | EVIDENCE / HISTORICAL / STALE | Contradicts later decision-log entries and merged implementation | **UPDATE** with dated amendment; retain original snapshot |
| `docs/migrations/slint/P-01-B1-investigation-2026-10-27.md` | B1 architecture investigation | Investigation evidence; later B1 conditional approval and implementation updates exist | SUPPORTING / EVIDENCE / HISTORICAL | Initial investigation findings are not current implementation status | **KEEP / LINK** |
| `docs/migrations/slint/P-01-B1-auth-provider-contract-2026-10-27.md` | Relay auth/session ownership and provider contract investigation | Evidence-backed proposal; implementation and later shared-session changes must be considered | SUPPORTING / EVIDENCE | Detailed implementation requirements overlap with token policy and decision log | **KEEP / LINK** |
| `docs/migrations/slint/spike-handover.md` | U-01/U-02/U-03/U-10/U-12 spike instructions and manual-test rules | Supporting test instructions; requires per-platform human evidence | SUPPORTING | Must not be replaced by a passing Linux CI result | **UPDATE / KEEP / LINK** |
| `docs/migrations/slint/spike-agent-prompt.md` | Agent instructions for feasibility spikes | Supporting execution prompt | SUPPORTING / DUPLICATE (partly) | Repeats some handover instructions | **UPDATE / KEEP / LINK** while active; handover is the primary procedural source |
| `docs/migrations/slint/spike-results.md` | Feasibility test results and unresolved owner questions | Evidence; Linux-container limitations and pending manual tests recorded | EVIDENCE / UNRESOLVED | Does not prove Windows/macOS/device behavior | **UPDATE / KEEP / LINK** |
| `docs/migrations/slint/backend-accounts-current-state.md` | Read-only Worker/D1 account and import baseline | Historical baseline from 2026-11-01; O-1–O-6 “not decided” wording is stale | EVIDENCE / HISTORICAL / STALE | Contradicted by later decision-log entry and product rules | **UPDATE** with dated correction; preserve observed baseline |
| `docs/migrations/slint/backend-accounts-handover.md` | Backend account lane scope, requirements and execution rules | Lane authorized to run in parallel; O-1–O-6 list is stale | SUPPORTING / STALE | Open-decision section conflicts with later delegated decisions | **UPDATE**; keep scope and safety constraints |
| `docs/migrations/slint/backend-accounts-creation-report-spec.md` | XLSX creation-report column and safety draft | Owner-approved .xlsx/report-content requirements; detailed order, status values, failed-row layout and delivery/retention remain unresolved | SUPPORTING / UNRESOLVED / STALE (old O-1/O-4 blocker labels) | Dated amendment reconciles O-1/O-4 and owner report decisions; do not ask those again | **UPDATE / KEEP / LINK** |
| `docs/product/file-transfer.md` | Product behavior for Internet, LAN and deferred emergency P2P | Owner decisions dated 2026-11-07; full product transfer behavior is not complete | AUTHORITATIVE product scope | Existing content-hash download route and sync/P2P infrastructure do not prove full Local/Internet file-transfer behavior; Local relay/P2P semantics conflict across docs | **UPDATE** per-file route selection clarification; preserve unresolved Local transport semantics |
| `docs/product/staff-and-team-rules.md` | Owner-confirmed staff import, IDs, roles, account creation | Owner-confirmed 2026-10-30 and 2026-11-01 requirements | AUTHORITATIVE product rules | Supports O-1–O-6 but does not approve each delegated technical mechanism | **KEEP / LINK** |
| `docs/product/web-ui-postponed.md` | Owner decision to postpone web UI and resume conditions | Owner decision dated 2026-11-08 | AUTHORITATIVE scope decision | Relevant to migration scope and file-transfer parity | **KEEP / LINK** |
| `docs/design/id-sequences.md` | Public ID generation and D1 allocation design | Design and source implementation recorded 2026-11-01; O-2 attribution needs correction because migration header says owner decision while decision log says coordinator-delegated | SUPPORTING / EVIDENCE / STALE (attribution) | Migration file exists; no production D1 application evidence | **UPDATE / KEEP / LINK** |
| `reports/slint-migration/B-01/README.md` | B-01 Wave 0 evidence packet index | Gate 0b blocked; C-01/C-03/C-09 drafts not frozen | SUPPORTING | Index repeats packet status | **UPDATE / KEEP / LINK** |
| `reports/slint-migration/B-01/B-01_Blockers_and_Decisions.md` | Detailed G0B-01–14 and DEP-01/02 register | Historical blocker snapshot; several entries are stale against later decisions and implementation | EVIDENCE / STALE | Overlaps master blocker summary and decision log | **UPDATE** with dated reconciliation; keep detailed closure evidence |
| `reports/slint-migration/B-01/B-01_Final_Report.md` | B-01 Wave 0 execution and CI report | Historical baseline and run-specific status; Gate 0b blocked | EVIDENCE / HISTORICAL | Older source SHA and CI runs must not be presented as current | **UPDATE / KEEP / LINK**; add new evidence only through dated updates |
| `reports/slint-migration/B-01/B-01_Handover.md` | B-01 handover, scope and implementation prohibition | Wave 0 only; implementation blocked by Gate 0b | SUPPORTING / EVIDENCE | Must remain consistent with current Gate 0b status | **UPDATE / KEEP / LINK** |
| `reports/slint-migration/B-01/B-01_Source_Inventory.md` | B-01 source inventory | Source-pinned evidence | EVIDENCE | Narrow B-01 scope, not whole-repository inventory | **KEEP / LINK** |
| `reports/slint-migration/B-01/B-01_Endpoint_Contract_Audit.md` | Endpoint contract audit | Source-level evidence | EVIDENCE / SUPPORTING | Not proof of deployed Worker behavior | **KEEP / LINK** |
| `reports/slint-migration/B-01/B-01_C01_Worker_API_Contract_Draft.md` | Draft Worker API contract | DRAFT — NOT FROZEN | SUPPORTING / UNRESOLVED | Requires exact source/handler reconciliation and approval | **KEEP / LINK** |
| `reports/slint-migration/B-01/B-01_C01_Dispatch_Reconciliation.md` | Source-pinned route dispatch predicates and gaps | Source-level reconciliation | EVIDENCE / SUPPORTING | Does not prove live/deployed semantics | **KEEP / LINK** |
| `reports/slint-migration/B-01/B-01_C03_Error_Mapping_Proposal.md` | Worker error taxonomy proposal | DRAFT; requires approval | SUPPORTING / UNRESOLVED | Not an approved frozen error map | **KEEP / LINK** |
| `reports/slint-migration/B-01/B-01_Test_Coverage_and_Gaps.md` | B-01 test coverage and gaps | Evidence inventory | EVIDENCE | Narrow B-01 test scope | **KEEP / LINK** |
| `docs/production/api-contract.md` | Cloudflare Worker API source contract | Source-level contract; baseline stamped 2026-11-01; explicitly not deployed proof | AUTHORITATIVE for documented Worker source contract / SUPPORTING | Must be re-pinned when Worker source changes | **KEEP / LINK** |
| `docs/SECURITY_AUTHENTICATION_MODEL.md` | Authentication, authorization and All-Father policy | Explicitly marked authoritative | AUTHORITATIVE | Cross-reference Desktop/Admin migration-specific session rules | **KEEP / LINK** |
| `docs/BACKEND_ARCHITECTURE.md` | Cloudflare Worker/D1 and native Axum architecture boundaries | Current architecture document | AUTHORITATIVE | Explicitly says Worker and Axum are separate backends | **KEEP / LINK** |
| `shared/i18n/README.md` | Canonical localization source, generator and validation contract | Localization governance document | AUTHORITATIVE | Translation quality review is separate from key parity/schema checks | **KEEP / LINK** |
| `shared/i18n/fa.json` | Persian locale catalogue | Present and synchronized; new strings still await Persian-speaker review | EVIDENCE / UNRESOLVED | Do not treat valid JSON or CI parity as translation approval | **KEEP / LINK** |
| `shared/i18n/en.json` | Canonical English locale catalogue | Canonical source language | AUTHORITATIVE localization source | Paired with Persian catalogue | **KEEP / LINK** |
| `scripts/sync-i18n.mjs` | Localization generation and validation tool | Executable validation mechanism | SUPPORTING / EVIDENCE | CI validates structure/parity, not human translation quality | **KEEP / LINK** |
| `docs/DECISIONS.md` | Project-wide architectural rulings, including Team 4 networking/P2P, Team 5 composition ownership and S2 unified keyring adapter | Historical project ruling record; P2P-1/2/4/6, Team 4 and S2/Team 5 sections are technical/orchestrator rulings, with no explicit owner approval in the inspected text | SUPPORTING / EVIDENCE / HISTORICAL | Relevant to relay/P2P architecture, content-hash downloads, composition and keyring implementation; not the current Slint session or file-transfer policy authority; do not elevate AI/orchestrator rulings to owner approval | **KEEP / LINK** |
| `docs/AUDIT_REGISTER.md` | Historical audit and revalidation register | Contains explicit warning that much older audit content is historical | HISTORICAL / EVIDENCE | Do not treat unrevalidated findings as current | **KEEP / LINK** |
| `docs/DECISIONS 1-5.md` | Older Increment 1–5 decision snapshot | Exact byte-for-byte prefix of current `docs/DECISIONS.md` (148,096 characters vs 234,327); historical duplicate retained | DUPLICATE / HISTORICAL | Duplicates the earlier portion of the current decisions file, including older Team 5 rulings | **ARCHIVE / KEEP** in place; no deletion or rename in this audit |
| `docs/Structure.md` | Repository path inventory | Baseline 2026-10-23; not a current full tree | SUPPORTING / STALE | Old inventory date; use as orientation only | **KEEP**; refresh only if separately requested |
| `README.md` | Product overview | General product description, not migration policy | SUPPORTING | No migration authority | **KEEP / LINK** |
| `ONYX_PHASE_1_REPORT.md` | Historical security/architecture remediation report with current-status addenda | Historical; later corrections and current-state notes exist | HISTORICAL / EVIDENCE | Older instructions may be superseded by explicit current addenda | **KEEP / LINK** |
| `docs/diagnostics/stage-3-unified-diagnostics.md` | Diagnostics auth and workflow evidence | Supporting operational record | SUPPORTING / EVIDENCE | Not a migration master | **KEEP / LINK** |
| `reports/auth-repair/01-baseline.md` | Authentication repair baseline | Historical evidence | HISTORICAL / EVIDENCE | Contains an earlier governance-file search; AGENTS/CONTRIBUTING absence separately rechecked at root | **KEEP / LINK** |
| `reports/auth-repair/02-auth-contract.md` | Auth route and contract review | Supporting evidence | SUPPORTING / EVIDENCE | Cross-cutting auth evidence | **KEEP / LINK** |
| `reports/auth-repair/04-allfather-provisioning.md` | All-Father provisioning procedure and safeguards | Supporting security/operations evidence | SUPPORTING / EVIDENCE | Do not execute production provisioning during this audit | **KEEP / LINK** |
| `docs/audits/2026-10-25/ONYX_Audit_Report.md` | Historical architecture and product audit | Historical snapshot | HISTORICAL / EVIDENCE | Architecture claims must be checked against current architecture document | **KEEP / LINK** |
| `docs/audits/desktop-ui-ux-forensic-inventory/` | Referenced Desktop/Admin UI forensic inventory directory | No individual file path could be revalidated in the current repository search; directory contents not accessible | UNRESOLVED | Do not rely on this directory reference until a concrete file path is located | **UNRESOLVED**; locate the intended files or remove only the dangling reference after owner review |
| `.github/workflows/ci.yml`, `.github/workflows/security.yml` | CI/security workflow evidence | Current workflow behavior must be checked at exact commit/run | EVIDENCE | Recent owner decision removed web/mobile-pwa checks; i18n remains | **KEEP / LINK** |
| `.github/workflows/deploy-cloudflare-edge.yml` | Deployment-capable Cloudflare Worker workflow | Present in repository; not dispatched in this audit | EVIDENCE / DEPLOYMENT | Requires explicit owner authorization; workflow presence is not deployment evidence | **KEEP / LINK** |
| `deploy/cloudflare/edge-gateway/wrangler.toml` | Production Worker/D1 configuration | Deployment configuration exists | EVIDENCE / DEPLOYMENT | Not inspected against production account state; never run as part of this audit | **KEEP / LINK** |
| `deploy/cloudflare/edge-gateway/wrangler.dev.toml` | Local-only Worker/D1 configuration | README says fixed local D1 identifier and restricted to local commands | EVIDENCE / DEPLOYMENT | Local configuration must not be used for remote commands | **KEEP / LINK** |
| `deploy/cloudflare/edge-gateway/README.md` and Worker source/tests | Worker operational/API behavior | Implementation evidence, not owner approval | EVIDENCE | Must be pinned to exact source and CI revision | **KEEP / LINK** |
| `deploy/cloudflare/edge-gateway/src/index.ts` | Worker authentication, D1 account creation, Onyx ID, password-change enforcement, staff import and report response implementation | Current audited source contains O-1–O-6 behavior and staff report generation | EVIDENCE / IMPLEMENTATION | Source presence does not prove production deployment or applied D1 migration | **KEEP / LINK** |
| `deploy/cloudflare/edge-gateway/src/xlsx.ts` | XLSX import parser and creation-report writer | One-sheet writer; text cells; formula-injection protection | EVIDENCE / IMPLEMENTATION | Unit-test source exists; audit did not execute tests | **KEEP / LINK** |
| `deploy/cloudflare/edge-gateway/migrations/0009_public_ids.sql` | D1 `id_sequences` and `public_ids` migration | Present in Wrangler migrations; source migration header calls O-2 an “owner decision” but the decision log attributes O-2 to coordinator delegation | EVIDENCE / IMPLEMENTATION / ATTRIBUTION CONFLICT | No production D1 application evidence; audit did not apply the migration | **KEEP / LINK**; do not modify schema in this audit |
| `deploy/cloudflare/edge-gateway/test/backend-accounts.test.ts` | Dedicated backend accounts and report tests | Test cases cover O-1–O-6, account creation, IDs, failure atomicity, password handling, report output and formula injection | EVIDENCE / TEST COVERAGE | Test file exists; current-head test execution was not independently verified | **KEEP / LINK** |
| `crates/applications/desktop-session/src/lib.rs`, `crates/applications/desktop-session/src/secure_storage/keyring_adapter.rs` | Shared session authority and OS-keyring adapter | Source evidence; hermetic tests do not prove real OS keyring behavior | EVIDENCE / IMPLEMENTATION | Device round-trip and two-host coexistence remain open | **KEEP / LINK** |
| `crates/bins/desktop-slint/src/composition.rs`, `crates/bins/desktop-slint/build.rs`, `crates/bins/desktop-slint/Cargo.toml` | Slint Desktop host composition and build | Source evidence for host boundary and dependencies | EVIDENCE / IMPLEMENTATION | Normal dependency proof does not establish packaged platform behavior | **KEEP / LINK** |
| `crates/bins/desktop-shell/Cargo.toml` | Existing Tauri Desktop shell dependency boundary | Source evidence for current production shell | EVIDENCE / SUPPORTING | Tauri remains production default until release authorization | **KEEP / LINK** |
| `crates/applications/client-composition/src/theme_contrast.rs` | Theme contrast policy/helper | Source evidence for theme contrast behavior | EVIDENCE / SUPPORTING | Requires per-theme/per-platform acceptance evidence | **KEEP / LINK** |
| `.github/workflows/desktop-slint.yml` | Desktop Slint CI workflow | CI evidence for selected compile/test paths | EVIDENCE | A workflow job is not manual/device verification | **KEEP / LINK** |

| `common/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md` | Governing BOUND migration plan referenced by the decision log | Not present at the referenced path (GitHub contents API returned 404); external/historical copy not accessible in this audit | UNRESOLVED / MISSING REFERENCE | Gate 0b criteria cannot be independently checked against this missing in-repository copy | **UNRESOLVED**; restore the approved plan or record its stable external location/version |
| `docs/i18n/PHASE-0.5-DECISIONS.md` | Phase 0.5 localization decisions, declared authoritative by `shared/i18n/README.md` | Not present at the referenced path (404) | UNRESOLVED / MISSING REFERENCE | The localization README points to an absent authority document; contents cannot be inferred | **UNRESOLVED**; locate the approved historical file or update the reference through owner review |

## Governance and contribution files checked

- Root `README.md`: present and read.
- Root `AGENTS.md`: not found (GitHub contents API returned 404).
- Root `CONTRIBUTING.md`: not found (GitHub contents API returned 404).
- The earlier `reports/auth-repair/01-baseline.md` also reports no AGENTS/CLAUDE/CONTRIBUTING/SECURITY/CODEOWNERS in its inspected tree; that historical statement is not treated as a substitute for a recursive current-tree check.
- `docs/Structure.md` exists but is a 2026-10-23 path-level inventory, not a full recursive list.

## Disposition summary

- **KEEP:** authoritative domain policies, decision history, technical specifications, evidence reports and historical records.
- **UPDATE:** master record; backend handover/current-state; P-01 evidence summary; file-transfer specification; B-01 blocker register; decision log (append-only amendment).
- **LINK:** detailed B-01 reports, architecture, security, localization and product specifications from the master.
- **MERGE:** verified status summaries only, into the existing master; do not copy detailed specs.
- **ARCHIVE:** preserve dated P-01, B-01 and audit snapshots in place.
- **RETIRE:** none recommended at this stage. No file is deleted or renamed by this audit.

## Audit limitations

GitHub history and pull requests were inspected selectively. PR #443 and PR #442 are merged; #443 added only the existing master record. The recent commit list includes both merges and relevant migration commits. The PR #443 issue has no comments, submitted reviews or inline review threads. No full recursive tree export, complete all-branch history, external project artifact or device-lab record was available in this audit session.


### PR and CI traceability note

PR #329 (B-01 Wave 0 packet), PR #442 (decision-log reconciliation) and PR #443 (the existing master record) are merged. PR #329 follow-up run 40428710390 completed with security audit jobs cancelled and `merge-gate` failed; its merged packet is not evidence of green required checks. PR #443 added only the existing master record and has no comments, submitted reviews or inline review threads.