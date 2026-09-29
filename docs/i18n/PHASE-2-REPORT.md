# ONYX i18n Phase 2 Report

## 1. Executive Summary

Phase 2 hardened the canonical translation catalog foundation. It audited all
150 canonical keys in both locales, corrected eight concrete catalog defects,
established deterministic key ordering, added a small terminology model, and
extended the existing canonical validator with five new catalog invariants that
the standalone `i18n` CI job now enforces.

No application string was migrated, no Android or Rust code was touched, no
visual/RTL/typography work was done, and no language was added. Exactly one
application file changed, and only to update a single call site for a key rename
that was required to repair a demonstrably referenced key.

All executable verification ran through GitHub Actions. The localization gate
(`i18n`) passed on every run, and all seven jobs passed on the branch tip
(run 36629491659). One earlier run failed only on the unrelated k6 `load-smoke`
performance threshold and is documented precisely in §8.

Branch: `feat/i18n-phase2-catalog-foundation`
PR: <https://github.com/SMozaff/Onyx-Framework/pull/20>

## 2. Catalog Baseline

| Metric | Before | After |
| --- | --- | --- |
| Locales | 2 (`en`, `fa`) | 2 (`en`, `fa`) — unchanged |
| Keys per locale | 150 | 150 |
| Keys total (both locales) | 300 | 300 |
| Namespaces | 14 | 14 — unchanged |
| Interpolation-bearing keys | 2 | 2 |
| Count-bearing keys | 2 | 2 |
| Keys using plural categories | 0 | 0 |
| Terminology concepts | 0 | 16 |
| Generated dictionaries | 4 | 4 — regenerated |
| CI catalog invariants | 5 | 10 |

Key count is unchanged by design. Phase 2 repaired meaning, ordering, and
enforcement; it deliberately added no speculative vocabulary (see §11).

The 2 interpolation-bearing keys are `approvals.pendingCount` (`{count}`) and
`dashboard.blockedAlertsNeedReview` (`{blocked}`, `{unread}`). Both are
count-bearing and both are now free of hard-coded plural forms.

## 3. Semantic Key Audit

### Renamed keys

| Before | After | Justification |
| --- | --- | --- |
| `dashboard.unreadAlerts` | `dashboard.unreadNotifications` | The key claimed "alerts", but its data field is `unread_notifications` and its sibling string `dashboard.blockedAlertsNeedReview` already described the same value as "unread notifications". The name contradicted the meaning it represented. |

This is the only rename. It required exactly one call-site update
(`web-ui/src/pages/Dashboard/components/StatsGrid.tsx`), which is the
"demonstrably referenced" repair permitted by P2-16 and is not a migration.

### Retained keys — duplicate semantics

Eight English values were byte-identical across two keys each:

```text
nav.missions      / missions.title
nav.tasks         / tasks.title
nav.notifications / notifications.title
nav.approvals     / approvals.title
nav.reports       / reports.title
nav.files         / files.title
nav.settings      / settings.title
nav.dashboard     / dashboard.title
```

These were **retained**, not consolidated. `nav.*` is the navigation-chrome
label for an entity; `<namespace>.title` is the heading of the page that entity
owns. P2-3 requires keeping distinct keys when context genuinely changes
meaning, and navigation chrome versus page heading is a real context
distinction. Consolidating would also have required deleting keys referenced
across multiple surfaces, which is application migration and therefore out of
scope.

Two further near-duplicates were retained with the same reasoning:

- `common.loading` ("Loading…") versus `status.loading` ("Loading"). One is a
  transient progress state with an ellipsis, the other is a categorical data
  state. Different meanings, deliberately different wording.
- `missions.noMissions` ("No missions") versus `missions.noMissionsYet`
  ("No missions yet"). Empty-after-use versus first-run empty state. Retained;
  consolidating would remove a state distinction for no benefit.

### Ambiguous and generic keys — reviewed, retained

