# Creation report (Excel) column spec: draft

**Status:** Draft for owner review, 2026-11-01. Not implemented. Source rules: `docs/product/staff-and-team-rules.md` ("Creation report") and `docs/migrations/slint/backend-accounts-handover.md` rule 6.
**Blocked by:** O-1 (hashing), O-3 (Onyx ID), O-4 (forced change, which sets the account status values), O-5 (blank `onyx_id`).

## Scope

- One `.xlsx` file per batch import and one per single admin creation.
- Visible to Admin only (same as staff import and export).
- Contains personal data and a one-time password for each new account. Treat the file as a secret: it is not stored by the Worker, not logged, and not sent by email or chat.

## Columns (draft order)

| # | Column | Source | Notes |
|---|---|---|---|
| 1 | `onyx_id` | Assigned by the system | Staff ID, e.g. `st26sm0035`. Format per `docs/product/staff-and-team-rules.md`. |
| 2 | `first_name` | Staff sheet | English. |
| 3 | `last_name` | Staff sheet | English. |
| 4 | `first_name_fa` | Staff sheet | Optional. Persian or Arabic script. |
| 5 | `last_name_fa` | Staff sheet | Optional. Persian or Arabic script. |
| 6 | `national_id` | Staff sheet | Iran only, 10 digits. Text cell, to keep leading zeros. |
| 7 | `contact_email` | Staff sheet | Optional in the sheet. See the email note in the current-state doc. |
| 8 | `contact_phone` | Staff sheet | Optional. Text cell. |
| 9 | `job_title` | Staff sheet | |
| 10 | `department` | Staff sheet | |
| 11 | `class_label` | Staff sheet | |
| 12 | `parent_onyx_id` | Staff sheet | Manager's Onyx ID. |
| 13 | `team_ids` | Staff sheet | `;`-separated team IDs (`tm...`). |
| 14 | `account_status` | System | Values not yet defined. **Pending O-4.** |
| 15 | `one_time_password` | System, once | Plain text, shown only in this report. **Pending O-1 and O-4 for how and when it is generated.** Never written to logs, D1, or audit rows. |

`date_of_birth` is dropped and is not a column.

## Rules for the writer (when implemented)

1. The password column is filled from the generator at the moment of creation. Only the hash is persisted.
2. Each cell that begins with `=`, `+`, `-` or `@` is written as text, to block spreadsheet formula injection. This applies to every text column, including names entered by admins.
3. Columns 1 to 13 come from the same row object as the import result. They are not re-read from D1 after creation.
4. A failed row is included with `account_status` set to a failure value and an empty password cell. A failed row never has a password.
5. The report is returned as the response to the admin request. It is not written to R2, D1, or logs.

## Open points for the owner

- Exact `account_status` values (O-4).
- Whether a failed row appears in the same file as created rows, or in a separate sheet (default: same file, one sheet).
- Delivery and retention of a file that contains plain passwords (the owner has not set this).


## Dated decision reconciliation — 2026-11-10

The status block and “Blocked by” list above are a 2026-11-01 draft snapshot. O-1 (Argon2id hashing) and O-4 (forced password-change flag and route restrictions) were recorded by the coordinator under the owner's standing “decide and document” authority on 2026-11-01. They are not unresolved owner questions and must not be used to ask the owner to decide hashing or the forced-change mechanism again. Their delegated attribution remains RECORDED; the owner may override them through a new decision-log entry.

The owner separately decided on 2026-11-01 that each batch import and each single staff creation produces an Excel (.xlsx) report, containing the staff-sheet fields plus the assigned `onyx_id`, one-time password and account status. These are approved report-format/content requirements. The column ordering in the table above remains a draft presentation order, not an independently approved ordering.

Still unresolved and not to be invented:
- Exact `account_status` values.
- Whether failed rows share the same sheet/file as successful rows or use a separate sheet/file.
- How the administrator securely receives and retains a file containing plaintext one-time passwords, and the retention/deletion policy.

The existing source contract describes the report as the creation response and the only place the one-time password is returned. Verify the current Worker implementation/tests at the exact source revision; this does not establish deployed behavior. Formula-injection protection and no logging/no persistence requirements remain mandatory.

See the [decision log](decision-log.md), [staff and team rules](../../product/staff-and-team-rules.md), and [Slint Migration Master Record](MASTER-MIGRATION-RECORD.md) for current authority and blocker status.


## Source implementation reconciliation — 2026-11-10

The Worker source and `test/backend-accounts.test.ts` now implement and cover the report behavior. The older “Open points” above are partly superseded:

- **Status values are implemented:** successful rows use `created`; failed rows use `failed: <CODE>` (including `INVALID_STAFF_ROW`, `PARENT_NOT_FOUND`, and account conflict codes).
- **Failed rows share the same report worksheet:** they have an empty `onyx_id` and empty `one_time_password`; a failed row creates no account/profile and consumes no ID number.
- **The actual implemented workbook has one worksheet** and 17 columns: the owner-required staff fields, `account_status`, `one_time_password`, plus implementation fields `source_row` and `row_notes`. The exact ordering and the two additional implementation fields were not separately owner-approved.
- **Delivery is implemented as the authenticated Worker response:** an `.xlsx` attachment with `Content-Disposition`, `Cache-Control: no-store`, and no Worker-side storage in D1/R2/logs. The one-time password is returned only in this report. Tests cover no database persistence, no password/hash logging, formula-injection protection and report generation for both batch and single creation.
- The account-status and failed-row-layout questions are therefore not open owner decisions. Do not ask them again unless new evidence contradicts the current source/tests.

**Still unresolved:** the post-download retention/deletion policy for a plaintext-password report on the receiving Admin device, and whether the owner wants to approve the exact 17-column order/additional `source_row`/`row_notes` fields. The Worker response uses `no-store`; that does not define what the client does with a downloaded file.

Implementation evidence: `deploy/cloudflare/edge-gateway/src/index.ts` (`creationReport`, `reportResponse`, `importProfiles`), `src/xlsx.ts`, and `test/backend-accounts.test.ts`. The test file exists in the repository; this audit did not independently execute it. The API contract is source-level evidence, not proof that the migration has been applied to production D1 or that the Worker revision is deployed.
