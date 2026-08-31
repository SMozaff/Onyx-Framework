# ONYX Mobile Client Strategy Manifesto v1.1

**Document ID:** ONYX-MOB-00  
**Status:** Normative Migration Baseline  
**Version:** 1.1 — Read-Only iOS Observer Revision  
**Product:** ONYX Mission Operations Platform  
**Method:** Interface-First Execution Methodology (IFEM)  
**Scope:** Native Android Operational Client + iOS Progressive Web App Observer Client  
**Normative language:** MUST, MUST NOT, SHOULD, SHOULD NOT, MAY  
**Date:** 31 August 2026

> **Execution belongs to trusted operational clients. Awareness may be delivered more broadly, but awareness must never silently become authority.**

---

# 1. Purpose

This manifesto fixes the mobile-client architecture of ONYX before implementation begins.

The approved target is:

- **Android:** native Kotlin **Operational Client**.
- **iOS:** installable Progressive Web App **Observer Client**.
- **Kotlin Multiplatform:** excluded.
- **Flutter:** frozen during migration and retired only after proven Android replacement.
- **Rust mobile-core:** retained as Android's behavioral authority.
- **PWA:** authenticated, server-enforced, read-only operational frame.

The iOS PWA is not a reduced imitation of the Android application.

It is a deliberately separate client class whose purpose is:

- situational awareness;
- Mission and Task visibility;
- lifecycle/status visibility;
- notification delivery;
- evidence and audit viewing where authorized;
- file viewing/download where authorized;
- read-only access to ONYX projections.

It MUST NOT alter ONYX operational truth.

---

# 2. Governing Principle

## The contract survives the client.

A UI framework is replaceable.

The following are not:

- command contracts;
- query contracts;
- aggregate invariants;
- authority rules;
- lifecycle epochs;
- authority epochs;
- object versions;
- conflict behavior;
- audit semantics;
- tenant boundaries;
- persistence ownership;
- synchronization rules.

Android Kotlin and the iOS PWA adapt to ONYX.

ONYX does not mutate its domain model merely to accommodate either client.

---

# 3. Client-Service Model

ONYX defines two distinct mobile client-service classes.

## 3.1 Android — Operational Client Service

Android is a full operational endpoint.

It MAY, subject to normal ONYX authority:

- execute commands;
- create or modify permitted operational objects;
- participate in synchronization;
- operate against local state;
- upload/download files;
- resolve conflicts;
- perform approval actions;
- run background work.

The Android client service composes:

- Kotlin presentation;
- Kotlin client facade;
- HTTP authentication/server gateway;
- JNI native gateway;
- Rust `mobile-core`;
- local SQLite/file state;
- synchronization/conflict behavior.

## 3.2 iOS PWA — Observer Client Service

The PWA is a read-only operational endpoint.

It MAY:

- authenticate;
- read permitted projections;
- inspect Missions and Tasks;
- inspect lifecycle/status;
- inspect approval requirements;
- inspect notifications;
- inspect permitted evidence/audit views;
- download/view permitted files;
- register/unregister Web Push subscriptions;
- manage its own session lifecycle.

It MUST NOT:

- create Missions;
- create Tasks;
- modify Missions;
- modify Tasks;
- approve or reject;
- change lifecycle state;
- resolve conflicts;
- upload files;
- delete files;
- mutate organization structure;
- mutate users;
- mutate policy;
- execute administrative commands;
- submit domain commands;
- queue offline domain mutations.

---

# 4. Read-Only Means Server-Enforced

The PWA is not read-only because buttons are hidden.

The PWA is read-only because the ONYX backend enforces a maximum capability ceiling for the observer client class.

The client MUST identify as:

`client_type = "mobile_observer"`

The server MUST compute:

> **effective permissions = authenticated user permissions ∩ mobile_observer capabilities**

A highly privileged administrator using the PWA still receives only observer capabilities through that client class.

Changing JavaScript, replaying requests, or calling APIs manually MUST NOT bypass this ceiling.

**Implementation note (H10):** this section is a requirement, not a
description of a mechanism that already existed. Before this hardening
task, `client_type` was an unchecked `Option<String>` compared against a
single hardcoded literal, and no endpoint denied a mutation on the basis
of client class — the ceiling described above had no enforcement point
anywhere in the codebase. See ONYX-MOB-01 §26 P1.1–P1.5 and this
project's `DECISIONS.md` H10 entry for what was actually built to satisfy
it, in `crates/bins/api-server/src/routes/client_type.rs`.

---

# 5. Operational Mutations vs Client-Control Mutations

“Read-only” applies to **ONYX operational/business state**.