The audit found no key in the defective shapes P2-4 calls out (`common.text`,
`common.label`, `common.value`, `app.message`, `nav.item1`, `page.title`,
`modal.message`). The generic-looking field labels `common.name`,
`common.title`, `common.description`, `common.status`, `common.owner`,
`common.version`, and `common.updated` are reusable field labels, not
UI-geometry keys, and P2-1 forbids renaming on preference alone. Retained.

Two keys have names slightly broader than their values and were retained:

- `dashboard.blockedAlertsNeedReview` covers blocked tasks *and* unread
  notifications.
- `dashboard.noBlockedAlerts` covers the same two conditions.

Both are referenced by live components. Their names are imprecise rather than
defective, and P2-16 restricts reference edits to demonstrated defects. The
imprecision is recorded in §11 instead.

### Machine-safe boundaries (P2-6)

A significant finding: `dashboard.summary`, `mission.list`, `approval.list`, and
`notification.list` are **query keys and cache keys**, not translations, and
they collide exactly with the `namespace.leaf` translation-key shape. All four
are passed to `useOnyxQuery` / `invalidateQueries` in `web-ui` and
`mobile-pwa`.

No machine identifier was added to a catalog, none was renamed, and no API
path, JSON field, event name, or wire value was touched. The collision is now
recorded in `terminology.json` under `reservedMachineIdentifiers` and in the
README, and it directly shaped a tooling decision: unresolved-key detection
stays scoped to translation call arguments, because a scan that flagged every
key-shaped string literal would report these machine identifiers as missing
translations and fail CI incorrectly.

### Phase 1 decisions (P2-12)

`missions.reviewApproval` and `common.total_items` were re-examined for defect
evidence and found sound. `missions.reviewApproval` names the concept it
represents; `common.total_items` is an explicit Phase 1 requirement whose
schema exception remains narrow and documented. Neither was reopened.

## 4. English Catalog

`en.json` is the canonical source language. Corrections:

| Key | Before | After | Reason |
| --- | --- | --- | --- |
| `common.readOnly` | "View only" | "Read-only" | Contradicted its own siblings `common.readOnlyScope` ("Read-only web scope") and `common.readOnlyProjection` ("Read-only projection"). One concept, three different English terms. |
| `dashboard.blockedAlertsNeedReview` | "{blocked} blocked task(s) and {unread} unread notification(s) need review." | "Needs review — blocked tasks: {blocked}, unread notifications: {unread}." | Removed a hard-coded plural hack; see §7. Also aligned "notification" with the key's own data field. |
| `dashboard.unreadAlerts` → `dashboard.unreadNotifications` | "Unread alerts" | "Unread notifications" | Key/data/terminology alignment. |
| `language.title` | "Language / زبان" | "Language" | The English catalog was not monolingual. A bilingual string duplicated into both catalogs defeats the source/target distinction. |

Terminology review found the catalog already coherent on its core nouns:
mission, task, report, file, approval, notification, and organization each use a
single stable English term. Verb/adjective pairs are correctly separated
(`common.approve` "Approve" versus `status.approved` "Approved";
`common.activate` "Activate" versus `status.active` "Active").

No stylistic rewrite was performed. The catalog remains UI/product copy, and no
marketing copy was introduced.

## 5. Farsi Catalog

The Farsi catalog was audited for semantic agreement, natural phrasing, and
terminology consistency. Corrections:

