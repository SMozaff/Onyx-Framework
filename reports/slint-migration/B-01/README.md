# B-01 Worker Client — Wave 0 documentation packet

This directory contains the seven audit/draft/handover deliverables plus the final report for ONYX Framework B-01.

## Reports
- `B-01_Final_Report.md`
- `B-01_Source_Inventory.md`
- `B-01_Endpoint_Contract_Audit.md`
- `B-01_C01_Worker_API_Contract_Draft.md`
- `B-01_C01_Dispatch_Reconciliation.md` — source-pinned dispatch predicates and contract gaps
- `B-01_C03_Error_Mapping_Proposal.md`
- `B-01_Test_Coverage_and_Gaps.md`
- `B-01_Blockers_and_Decisions.md`
- `B-01_Handover.md`

## State
- Audited baseline: `main` at `b056c6ef204080446e84b31f3d35a226e6a40867`.
- Gate 0b: **BLOCKED — required evidence/contract approvals and signed Contract Freeze v1 remain outstanding**. The project owner authorized the assistant to close the gate and proceed to implementation only after its mandatory criteria are satisfied; this is not itself a gate sign-off.
- C-01/C-03/C-09: **DRAFT — NOT FROZEN**.
- No production source code is included.
- Latest verified docs-PR CI run [40426925219](https://github.com/SMozaff/Onyx-Framework/actions/runs/40426925219) **FAILED** at `Security audits / rust-advisories` → `Advisory and source-policy audit` (`cargo deny check advisories bans licenses sources`); merge-gate failed as a consequence. Web, Rust build/test, i18n, mobile-PWA, native UI evidence and other listed jobs passed in that run. Slint scaffold run 40336190967 passed for scaffold only.
- Proposed destination `reports/slint-migration/B-01/` was approved for documentation by the user. This does not authorize implementation or freeze contract drafts.
- Candidate fixture examples exist in the separate local artifact package but are **not included in this repository PR**.

Review the final report, dispatch reconciliation, and blockers before contract freeze. Route reconciliation is source-level only; no deployed Worker behavior is inferred.
