# ONYX Slint Migration — Master Migration Record (DRAFT)

**Status:** Draft for project-owner review. This document is an overview and proposed navigation point; it does **not** pass Gate 0b, approve a technical decision, authorize a cutover, or supersede existing records until the owner explicitly approves that change.

**Prepared:** 2026-11-10  
**Repository:** [SMozaff/Onyx-Framework](https://github.com/SMozaff/Onyx-Framework)  
**Migration method:** BOUND  
**Current gate:** Gate 0b remains OPEN / BLOCKED.

> **Interim authority rule:** Until the owner approves this master record and its authority model, the existing [Slint decision log](decision-log.md) remains the migration decision record. Where source documents conflict, this master record flags the conflict; it does not silently choose a winner. Preserve original documents and history.

## 1. Purpose and how to use this record

This document gives one place to understand the migration's scope, current decisions, unresolved questions, blockers, evidence and source documents. It is a controlled summary and index, not a replacement for detailed technical specifications, test reports, handovers or historical records.

After owner approval, the intended model is:
- This master record: current migration overview, decision index, blocker index, gate status and links.
- Decision log: append-only decision history and decision evidence.
- B-01 blocker register: detailed blocker ownership and exit evidence.
- Domain specifications and handovers: detailed technical contracts and implementation instructions.
- Other documents: link to the relevant decision or blocker IDs rather than maintaining independent, conflicting status statements.

Do not delete source documents or rewrite historical entries to make them appear as if they were always consistent. Correct stale current-state statements with dated amendments and links to the evidence.

## 2. Current project position

- The project is completing the ONYX Slint migration under the BOUND method, in small reviewable changes.
- B1 (a separate native Slint desktop host sharing Rust crates in-process) is recorded as conditionally approved on 2026-11-04. Track A was required first; Track B follows only under the recorded conditions.
- Track A relay-auth work is recorded as implemented and merged in PR #407. The decision log says live relay-ticket checks still require a deployed Worker and an authorized test account.
- Track B has progressed: the shared `desktop-session` crate is used by the Slint host; the Tauri-free dependency check is recorded as passing; lifecycle and coexistence evidence remains incomplete.
- Gate 0b is **not passed**. The B-01 blocker register says mandatory evidence, decisions, approvals and signed Contract Freeze v1 remain outstanding.
- Do not start the postponed web-ui Vite upgrades (#424 and #429) without owner approval. The web UI is explicitly postponed.
- No final cutover or production migration implementation is authorized by this overview.

## 3. Decision index

The entries below summarize what the currently inspected records say. “Owner-approved” means the source record explicitly attributes approval to the owner. “Delegated decision” means the coordinator recorded the decision under the owner's standing “decide and document” instruction; it must not be relabelled as a direct, verbatim owner approval.

| Topic / ID | Recorded position | Authority / status | Primary record |
|---|---|---|---|
| P-01 desktop architecture | B1 separate native Slint host, sharing Rust crates in-process; Track A first, Track B conditional on Track A | Owner approval recorded 2026-11-04; Gate 0b still open | [Decision log](decision-log.md), [P-01 investigation](P-01-B1-investigation-2026-10-27.md) |
| Q-01 Admin token persistence | Admin tokens memory-only; no disk, OS keyring or browser storage | Owner-approved decision recorded in decision log | [Decision log](decision-log.md) |
| Desktop session persistence | Desktop keeps a persistent session; refresh token in OS keyring, access token memory-only; restore on startup; logout clears session/keyring | High-level persistence distinction is also recorded as an owner decision in [Contract Freeze v1](../../slint/CONTRACT_FREEZE_V1.md). Detailed refresh-token-only policy says coordinator decided under standing authority; reconcile attribution and exact wording before freezing | [Desktop token policy](desktop-token-storage-policy.md), [Contract Freeze v1](../../slint/CONTRACT_FREEZE_V1.md) |
| O-1 to O-6 backend accounts | Argon2id; D1 authority; Onyx ID login with username/email aliases; forced-change flag; generated ID for import rows; extend import endpoint for XLSX/account creation | Decision log says coordinator decided under owner’s 2026-11-01 “decide and document” instruction. Backend current-state and handover docs still label these open. Treat as a documentation/authority conflict, not six fresh questions | [Decision log](decision-log.md), [backend current state](backend-accounts-current-state.md), [backend handover](backend-accounts-handover.md) |
| File-send route | Decision log says user chooses Local (BLE and LAN) or Internet before sending; details of P2P scope and per-send vs per-session choice remain open | Owner decision recorded 2026-11-07; product spec differs on routine LAN and deferred emergency BLE/Wi-Fi Direct. Reconcile before implementation | [Decision log](decision-log.md), [file-transfer spec](../../product/file-transfer.md) |
| Q-05 profile membership | Preserve list/API contract; semicolon-delimited behavior; reject semicolon in team names | Approved/recorded; implementation and route wiring evidence must be read from current code | [Decision log](decision-log.md) |
| Q-14 preferences | Persist theme and language preferences; schema details and implementation remain incomplete | Owner approval recorded for schema, but required schema artifact and implementation remain outstanding | [Decision log](decision-log.md) |
| Q-16 / U-05 ID digits | Latin digits for display only; canonical data unchanged | Approved and display helper implemented; decision log says helper is not yet used by every screen | [Decision log](decision-log.md) |
| U-04 policy lifecycle | Active/Retired lifecycle, immutable published versions, one pending draft, explicit precondition errors | Approved; parity and implementation evidence remain required | [Decision log](decision-log.md) |
| Web UI | Web UI postponed; no Vite upgrades #424/#429 without owner approval | Owner decision recorded 2026-11-08 | [Web UI postponed](../../product/web-ui-postponed.md) |

## 4. Conflicts and stale statements requiring reconciliation

These are confirmed document-level discrepancies found during the initial review. They are not silently resolved here.

| Issue | Conflicting records | Required treatment |
|---|---|---|
| Desktop/Admin authentication | Contract Freeze v1 records the owner’s 2026-10-28 decision that Desktop persists across restarts and Admin is memory-only. The detailed desktop policy records a later coordinator decision under standing authority. The 2026-11-01 P-01 evidence summary still calls the difference an owner question. | Preserve the owner decision and date; amend the stale evidence summary. Separately identify which detailed implementation constraints were delegated decisions. |
| Backend O-1 to O-6 | Decision log records the six decisions under delegated authority on 2026-11-01. Backend current-state and handover docs still label them undecided / owner-only. | Update the stale status in both docs with links to the decision-log entry. Do not change the technical decisions without a new authorized decision. |
| File-transfer contract | Decision log describes Local as BLE + LAN and says Local uses relay/P2P. Product spec describes LAN as the routine local route and BLE/Wi-Fi Direct as a separate deferred emergency module; it also describes switching LAN/Internet during a session. | Obtain source evidence for the exact owner instruction, then issue a dated reconciliation. Do not infer the unresolved P2P details or selection cadence. |
| Gate 0b blocker status | Decision log and B-01 blocker register contain different blocker descriptions and older “open” states for items with later decisions or implementation evidence. | Keep Gate 0b blocked, but reconcile each blocker by ID against latest evidence; distinguish decision made from implementation/evidence accepted. |
| Track A / P-01 evidence | P-01 evidence summary dated 2026-11-01 says Track A not started; decision log dated 2026-11-07 says Track A merged in PR #407, with live checks still pending. | Update the summary with a dated status amendment; retain its original historical snapshot. |
| Historical audit register | `docs/AUDIT_REGISTER.md` explicitly warns that much of its August 2026 audit content is historical and not uniformly revalidated. | Use only its current reconciliation table as current evidence unless an individual historical finding has been rechecked. |

## 5. Gate 0b and blocker tracking

**Gate 0b status: BLOCKED / NOT PASSED.** This status is explicit in the decision log, B-01 blocker register and Contract Freeze v1.

The B-01 register currently tracks:
- G0B-01: Gate 0b / Contract Freeze v1.
- G0B-02: crate/path confirmation.
- G0B-03: C-01 Worker API source baseline.
- G0B-04: Q-01 token persistence/session lifecycle.
- G0B-05: Q-05 profile membership/wire representation.
- G0B-06: Q-14 preference persistence.
- G0B-07: Q-16/U-05 ID display digits.
- G0B-08: U-04 policy transitions.
- G0B-09: Slint feasibility evidence U-01/U-02/U-03/U-10/U-12.
- G0B-10: C-03 error taxonomy.
- G0B-11: C-09 fixture schema/version.
- G0B-12: organization-admin create DTO.
- G0B-13: security/license CI failure.
- G0B-14: B-01 documentation PR CI.
- DEP-01 / DEP-02: Rust verification environment and documentation handover.

The decision log has additional gate evidence and governance requirements. The two lists must be mapped by requirement and evidence, not mechanically treated as identical numbering.

**Gate closure rule:** every required item needs current evidence and an explicit accepted disposition; the formal sign-off must identify the exact baseline, accepted evidence, frozen contract versions, approved fallbacks, approver/date and scoped authorization. This master record is not that sign-off.

## 6. Source-document catalogue

Preserve these documents as detailed records. This list is the initial inventory of the key files reviewed, not a claim that every migration-related file in the repository has been inventoried.

| Document | Role |
|---|---|
| [Decision log](decision-log.md) | Existing migration decision history; interim decision authority |
| [B-01 blocker register](../../../reports/slint-migration/B-01/B-01_Blockers_and_Decisions.md) | Gate 0b blocker ownership and exit evidence |
| [Contract Freeze v1](../../slint/CONTRACT_FREEZE_V1.md) | Current contract snapshot and freeze gaps |
| [P-01 evidence summary](P-01-evidence-summary-2026-11-01.md) | Historical evidence snapshot; contains stale statements requiring amendment |
| [P-01 B1 investigation](P-01-B1-investigation-2026-10-27.md) | Architecture investigation and evidence |
| [Desktop token policy](desktop-token-storage-policy.md) | Detailed Desktop/Admin token-storage rules |
| [Backend current state](backend-accounts-current-state.md) | Worker/D1 current-state findings; O-1 to O-6 status needs reconciliation |
| [Backend handover](backend-accounts-handover.md) | Backend lane scope, constraints and implementation instructions |
| [Backend creation-report spec](backend-accounts-creation-report-spec.md) | Excel account-creation report fields and safeguards |
| [File-transfer specification](../../product/file-transfer.md) | Owner file-transfer scope and behavior |
| [Web UI postponed decision](../../product/web-ui-postponed.md) | Web UI scope and dependency work explicitly postponed |
| [Historical audit register](../../AUDIT_REGISTER.md) | Historical audit plus current revalidation table; do not treat all archived findings as current |

## 7. Governance rules proposed for owner approval

1. Before asking the owner a question, search this master record, the decision log, relevant specifications, PR discussion and available conversation evidence.
2. Ask only about a genuinely unresolved choice, or a conflict that cannot be resolved from existing evidence.
3. Distinguish **owner-approved**, **delegated/coordinator decision**, **proposal**, **pending**, **implemented**, **verified**, and **superseded**. These states are not interchangeable.
4. Every new decision gets a unique ID, exact wording, date, decision authority, source evidence, affected documents and implementation/evidence status.
5. Append corrections and superseding decisions; do not erase prior history.
6. A document's “open” label is stale if a later authoritative decision exists. Update it with a dated amendment and link rather than reopening the question.
7. A decision being recorded does not mean it is implemented, tested, accepted as Gate 0b evidence or deployed.
8. No production cutover, Gate 0b sign-off, merge or deployment is implied by this document. Follow the existing owner-controlled review and approval process.

## 8. Proposed next steps

1. Owner reviews and approves or corrects this master record and its proposed authority model.
2. Inventory the remaining migration-related files and open PRs; compare each against the decision log and B-01 register.
3. Reconcile the confirmed stale statements in the backend documents and P-01 evidence summary, preserving historical wording through dated amendments.
4. Resolve the file-transfer contract discrepancy only from the original owner instruction/evidence; do not guess.
5. Map every B-01 blocker to the latest evidence and the decision-log entry, then update statuses with source links.
6. Keep all application behavior unchanged in this documentation pass. Keep Gate 0b blocked until its formal exit criteria are met and the owner signs off.

---

**Review boundary:** This draft was prepared from the key repository documents listed above. It is a first-pass master index and discrepancy report, not an exhaustive audit of every source file, Git commit, PR discussion, device test or external conversation.