A small set of client-control operations MAY still mutate non-operational state:

- login/session creation;
- token/session refresh;
- logout/session revocation;
- Web Push subscription registration;
- Web Push subscription deletion;
- browser/device preference metadata where separately approved.

These exceptions MUST NOT modify Mission, Task, Approval, File-domain, governance, organizational, policy, or execution state.

This distinction is normative.

---

# 6. Android Decision

ONYX Android MUST be a **plain native Android Kotlin application**.

It MUST NOT use:

- Flutter for the replacement;
- Kotlin Multiplatform;
- a cross-platform abstraction introduced solely to recover the old shared-client model.

The baseline Android UI technology is **Jetpack Compose** with:

- ViewModel-owned state;
- unidirectional data flow;
- type-safe navigation;
- platform accessibility semantics;
- Android-native lifecycle integration.

The current minimum Android version remains **API 29** unless changed by explicit compatibility decision.

---

# 7. iOS Decision

ONYX iOS MUST be delivered as an **installable PWA Observer Client**.

The PWA does not contain:

- Rust native FFI;
- local ONYX SQLite;
- the Rust local sync engine;
- native WorkManager/BGTask execution;
- operational command submission;
- conflict resolution;
- approval actions;
- file upload;
- durable background command reconciliation.

It is an authenticated ONYX read frame with:

- Home Screen installation;
- application-shell caching;
- responsive mobile UI;
- authorized file viewing/download;
- Web Push where supported;
- explicit offline-state handling.

---

# 8. Flutter Is a Reference, Not Waste

The Flutter client currently represents working, tested behavior.

It MUST NOT be deleted at migration start.

During migration it becomes a **Frozen Reference Implementation**:

- no ordinary new product development;
- security fixes MAY continue;
- critical defects MAY continue;
- existing Android behavior is available for parity comparison;
- removal occurs only after Kotlin Android acceptance;
- removal occurs in a dedicated cleanup change.

The PWA does not need Flutter feature parity because its scope is intentionally narrower.

Its target is **Observer Capability Parity**, not Android parity.

---

# 9. This Is Not a Domain Rewrite

The migration replaces:

- screens;
- widgets;
- navigation;
- client state presentation;
- Android platform integration;
- Dart bindings;
- Android native adapter code.

The migration MUST NOT rewrite in Kotlin or JavaScript:

- Mission rules;
- Task rules;
- approval semantics;
- authority logic;
- synchronization;
- CRDT logic;
- conflicts;
- persistence semantics;
- file-domain rules;
- audit semantics.

Those remain ONYX responsibilities.

---

# 10. mobile-core Constitution

The existing `mobile-core` remains Android's local execution engine.

The invariant is:

> **The public mobile-core contract and observable ONYX behavior remain frozen.**

Internal composition MAY change to inject required platform adapters if observable behavior is preserved and the change is separately tested.

The PWA does not call `mobile-core`.

---

# 11. JNI Boundary Law

The Android client MUST use a deliberately thin JNI adapter between Kotlin and Rust.

The JNI layer MUST NOT contain business logic.

Its responsibilities are limited to:

- native library registration/loading;
- value marshalling;
- native handle management;
- string and byte conversion;
- callback translation;
- memory ownership;
- error translation.

Preferred architecture:

`Kotlin → mobile-android-jni → mobile-core`

Native loading SHOULD occur once at application/process initialization.

---

# 12. Android Is the Local-First Operational Client

Android MUST preserve ONYX's real local capabilities where frozen contracts permit them:

- local startup;
- local SQLite;
- offline queries;
- offline commands;
- local file storage;
- synchronization;
- conflict inspection;
- conflict resolution;
- background execution;
- hierarchy-aware approval checks.

A migration that turns Android into a thin HTTP wrapper has failed even if its UI reaches visual parity.

---

# 13. PWA Is an Awareness Endpoint

The PWA exists to answer:

- What is happening?
- What requires attention?
- What is the current state?
- What evidence is available?
- What changed?
- What requires action in an Operational Client?

It does not answer:

- What should be executed locally?
- What command should be committed?
- What conflict should be resolved?
- What approval should be decided?
- What operational state should be mutated?

For actionable states it SHOULD display language such as:

> **Action required in an ONYX Operational Client.**

The UI SHOULD NOT render disabled operational controls in a way that implies the PWA is merely malfunctioning.

---

# 14. Observer Client-Service Contract

The PWA client service SHOULD expose a deliberately narrow interface.

Conceptually:

