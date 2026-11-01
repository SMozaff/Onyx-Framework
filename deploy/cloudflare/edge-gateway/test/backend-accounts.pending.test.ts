/**
 * Pending contract tests for the backend accounts lane.
 *
 * These pin the owner rules in docs/migrations/slint/backend-accounts-handover.md
 * as todo items. Each is marked with the open item (O-1 to O-6) that blocks it,
 * or "unblocked" when no open item applies. Convert a todo to a real test when
 * its blocking item is decided and the implementation lands.
 *
 * Fixtures must be generated. No real names, emails, passwords or tokens.
 */
import { describe, it } from "vitest";

describe("backend accounts: import creates accounts (rule 1)", () => {
  it.todo("[BLOCKED O-6] batch import creates one account per staff row");
  it.todo("[BLOCKED O-5] a row with a blank onyx_id is rejected or creates account and ID together");
  it.todo("[unblocked] a failed row leaves no account, no profile and no ID gap");
});

describe("backend accounts: single creation follows the same rule (rule 1)", () => {
  it.todo("[unblocked] POST /api/admin/users creates an account and assigns a staff ID");
});

describe("backend accounts: sign-in with Onyx ID (rule 2)", () => {
  it.todo("[BLOCKED O-3] login accepts the Onyx ID with the one-time password");
  it.todo("[BLOCKED O-3] login with username or email is rejected or kept as an alias, as decided");
});

describe("backend accounts: one-time password (rule 3)", () => {
  it.todo("[BLOCKED O-1] the generated password meets the decided policy length and character rules");
  it.todo("[unblocked] every new account gets a distinct generated password");
});

describe("backend accounts: forced change at first login (rule 4)", () => {
  it.todo("[BLOCKED O-4] a new account cannot call any route except password change until the password is changed");
  it.todo("[BLOCKED O-4] after a password change the forced-change flag is cleared and other routes open");
});

describe("backend accounts: password handling (rule 5)", () => {
  it.todo("[BLOCKED O-1] only a hash is stored for each account");
  it.todo("[unblocked] no response body from import, create or login contains a password, other than the creation report");
  it.todo("[unblocked] no log line from import, create or login contains a password or a hash");
  it.todo("[unblocked] import and create request bodies are not echoed in errors");
});

describe("backend accounts: creation report (rule 6)", () => {
  it.todo("[BLOCKED O-4] the report has the columns in docs/migrations/slint/backend-accounts-creation-report-spec.md");
  it.todo("[unblocked] the report is produced after every batch import");
  it.todo("[unblocked] the report is produced after every single creation");
  it.todo("[unblocked] text cells starting with = + - @ are written as text");
});

describe("backend accounts: IDs (rule 7)", () => {
  it.todo("[BLOCKED O-2] public_ids and id_sequences exist in migrations/ only after owner confirmation");
  it.todo("[unblocked] staff IDs are unique per organization and year, and never reused after removal");
  it.todo("[unblocked] concurrent allocations in one organization and year produce distinct, gapless numbers");
});