| Key | Before | After | Reason |
| --- | --- | --- | --- |
| `nav.staffLoans` | مأموریت‌های پرسنل ("staff **missions**") | واگذاری پرسنل | Domain fact. `todo-domain` documents `StaffLoan` as a secondment of a staff member to another manager in the chain of authority, approved or declined by the real owner. The old value was not merely awkward, it asserted the wrong domain concept and collided with Mission terminology. |
| `status.unacknowledged` | تأیید نشده ("not approved") | تأیید دریافت نشده | Semantic collision. The old value meant "not approved", which is indistinguishable from rejection, while the key means an unseen notification. `status.acknowledged` already read تأیید دریافت شده, so the pair is now consistent. |
| `common.refresh` | به‌روزرسانی ("update") | تازه‌سازی | The value meant "update" and duplicated `common.updated` / `common.recentlyUpdated`. The key says refresh. |
| `common.readOnly` | فقط مشاهده ("view only") | فقط‌خواندنی | Matched the English correction and its two Farsi siblings. |
| `common.search` | جستجو | جست‌وجو | Codified Persian spelling. |
| `language.title` | Language / زبان | زبان | Farsi catalog made monolingual. |
| `dashboard.blockedAlertsNeedReview` | {blocked} وظیفه مسدود و {unread} اعلان خوانده‌نشده نیازمند بررسی است. | نیازمند بررسی — وظایف مسدود: {blocked}، اعلان‌های خوانده‌نشده: {unread} | Restructured to mirror the English and to read naturally in an RTL UI, while keeping the same placeholders. |
| `dashboard.unreadAlerts` → `dashboard.unreadNotifications` | هشدارهای خوانده‌نشده | اعلان‌های خوانده‌نشده | Alert (هشدار) and notification (اعلان) are distinct concepts elsewhere in this catalog; this key holds a notification count. |

Deliberately **not** changed: `nav.todos` ("کارها و اهداف"). The Farsi term
کار overlaps conceptually with وظیفه (task), but `TodoList`/`TargetList` is a
genuinely separate domain aggregate, and resolving the collision is a product
terminology decision, not a catalog defect. See §11.

No literal word-for-word retranslation was performed. Typography, fonts, and
visual RTL remain untouched.

## 6. Interpolation

Both interpolation-bearing keys were audited for variable name, presence, count,
and shape.

- `approvals.pendingCount` — `{count}` in both locales, identical name and
  count. No correction needed.
- `dashboard.blockedAlertsNeedReview` — `{blocked}` and `{unread}` in both
  locales, identical names and count. Placeholders were preserved through the
  English rewrite, so the existing call site in
  `web-ui/src/pages/Dashboard/components/AlertBanner.tsx` required no change.
  Placeholder identifiers remain machine-facing camelCase and were not
  translated.

**Gap found and closed in CI.** Placeholder *parity* cannot detect a
placeholder that is malformed in both locales identically. The validator now
also checks brace syntax and placeholder identifier shape per locale, so
`{count }`, `{2 items}`, `{na-me}`, and unbalanced braces fail even when both
sides agree on the defect.

## 7. Pluralization

Phase 1 established `pluralCategory` / `selectPluralForm`. The catalog was
audited for compatibility with that API.

One hard-coded plural violation existed:

```text
"{blocked} blocked task(s) and {unread} unread notification(s) need review."
```

The `(s)` form is exactly the pattern P2-10 prohibits: it encodes a
singular/plural decision in the string instead of leaving it to the
pluralization API. It was replaced with a plural-neutral construction that is
grammatical for any count in English, and with an invariant Persian noun
phrase for the Farsi side.

The result uses no `count === 1`, no `count > 1`, and no component-specific
plural rule. `approvals.pendingCount` ("{count} pending" / "{count} در انتظار")
was already plural-safe: "pending" and "در انتظار" are invariant.

**New CI invariant.** `--check` now rejects `(s)`, `(es)`, `{count}s`, and
`if count` inside catalog values, so a plural hack cannot be reintroduced.

No application plural string was migrated; the three hard-coded English plural
sites identified in Phase 1 remain for a later phase, as instructed.

## 8. CI

**GitHub Actions workflow:** CI
**Run ID:** 36629491659 (branch tip)
**Commit:** 4e6460b
**PR:** #20

| Job | Result |
| --- | --- |
| `i18n` | PASS |
| `check` | PASS |
| `web` | PASS |
| `mobile-android-kotlin` | PASS |
| `deploy-check` | PASS |
| `load-smoke` | PASS |
| `native-ui-evidence` | PASS |

