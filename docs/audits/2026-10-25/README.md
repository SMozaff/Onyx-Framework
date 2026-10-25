# ONYX Audit — 2026-10-25

**Repository:** [SMozaff/Onyx-Framework](https://github.com/SMozaff/Onyx-Framework)  
**Audited base:** `main` at `d8f64889cfe4473d40d50c41b0992bd22955eb7b`  
**Status:** Partial source audit; runtime acceptance criteria remain unverified.

## Report

- [ONYX Audit Report](./ONYX_Audit_Report.md) — consolidated report covering the desktop UI/UX forensic inventory and product knowledge/operational audit prompts.

The report includes scope and methodology, application and route inventory, static UI counts, Rust/Tauri integration, backend boundaries, authentication and authorization model, operational capabilities, organization/hierarchy evidence, platform strategy history, CI/demo evidence, discrepancies, and unresolved facts.

## Method and limitations

Inspection was performed against the pinned default-branch source through GitHub repository and file APIs. A local clone was unavailable in the audit environment. The work made no changes to application source, runtime configuration, credentials, schemas, deployments, or production services.

The report explicitly distinguishes source-confirmed findings, documentation-only statements, recorded CI evidence, and unverified runtime behavior. It does not claim Windows/Linux or Android end-to-end verification, production All-Father login success, live Worker health, or cross-backend persistence consistency.

## Main findings at a glance

- Two Tauri 2 desktop apps are present: ONYX and ONYX Admin; each configures a single 1280×800 window.
- Main-branch CI run `40198813416` was observed as successful, but this is not live end-to-end proof.
- Current All-Father authentication is documented as ONYX username/password through Cloudflare Worker/D1, not Google/Supabase. Live production login remains unverified.
- Native Axum/SQLx and Cloudflare Worker/D1 are separate backend implementations; a common authoritative datastore or end-to-end replication guarantee must not be assumed.
- User authority roles, operational class labels, reporting parent fields, and legacy manager flags are separate concepts.
- Existing source supports organizational user/profile administration, hierarchy metadata, and CSV/JSON profile transfer. Dedicated department/team tables, an interactive spiral/org-chart module, XLSX import, standalone timesheets/payments, and an optional product-module activation system were not found in inspected current sources.
- No application or production state was changed. The audit is marked partial where the attached criteria require exhaustive static-instance reconciliation or runtime tests.

See the consolidated report for sources, detailed findings, confidence labels, and open questions.
