# Staff import, IDs, and team rules

**Status:** Owner-confirmed decisions, 2026-10-30. Open items are marked **OPEN**.

## Staff import (one Excel sheet, one row per person)

| Column | Required | Notes |
|---|---|---|
| `onyx_id` | yes | System-generated (see IDs). Existing `user_id`. |
| `first_name` | yes | English. Split from `full_name`. |
| `last_name` | yes | English. |
| `first_name_fa` | no | Persian/Arabic (Farsi in Iran). |
| `last_name_fa` | no | Persian/Arabic (Farsi in Iran). |
| `national_id` | yes | Iran only: exactly 10 digits. Digits only for now. |
| `contact_email` | | |
| `contact_phone` | | |
| `job_title` | | Same as "position". |
| `department` | | |
| `class_label` | | |
| `parent_onyx_id` | | Manager's Onyx ID. |
| `team_ids` | | `;`-separated team IDs (`tm…`). |

Dropped: `date_of_birth`.

**Visibility:** Import and export of staff personal data: Admin only.

## Staff ID

- Format example: `st26sm0035` = `st` (staff) + `26` (year) + `sm` (initials of first and last name) + `0035` (number).
- The number is counted per year, starts at 1 each year, and is never reused.
- Initials and full names may be shown across the organization.
- Number is always 4 digits.
- Names always have two main parts, so initials are always two letters (first name, last name).
- Initials come from the English names.

## Team ID

- Format: `tm` + creation date as `YYMMDD` + team leader initials (two letters) + creation order as 4 digits, counted per year.
- Example: `tm261030sm0007` = team created 2026-10-30, leader initials `sm`, 7th team created in 2026.
- A team leader change updates the system but not the ID.
- The ID changes only if the team is removed and recreated. Order numbers are never reused.

## Roles and team management

- **All-Father:** Absolute access, no boundaries.
- **Admin (ORGANIZATION_ADMIN):** Create, modify, and remove teams and members at any level.
- **Top-level manager, senior manager:** Create, modify, and remove teams and members only beneath their own authority. Cannot change another manager's teams.
- **Team leader:** Required on every team a manager creates.
- **Supervisor:** Optional on a team. Same rank as staff, below team leader. Informal, verbal authority only. No real authority and no special access.
- **Parent:** The manager who created the team is its parent. Every team has a parent manager.

## Deferred

- **Employee board** (organization-wide search of names, phones, emails, managers): a separate, modular feature to be added later. Not part of the current design.

## Accounts and sign-in (owner decision, 2026-11-01)

- Every staff row in an import creates an account, and the import assigns that person's staff ID.
- Every staff member must have an account. Single creation by an admin follows the same rule.
- Staff sign in with their Onyx ID.
- The system generates a one-time password for each new account.
- Staff must change their password after the first login.
- Passwords are never stored, logged, prefilled, or requested in chat.
- This replaces the 2026-08-13 rule that import does not create accounts.