```text
ObserverClient
  authenticate(...)
  refreshSession(...)
  logout(...)
  getDashboard(...)
  listMissions(...)
  getMission(...)
  listTasks(...)
  getTask(...)
  listNotifications(...)
  getEvidence(...)
  getAuditView(...)
  getHierarchyView(...)
  getFileMetadata(...)
  downloadFile(...)
  registerPushSubscription(...)
  unregisterPushSubscription(...)
```

It MUST NOT expose:

```text
createMission
updateMission
createTask
updateTask
approve
reject
transitionLifecycle
resolveConflict
uploadFile
deleteFile
mutateUser
mutateOrganization
mutatePolicy
executeCommand
```

The absence of mutation methods in the client service is defense in depth.

The backend capability ceiling is the actual security boundary.

---

# 15. Service Worker Constitution

The PWA service worker exists for shell resilience and Web Push.

It MUST:

- precache the application shell;
- precache versioned static assets;
- provide an offline fallback;
- clean obsolete caches;
- use controlled update semantics;
- process supported Web Push.

It MUST NOT:

- implement a second ONYX database;
- cache authority as reusable permission;
- submit domain commands;
- queue domain mutations;
- reimplement CRDTs;
- resolve conflicts;
- upload files;
- create operational drafts that later auto-commit;
- claim durable Background Sync on iOS.

---

# 16. Cache Policy

The PWA MAY cache:

- application HTML shell;
- JS/CSS bundles;
- fonts;
- icons;
- non-sensitive static assets;
- an offline informational page.

The PWA MUST NOT treat cached operational data as authoritative.

By default it MUST NOT service-worker-cache:

- authentication responses;
- Mission projections;
- Task projections;
- hierarchy;
- authorization decisions;
- audit/evidence payloads;
- file contents;
- notification payloads containing sensitive operational details.

Previously rendered in-memory data MAY remain visible while offline, but MUST be marked as potentially stale.

---

# 17. Observer Capability Matrix

| Capability | `mobile_observer` |
|---|---:|
| Authenticate | YES |
| Refresh/logout session | YES |
| Query/read projections | YES |
| Read authorized notifications | YES |
| View audit/evidence where authorized | YES |
| Download authorized files | YES |
| Register/unregister Web Push | YES |
| Execute domain command | NO |
| Create Mission/Task | NO |
| Edit Mission/Task | NO |
| Approve/reject | NO |
| Lifecycle transition | NO |
| Resolve conflict | NO |
| Upload/delete file | NO |
| User/org/policy mutation | NO |
| Administrative command | NO |

The backend SHOULD implement this with an allowlist-oriented capability layer.

---

# 18. Read Authorization Still Matters

Read-only does not mean low-risk.

ONYX may contain sensitive operational intelligence.

Therefore the PWA MUST still enforce:

- authentication;
- tenant isolation;
- object-level read authorization;
- field-level data minimization where needed;
- file download authorization;
- audit/evidence authorization;
- session expiry;
- token revocation;
- HTTPS;
- XSS protection;
- CSP;
- service-worker integrity.

The observer client reduces integrity risk.

It does not eliminate confidentiality risk.

---

# 19. Files

Android MAY upload and download according to ONYX File-domain contracts.

The iOS PWA is **download/view only**.

The PWA MUST NOT upload, replace, delete, quarantine-release, or mutate file state.

Authorized download MAY be supported through an ONYX HTTP read contract.

Every download MUST be re-authorized.

Sensitive file contents SHOULD NOT be persisted in service-worker caches.

---

# 20. Notifications

Android MAY use native notification infrastructure and act on notifications when authorized.

The PWA MAY receive standards-based Web Push.

Push is informational.

A push notification MUST NOT itself mutate operational state.

Payloads SHOULD minimize sensitive operational content.

Notification interaction SHOULD open the authenticated Observer Client and fetch current authorized state.

The PWA MUST NOT depend on silent/data-only push or Background Sync.

---

# 21. Authority Is Never UI State

Hiding buttons is not authorization.

Every privileged action MUST still be denied when an observer session attempts it directly.

The server MUST deny forbidden mutation requests even if:

- the user is an administrator;
- JavaScript has been modified;
- the request was generated manually;
- the API is called outside the PWA;
- an old client version attempts it.

The client class itself imposes the capability ceiling.

---

# 22. Error Semantics

The PWA MUST distinguish at minimum:

- authentication failure;
- authorization/read denial;
- observer capability denial;
- not found;
- network unavailable;
- timeout;
- transient infrastructure failure;
- unsupported browser capability.

If a mutation is attempted through an observer session, the server SHOULD return a deterministic capability-denied response.