Run conclusion: **success**. All seven jobs in the workflow ran; none were
skipped.

### The one intermittent failure seen during Phase 2: `load-smoke`

Run 36627197624 concluded `failure` solely because `load-smoke` failed. The
k6 step reported:

```text
level=error msg="thresholds on metrics 'command_latency' have been crossed"
http_req_failed................: 0.04% 10 out of 23612
http_req_duration..............: avg=255.27ms p(95)=598.3ms
Process completed with exit code 99
```

That run's commit changed exactly one file,
`docs/i18n/PHASE-2-REPORT.md`. The runs on either side of it — 36624890985
before, 36629491659 after — both concluded `success` with `load-smoke` green,
and all three runs build byte-identical catalog and generated-dictionary
content, since none of them changed a catalog, the generator, the schema, the
terminology model, or any application code.

Classification: an unrelated, pre-existing instability in a performance
threshold, not a Phase 2 regression. It is reported rather than dismissed, and
it was deliberately not "fixed": relaxing a k6 threshold or editing the load
test to force it green would be an out-of-scope change to an unrelated system
and would hide a real signal. Three runs with identical build input produced
two passes and one failure, which is the definition of a flake.

### All runs on this branch

| Run | Commit | Conclusion | `i18n` | `load-smoke` |
| --- | --- | --- | --- | --- |
| 36622328490 | ed53f74 | success | pass | pass |
| 36624655634 | c0d3923 | success | pass | pass |
| 36624890985 | 01e031e | success | pass | pass |
| 36627197624 | 914bd3c | failure | pass | **fail** |
| 36629491659 | 4e6460b | success | pass | pass |

The `i18n` job passed in every run, including the one where `load-smoke` failed.
It declares no `needs:` and is therefore independent of every other job.

Any commit made after this report is documentation-only and cannot weaken a
catalog invariant, because every catalog guarantee is enforced by the `i18n`
job on the commit that introduces it.

### `i18n` job on the branch tip

```text
1  success  Set up job
2  success  Run actions/checkout@v7
3  success  Run actions/setup-node@v7
4  success  Verify catalogs, schema, parity, interpolation, ordering, pluralization, terminology, unresolved keys, and determinism
5  success  Audit user-facing literals in enforced surfaces
6  success  Unit-test the canonical formatting and pluralization API
```

Validator output:

```text
i18n unresolved-key scan passed: 55 literal keys in 134 source files.
i18n validation passed: schema, EN/FA parity, interpolation parity, canonical ordering, pluralization structure, terminology, and unresolved-key scan (150 keys).
i18n in sync.
i18n literal audit passed: web-ui/src/pages/Dashboard
```

The `i18n` job remains free of `needs:` and therefore independent of the other
jobs; it was green in every run listed above, including the run in which
`load-smoke` failed.

### New CI invariants

The existing `sync-i18n.mjs` validation was extended rather than duplicated, so
the standalone job picks the checks up with no workflow restructuring. No second
competing localization validation system was created.

| Check | Defect class it now catches |
| --- | --- |
| canonical ordering per catalog | editor- or merge-dependent key order, non-reproducible diffs |
| unknown namespaces | a key using a namespace outside the ratified 14 |
| interpolation brace syntax and identifier shape | a placeholder malformed in *both* locales |
| pluralization structure | `task(s)`-style hard-coded plural forms |
| terminology-model agreement | `terminology.json` drifting away from the catalogs |

Ordering is compared with an explicit locale-independent comparator, not
`localeCompare`, so it cannot drift with the runner's ICU build. A
`--sort-catalogs` mode normalizes ordering mechanically so the invariant does
not depend on a human hand-sorting 150 lines.

