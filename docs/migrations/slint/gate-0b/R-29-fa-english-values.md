# R-29 — FA values byte-identical to EN (2026-11-23)

**Status:** evidence list for Persian review (D-01 / I18N-FA-01). No translation is claimed or
approved here. Method: parse `shared/i18n/en.json` and `shared/i18n/fa.json` and compare each value
by UTF-8 bytes. Both catalogs contain 565 keys with identical key sets. **23 keys are identical.**

The earlier snapshot (R-29) predicted "up to 10"; all 10 predicted keys are present, plus 13 more.

| # | Key | Identical value |
|---|---|---|
| 1 | `app.name` | `ONYX` |
| 2 | `auth.changePasswordBody` | `Your account requires a new password before you can continue.` |
| 3 | `auth.changePasswordSubmit` | `Change password` |
| 4 | `auth.changePasswordTitle` | `Change your password` |
| 5 | `auth.changingPassword` | `Changing password…` |
| 6 | `auth.currentPassword` | `Current password` |
| 7 | `auth.newPassword` | `New password` |
| 8 | `common.english` | `English` |
| 9 | `common.farsi` | `فارسی` |
| 10 | `common.hideDetails` | `Hide details` |
| 11 | `common.id16Placeholder` | `[12,34,...]` |
| 12 | `common.ruleType` | `Rule type` |
| 13 | `common.showDetails` | `Show details` |
| 14 | `files.downloadPathPlaceholder` | `/home/user/downloads/report.pdf` |
| 15 | `files.uploadPathPlaceholder` | `/home/user/documents/report.pdf` |
| 16 | `settings.addRule` | `Add rule` |
| 17 | `settings.concurrencyAuthorityEpochRequired` | `Required authority version is missing. Refresh and try again.` |
| 18 | `settings.concurrencyEpochMismatch` | `This record has changed. Refresh it before continuing.` |
| 19 | `settings.concurrencyLifecycleEpochRequired` | `Required lifecycle version is missing. Refresh and try again.` |
| 20 | `settings.concurrencyVersionConflict` | `The record was updated by another operation. Review the latest version and retry.` |
| 21 | `settings.createVersion` | `Create version` |
| 22 | `settings.removeRule` | `Remove rule` |
| 23 | `settings.rulesPending` | `Rules pending` |

Notes for the reviewer:

- `app.name` (`ONYX`), `common.english`, `common.farsi`, the path placeholders and the ID
  placeholder may be intentionally identical; mark them as accepted if so.
- Keys 10, 13, 22 were introduced by S5 and keys 17–20 by S5's concurrency work; they are English
  placeholders awaiting approved Persian (D-01 / D-03).
- Do not change `fa.json` without a recorded Persian reviewer, catalog commit and corrections list
  (I18N-FA-01 exit evidence).
