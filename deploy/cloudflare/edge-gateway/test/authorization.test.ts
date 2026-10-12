import { describe, expect, it } from "vitest";
import {
  authorizeCommand,
  authorizeQuery,
  COMMAND_POLICIES,
  ONYX_ROLES,
  resourceOwnedBy,
} from "../src/authorization";
import { COMMAND_SEMANTICS, validateCommandSemantics } from "../src/command-semantics";

describe("explicit command authorization matrix", () => {
  it("has an explicit result for every role × registered action", () => {
    for (const [action, policy] of Object.entries(COMMAND_POLICIES)) {
      for (const role of ONYX_ROLES) {
        const decision = authorizeCommand(role, action, policy.targetType);
        expect(decision.allowed, `${role} / ${action}`).toBe(policy.roles.includes(role));
      }
    }
  });

  it("denies unknown actions, deletion-shaped names, and target-type spoofing by default", () => {
    for (const role of ONYX_ROLES) {
      expect(authorizeCommand(role, "DeleteEverything", "task")).toEqual({
        allowed: false,
        reason: "UNKNOWN_COMMAND",
      });
      expect(authorizeCommand(role, "RemoveUser", "staff_profile").allowed).toBe(false);
      expect(authorizeCommand(role, "ApproveTask", "mission")).toEqual({
        allowed: false,
        reason: "TARGET_TYPE_MISMATCH",
      });
    }
  });

  it("denies Staff access to organization provisioning and approval actions", () => {
    expect(authorizeCommand("STAFF", "CreateOrganization", "organization").allowed).toBe(false);
    expect(authorizeCommand("STAFF", "ApproveTask", "task").allowed).toBe(false);
    expect(authorizeCommand("ORGANIZATION_ADMIN", "CreateOrganization", "organization").allowed).toBe(false);
  });

  it("restricts privileged query classes and denies unknown queries", () => {
    expect(authorizeQuery("STAFF", "policy.list")).toBe(false);
    expect(authorizeQuery("STAFF", "legal_hold.detail")).toBe(false);
    expect(authorizeQuery("ORGANIZATION_ADMIN", "policy.list")).toBe(true);
    expect(authorizeQuery("STAFF", "mission.list")).toBe(true);
    expect(authorizeQuery("STAFF", "admin.dump_all_users")).toBe(false);
  });
});

describe("resource ownership policy", () => {
  it("requires a concrete owner and rejects missing or conflicting ownership", () => {
    expect(resourceOwnedBy({ title: "no owner" }, "staff-1")).toBe(false);
    expect(resourceOwnedBy({ owner_id: "staff-1" }, "staff-1")).toBe(true);
    expect(resourceOwnedBy({ owner_id: "staff-2" }, "staff-1")).toBe(false);
    expect(resourceOwnedBy({ owner_id: "staff-1", assignee_id: "staff-2" }, "staff-1")).toBe(false);
    expect(resourceOwnedBy({ owner: { user_id: "staff-1" } }, "staff-1")).toBe(true);
  });
});

describe("explicit command payload and lifecycle semantics", () => {
  it("has a semantics entry for every role/action policy", () => {
    for (const action of Object.keys(COMMAND_POLICIES)) {
      expect(COMMAND_SEMANTICS[action], action).toBeDefined();
    }
  });

  it("accepts a valid task transition and denies invalid transitions", () => {
    expect(validateCommandSemantics("StartTask", { status: "Ready", owner_id: "staff-1" }, { reason: "start" }))
      .toEqual({ allowed: true, nextStatus: "Active" });
    expect(validateCommandSemantics("StartTask", { status: "Closed", owner_id: "staff-1" }, { reason: "start" }))
      .toEqual({ allowed: false, reason: "INVALID_STATE_TRANSITION" });
    expect(validateCommandSemantics("CancelTask", { status: "Closed" }, { reason: "cancel" }))
      .toEqual({ allowed: false, reason: "INVALID_STATE_TRANSITION" });
  });

  it("sets create lifecycle state on the server, not from client payload", () => {
    expect(validateCommandSemantics("CreateTask", null, {
      mission_id: "mission-1", title: "Task", owner_id: "staff-1",
    })).toEqual({ allowed: true, nextStatus: "Draft", statePatch: { priority: "Normal", dependencies: [] } });
    expect(validateCommandSemantics("CreateTask", null, {
      mission_id: "mission-1", title: "Task", owner_id: "staff-1", status: "Closed",
    })).toEqual({ allowed: false, reason: "INVALID_COMMAND_PAYLOAD" });
  });

  it("rejects unknown payload fields and invalid enum values", () => {
    expect(validateCommandSemantics("PauseMission", { status: "Active" }, { reason: "pause", is_admin: true }))
      .toEqual({ allowed: false, reason: "INVALID_COMMAND_PAYLOAD" });
    expect(validateCommandSemantics("todo_list.VerifyTodoList", { status: "Submitted" }, { outcome: "approve" }))
      .toEqual({ allowed: false, reason: "INVALID_COMMAND_PAYLOAD" });
  });

  it("fails closed where domain-specific authority/time preconditions are not yet provable", () => {
    expect(validateCommandSemantics("staff_loan.ApproveStaffLoan", { status: "Requested" }, {}))
      .toEqual({ allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" });
    expect(validateCommandSemantics("target_list.VerifyTargetList", { status: "Submitted" }, { outcome: "Flawless" }))
      .toEqual({ allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" });
  });

  it("requires exact approved transition prerequisites", () => {
    expect(validateCommandSemantics("approval.Approve", { status: "pending", web_action_permitted: false }, {}))
      .toEqual({ allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" });
    expect(validateCommandSemantics("SubmitBlueprint", { status: "Draft", blueprint_revision_id: "r2" }, { revision_id: "r1" }))
      .toEqual({ allowed: false, reason: "COMMAND_PRECONDITION_UNSUPPORTED" });
  });
});
