# Staff import, IDs, and team rules

**Status:** Owner-confirmed decisions, 2026-10-30. Open items are marked **OPEN**.

## Staff import (one Excel sheet, one row per person)

| Column | Required | Notes |
|---|---|---|
| `onyx_id` | yes | System-generated (see IDs). Existing `user_id`. |
| `first_name` | yes | Split from `full_name`. |
| `last_name` | yes | Split from `full_name`. |
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
- **OPEN:** Number width (4 digits assumed from the example); initials rule when names have more than two parts.

## Team ID

- Format: `tm` + year + creation order + creation date + team leader data at creation.
- A team leader change updates the system but not the ID.
- The ID changes only if the team is removed and recreated.
- **OPEN:** Exact layout of the segments (example needed).

## Roles and team management

- **All-Father:** Absolute access, no boundaries.
- **Admin (ORGANIZATION_ADMIN):** Create, modify, and remove teams and members at any level.
- **Top-level manager, senior manager:** Create, modify, and remove teams and members only beneath their own authority. Cannot change another manager's teams.
- **Team leader:** Required on every team a manager creates.
- **Supervisor:** Optional on a team. Same rank as staff, below team leader. Informal, verbal authority only. No real authority and no special access.
- **Parent:** The manager who created the team is its parent. Every team has a parent manager.

## Not yet defined

- **Employee board** (organization-wide search of names, phones, emails, managers): not established.