The checks are demonstrably not no-ops. During authoring, the ordering check
caught a real misordering introduced while hand-editing (`operatorQueue` placed
before `operationsStable`), and the terminology check caught a concept term that
did not exist in the catalog.

### Phase 1 protections preserved (P2-20)

- The sanctioned locale persistence write
  `localStorage.setItem(LOCALE_STORAGE_KEY, locale)` is untouched. No new
  browser storage write was introduced anywhere. Phase 2 added no storage code
  at all.
- `mobile-android/app/src/main/kotlin/com/onyx/util/LocaleHelper.kt` was not
  modified. `mobile-android/` has no Phase 2 diff.
- The literal-audit scope remains exactly
  `web-ui/src/pages/Dashboard`; it was not widened.
- No frozen contract was modified: `docs/api/openapi.json`, `ShellError`,
  `NotificationAggregate`, and the notification wire schema are all unchanged.

Note on the baseline: commit `b226001` ("fix(ci): restore green web and Android
checks"), landed on `main` before this branch, resolved the two blockers
carried from Phase 1 — the `web` feature-scope audit now recognizes the
sanctioned locale write, and the Android `LocaleHelper` Kotlin compilation was
fixed in `3a575e2`. As a result `web` and `mobile-android-kotlin` pass, and
`native-ui-evidence` now executes instead of being skipped behind a failing
`web` job. Neither change was made or required by Phase 2.

## 9. Files Changed

Branch `feat/i18n-phase2-catalog-foundation`, six commits.

Canonical catalog:

```text
shared/i18n/en.json
shared/i18n/fa.json
shared/i18n/metadata/terminology.json          (new)
```

Generator and CI:

```text
scripts/sync-i18n.mjs
.github/workflows/ci.yml                        (step label only)
```

Generated outputs, produced by `node scripts/sync-i18n.mjs` and never hand-edited:

```text
web-ui/src/i18n/dictionaries.generated.ts
mobile-pwa/src/i18n/dictionaries.generated.ts
crates/bins/desktop-shell/ui/src/i18n/dictionaries.generated.ts
crates/bins/admin-shell/ui/src/i18n/dictionaries.generated.ts
```

Documentation:

```text
shared/i18n/README.md
```

The single application file, for the key rename only:

```text
web-ui/src/pages/Dashboard/components/StatsGrid.tsx
```

Explicitly unchanged: `mobile-pwa` application code, both Rust shell
applications' application code, all `crates/domains/**` and
`crates/applications/**`, `mobile-android/**`, every CSS file, every font
asset, `docs/api/openapi.json`, and `web-ui/tests/**`.

Regeneration was required because catalog content and ordering changed, and was
performed with the canonical generator. `--check` reports the generated
dictionaries as in sync.

## 10. Out-of-Scope Items

The following were explicitly not performed in Phase 2:

- **No broad application migration.** No user-facing string in `web-ui`,
  `mobile-pwa`, `desktop-shell`, `admin-shell`, Android, or Rust/native was
  migrated to translation keys. The only application edit in the whole phase is
  one lookup-table entry updated because its key was renamed.
- **No Android or Rust migration.** No `stringResource` migration, no Rust
  presentation localization, no Rust error-architecture change, no API error
  wire-format change, no notification wire-schema change, and no Android
  resource generation.
- **No visual, RTL, or typography work.** No typography, font, CSS direction,
  RTL layout, icon mirroring, spacing, visual hierarchy, responsive behaviour,
  page design, or component styling change. `rtl.css` was not modified.
- **No new languages.** `en` and `fa` only. The registry still declares exactly
  one default locale and two entries.
- **No catalog expansion.** No new namespaces, no new status keys, and no
  speculative vocabulary were added.
- **No second validation system, and no translation-management system.** The
  terminology model is a 16-concept reference with one CI check.
- **No unrelated cleanup, refactor, dependency change, or architecture
  redesign.**

## 11. Remaining Issues

Discovered, deliberately deferred, and not defects introduced by this phase:

1. **Indirectly referenced keys escape the static scan.** A key used through a
   `const` table passed to `t(label)` — precisely
   `dashboard.unreadNotifications` in `StatsGrid.tsx` — is invisible to the
   literal-argument scan. This is how a genuinely referenced key nearly looked
   orphaned, and it is a real gap in unresolved-key detection. Widening the
   scan to all key-shaped string literals is unsafe because of the machine
   identifiers in §3. A safe widening needs a call-graph-aware approach and
   belongs to a later phase. Documented in the README.
2. **104 of 150 keys are not referenced by any React surface today.** This is
   expected: the catalog was seeded ahead of migration. It is recorded as an
   inventory fact, not a defect, and no keys were deleted on that basis.
3. **`projection` is transliterated in Farsi.** `missions.noMissionsBody`,
   `reports.noReport`, and `common.readOnlyProjection` render the domain term
   as پروژکشن. The English side is a real domain term, so inventing a Persian
   equivalent unilaterally risked semantic drift. Deferred pending a product
   terminology ruling.
4. **`nav.todos` Farsi terminology overlap.** کارها overlaps with وظیفه
   (task). `TodoList`/`TargetList` is a separate aggregate, so consolidating is
   a product decision, not a catalog defect.
5. **`status.*` is a cross-entity subset.** The domain `MissionStatus` also has
   `Draft`, `Planning`, `Halted`, `Review`, `Closed`, `Archived`, and
   `Cancelled`, none of which have `status.*` keys. They were deliberately not
   added: they are mission-specific, and per P2-2 they belong in the missions
   namespace (for example `missions.draft`) rather than being pushed into a
   shared `status` namespace that would imply a universal status model the
   domain does not have. Adding them needs a consumer, not just vocabulary.
6. **`auth.sessionNote` embeds the version claim "in v1".** Version-specific
   copy ages badly in a canonical catalog, but changing user-facing product
   wording is not a consistency defect and was left alone.
7. **`action` is not a terminology concept.** P2-9 lists it among example
   concepts. The catalog has no domain noun for "action" — actions are verbs
   under `common.*` — so a canonical term would have no semantic referent and
   would have to be invented. Action vocabulary is instead audited in this
   report under P2-11. `user` and the other named concepts are in the model.

No localization-related blocker is currently open. The Phase 1 blockers B-1 and
B-2 were resolved on `main` before this branch and both jobs are green.

8. **`load-smoke` is non-deterministic.** The k6 `command_latency` threshold
   was crossed on run 36627197624 and not on run 36624890985, for identical
   build input. The smoke test is the only unstable job observed in Phase 2.
   Left for the owning phase; loosening the threshold to force green would
   hide a real signal.

## 12. Final Status

**PHASE 2 COMPLETE**

Every applicable Definition-of-Done condition is satisfied: both catalogs
audited; key semantics, duplicate semantics, and naming reviewed; interpolation
parity and syntax verified; pluralization compatibility enforced; terminology
consistency established and CI-checked; deterministic ordering established and
CI-enforced; generated outputs regenerated deterministically through the
canonical generator; CI validation extended to cover the Phase 2 invariants
within the existing standalone job; no application, Android/Rust, or
visual/RTL/typography migration performed; no new language introduced; Phase 1
CI protections preserved; GitHub Actions verification completed with all seven
jobs passing on the branch tip (run 36629491659, commit 4e6460b).

The one exception encountered during the phase — the intermittent k6
`load-smoke` `command_latency` threshold on run 36627197624 — is unrelated to
localization, is documented with evidence in §8, and is left visible rather
than masked. It does not gate any Phase 2 invariant, the `i18n` job passed in
that same run, and `load-smoke` is not owned by this phase.

No Phase 2 contract item is unimplemented.

Working tree is clean and fully pushed. PR #20 is open against `main` and is
not merged; merging is left to the project owner.

Work has stopped at the end of Phase 2. Phase 3 was not started.
