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
