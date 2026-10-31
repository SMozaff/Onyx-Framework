import { describe, expect, it } from "vitest";
import { authorizeCommand } from "../src/authorization";
import { validateCommandSemantics } from "../src/command-semantics";

// Characterization tests: these record what the Worker does today.
// They do not assert that the current behaviour is the desired behaviour.
// Where the Worker and the Rust API disagree, the test says so.

const ACTOR = "user-1";
const scope = { Organization: "org-1" };
const rules = [{ key: "max_hours", value: 8 }];

const activeWithPublished = {
  status: "Active",
  versions: [{ version_number: 1, status: "Published", rules }],
};
const activeWithDraftOnly = {
  status: "Active",
  versions: [{ version_number: 1, status: "Draft", rules }],
};
const activeNoVersions = { status: "Active", versions: [] };
const retired = { status: "Retired", versions: [{ version_number: 1, status: "Published", rules }] };

describe("policy: authorization (Worker, unprefixed names)", () => {
  it("allows ORGANIZATION_ADMIN on every policy command", () => {
    for (const cmd of [
      "CreatePolicy",
      "CreatePolicyVersion",
      "PublishPolicyVersion",
      "EvaluatePolicy",
      "RegisterViolation",
      "RetirePolicy",
    ]) {
      expect(authorizeCommand("ORGANIZATION_ADMIN", cmd, "policy").allowed).toBe(true);
    }
  });

  it("denies STAFF on policy commands", () => {
    expect(authorizeCommand("STAFF", "RetirePolicy", "policy")).toEqual({
      allowed: false,
      reason: "ROLE_DENIED",
    });
  });

  // authorizeCommand itself still knows only unprefixed names. The "policy." alias
  // is mapped in the /api/command route (src/index.ts) before this check runs; the
  // end-to-end test in security.test.ts covers the route.
  it.each([
    "policy.CreatePolicyVersion",
    "policy.PublishPolicyVersion",
    "policy.EvaluatePolicy",
    "policy.RegisterViolation",
    "policy.RetirePolicy",
  ])("rejects prefixed name %s with UNKNOWN_COMMAND today", (cmd) => {
    expect(authorizeCommand("ORGANIZATION_ADMIN", cmd, "policy")).toEqual({
      allowed: false,
      reason: "UNKNOWN_COMMAND",
    });
  });
});

describe("policy: state transitions (validateCommandSemantics)", () => {
  it("CreatePolicy creates an Active policy when the scope is valid", () => {
    const result = validateCommandSemantics("CreatePolicy", undefined, { name: "p", scope }, ACTOR);
    expect(result).toMatchObject({ allowed: true, nextStatus: "Active" });
  });

  it("CreatePolicy is rejected when the policy already exists", () => {
    const result = validateCommandSemantics("CreatePolicy", activeNoVersions, { name: "p", scope }, ACTOR);
    expect(result).toEqual({ allowed: false, reason: "RESOURCE_ALREADY_EXISTS" });
  });

  it("CreatePolicy rejects an invalid scope", () => {
    const result = validateCommandSemantics("CreatePolicy", undefined, { name: "p", scope: { Other: "x" } }, ACTOR);
    expect(result).toEqual({ allowed: false, reason: "INVALID_COMMAND_PAYLOAD" });
  });

  it("CreatePolicyVersion adds a Draft version and keeps the policy Active", () => {
    const result = validateCommandSemantics(
      "CreatePolicyVersion",
      activeNoVersions,
      { rules },
      ACTOR,
    );
    expect(result).toMatchObject({ allowed: true, nextStatus: "Active" });
  });

  it("CreatePolicyVersion is rejected while a Draft already exists", () => {
    const result = validateCommandSemantics("CreatePolicyVersion", activeWithDraftOnly, { rules }, ACTOR);
    expect(result).toEqual({ allowed: false, reason: "DRAFT_ALREADY_PENDING" });
  });

  it("PublishPolicyVersion needs a Draft", () => {
    expect(validateCommandSemantics("PublishPolicyVersion", activeWithPublished, {}, ACTOR))
      .toEqual({ allowed: false, reason: "NO_DRAFT_VERSION" });
    expect(validateCommandSemantics("PublishPolicyVersion", activeWithDraftOnly, {}, ACTOR))
      .toMatchObject({ allowed: true, nextStatus: "Active" });
  });

  it("EvaluatePolicy returns the matching rule value from the published version", () => {
    const result = validateCommandSemantics(
      "EvaluatePolicy",
      activeWithPublished,
      { rule_key: "max_hours" },
      ACTOR,
    );
    expect(result).toMatchObject({
      allowed: true,
      commandResult: { rule_key: "max_hours", result: 8, evaluated_version: 1 },
    });
  });

  it("EvaluatePolicy returns null for an unknown rule key", () => {
    const result = validateCommandSemantics(
      "EvaluatePolicy",
      activeWithPublished,
      { rule_key: "missing" },
      ACTOR,
    );
    expect(result).toMatchObject({ allowed: true, commandResult: { result: null } });
  });

  it("EvaluatePolicy needs a published version", () => {
    expect(validateCommandSemantics("EvaluatePolicy", activeWithDraftOnly, { rule_key: "max_hours" }, ACTOR))
      .toEqual({ allowed: false, reason: "NO_PUBLISHED_VERSION" });
  });

  it("RegisterViolation records even when the rule key is not in the policy (matches Rust)", () => {
    const result = validateCommandSemantics(
      "RegisterViolation",
      activeWithPublished,
      { rule_key: "not-a-rule", reason: "late" },
      ACTOR,
    );
    expect(result).toMatchObject({
      allowed: true,
      commandResult: { recorded: true, rule_key: "not-a-rule", policy_version: 1 },
    });
  });

  it("RetirePolicy moves Active to Retired", () => {
    const result = validateCommandSemantics("RetirePolicy", activeWithPublished, {}, ACTOR);
    expect(result).toMatchObject({ allowed: true, nextStatus: "Retired" });
  });

  it("RetirePolicy is rejected on a policy that is already Retired", () => {
    const result = validateCommandSemantics("RetirePolicy", retired, {}, ACTOR);
    expect(result).toEqual({ allowed: false, reason: "INVALID_STATE_TRANSITION" });
  });

  // Rust's Policy aggregate and the Worker both reject these commands on a Retired policy.
  // Keep these as parity assertions; the former mismatch comment was stale.
  it("Worker rejects EvaluatePolicy on a Retired policy (Rust parity)", () => {
    const result = validateCommandSemantics("EvaluatePolicy", retired, { rule_key: "max_hours" }, ACTOR);
    expect(result).toEqual({ allowed: false, reason: "INVALID_STATE_TRANSITION" });
  });

  it("Worker rejects RegisterViolation on a Retired policy (Rust parity)", () => {
    const result = validateCommandSemantics("RegisterViolation", retired, { rule_key: "x", reason: "y" }, ACTOR);
    expect(result).toEqual({ allowed: false, reason: "INVALID_STATE_TRANSITION" });
  });

  it("any policy command on a missing policy is rejected as unverified state", () => {
    expect(validateCommandSemantics("RetirePolicy", undefined, {}, ACTOR))
      .toEqual({ allowed: false, reason: "RESOURCE_STATE_UNVERIFIED" });
  });
});