Android continues to preserve the richer operational error model including stale versions, lifecycle epochs, authority epochs, and conflicts.

---

# 23. Accessibility Constitution

Accessibility is part of acceptance.

Android MUST support TalkBack, semantic labels, logical traversal, adequate touch targets, text scaling, non-color-only status, and accessible error announcements.

The PWA MUST support semantic HTML, keyboard navigation, visible focus, screen readers, reduced motion, text enlargement, accessible Home Screen installation guidance, accessible notification onboarding, and clear communication of read-only status.

Accessibility regressions block release.

---

# 24. Verification Law

Compilation is not migration acceptance.

Evidence MUST exist at:

1. contract-test level;
2. implementation-test level;
3. integration-test level;
4. real-device/browser level.

For the PWA, one negative test family is mandatory:

> **Every prohibited operational mutation MUST remain prohibited when attempted directly against the backend using a valid `mobile_observer` session.**

---

# 25. Migration Sequence

The normative order is:

1. Freeze Flutter Android reference behavior.
2. Freeze Observer Client read model and capability matrix.
3. Establish native Android project.
4. Build JNI adapter.
5. Prove Kotlin ↔ JNI ↔ Rust execution.
6. Build Android startup/authentication.
7. Port Android screens.
8. Prove Android local/offline behavior.
9. Add backend `mobile_observer` capability class.
10. Build PWA Observer Client.
11. Add authorized file-download and Web Push observer contracts.
12. Prove Home Screen behavior on iPhone.
13. Produce production evidence.
14. Retire Flutter after Android acceptance.

---

# 26. Prohibited Patterns

The migration MUST NOT introduce:

- Kotlin domain reimplementation.
- JavaScript domain reimplementation.
- business rules in JNI.
- PWA domain-command APIs.
- PWA approval actions.
- PWA conflict resolution.
- PWA file uploads.
- PWA authority caches.
- PWA shadow synchronization.
- unsupported iOS Background Sync assumptions.
- client-side-only read-only enforcement.
- observer sessions that retain domain mutation rights.
- silent Android feature deletion.
- premature Flutter deletion.
- acceptance based solely on build status.

---

# 27. Normative Decision Ledger

| ID | Decision | Status |
|---|---|---|
| MOB-001 | Android = native Kotlin Operational Client | RATIFIED |
| MOB-002 | iOS = PWA Observer Client | RATIFIED |
| MOB-003 | No Kotlin Multiplatform | RATIFIED |
| MOB-004 | Flutter retained until Android parity | RATIFIED |
| MOB-005 | Jetpack Compose baseline | RATIFIED |
| MOB-006 | Android minSdk 29 | VERIFIED/RATIFIED |
| MOB-007 | JVM target 17 | VERIFIED/RATIFIED |
| MOB-008 | Thin JNI adapter required | RATIFIED |
| MOB-009 | mobile-core public behavior remains frozen | RATIFIED |
| MOB-010 | Android remains local-first operational client | RATIFIED |
| MOB-011 | PWA is operationally read-only | RATIFIED |
| MOB-012 | PWA uses `client_type="mobile_observer"` | RATIFIED |
| MOB-013 | Observer permissions are server-enforced | RATIFIED |
| MOB-014 | Observer client service exposes read operations only | RATIFIED |
| MOB-015 | PWA service worker caches shell, not authority | RATIFIED |
| MOB-016 | PWA files are download/view only | RATIFIED |
| MOB-017 | PWA push is informational | RATIFIED |
| MOB-018 | Background Sync is not an iOS dependency | RATIFIED |
| MOB-019 | Observer may mutate only client/session-control state | RATIFIED |
| MOB-020 | Flutter retirement is final Android migration increment | RATIFIED |

---

# 28. Inherited Debt Rule

The migration MUST distinguish migration defects from existing ONYX runtime limitations.

If current `mobile-core` lacks a production transport/auth adapter required for Android parity, that limitation must be tracked separately.

It MUST NOT be hidden inside Kotlin business logic.

The PWA Observer Client does not inherit Android local transport requirements because it is server-connected and read-only.

---

# 29. Exit Gate

This manifesto is complete when:

- Android is classified as Operational Client;
- iOS PWA is classified as Observer Client;
- `mobile_observer` capability ceiling is defined;
- server-side read-only enforcement is mandatory;
- operational vs client-control mutations are distinguished;
- observer client-service surface is explicitly read-only;
- files are download-only in PWA;
- push is informational;
- service worker remains non-authoritative;
- Android local-first behavior is preserved;
- Flutter retirement rules are fixed.

**Document status: COMPLETE — READY FOR ONYX-MOB-01 v1.1.**
