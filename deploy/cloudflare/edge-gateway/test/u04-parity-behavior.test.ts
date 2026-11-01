import { describe, expect, it } from "vitest";
import { validateCommandSemantics } from "../src/command-semantics";

// U-04 behavior tests (docs/migrations/slint/u04-policy-parity.md).
// These pin the Worker side of the policy parity matrix. They are characterization
// tests: a failing assertion means the Worker changed, not that the test is wrong.

const ACTOR = "user-1";
const scope = { Organization: "org-1" };
const rules = [{ key: "max_hours", value: 8 }];

const publishedOnly = { status: "Active", versions: [{ version_number: 1, status: "Published", rules }] };
const draftOnly = { status: "Active", versions: [{ version_number: 1, status: "Draft", rules }] };
const publishedAndDraft = {
  status: "Active",
  versions: [
    { version_number: 1, status: "Published", rules },
    { version_number: 2, status: "Draft", rules },
  ],
};
const retired = { status: "Retired", versions: [{ version_number: 1, status: "Published", rules }] };

describe("U-04 parity: published versions are immutable", () => {
  it("creating a new version next to a Published one is allowed (Published is not edited)", () => {
    const r = validateCommandSemantics("CreatePolicyVersion", publishedOnly, { rules }, ACTOR);
    expect(r).toMatchObject({ allowed: true });
  });

  it("no command is allowed to target a Published version directly (publish needs a Draft)", () => {
    const r = validateCommandSemantics("PublishPolicyVersion", publishedOnly, undefined, ACTOR);
    expect(r).toEqual({ allowed: false, reason: "NO_DRAFT_VERSION" });
  });
});

describe("U-04 parity: error codes match the decided API names", () => {
  it("row 1: create version while a Draft exists returns DRAFT_ALREADY_PENDING", () => {
    expect(validateCommandSemantics("CreatePolicyVersion", draftOnly, { rules }, ACTOR)).toEqual({
      allowed: false,
      reason: "DRAFT_ALREADY_PENDING",
    });
  });

  it("row 2: publish with no Draft returns NO_DRAFT_VERSION", () => {
    expect(validateCommandSemantics("PublishPolicyVersion", publishedOnly, undefined, ACTOR)).toEqual({
      allowed: false,
      reason: "NO_DRAFT_VERSION",
    });
  });

  it("row 3: evaluate with no Published version returns NO_PUBLISHED_VERSION", () => {
    expect(
      validateCommandSemantics("EvaluatePolicy", draftOnly, { rule_key: "max_hours" }, ACTOR),
    ).toEqual({ allowed: false, reason: "NO_PUBLISHED_VERSION" });
  });

  it("row 6: register violation with no Published version returns NO_PUBLISHED_VERSION", () => {
    expect(
      validateCommandSemantics("RegisterViolation", draftOnly, { rule_key: "max_hours", reason: "x" }, ACTOR),
    ).toEqual({ allowed: false, reason: "NO_PUBLISHED_VERSION" });
  });
});

describe("U-04 parity: unknown rule keys", () => {
  it("row 4: evaluate an unknown rule key is accepted", () => {
    expect(validateCommandSemantics("EvaluatePolicy", publishedOnly, { rule_key: "nope" }, ACTOR)).toMatchObject({
      allowed: true,
    });
  });

  it("row 5: register a violation with an unknown rule key is accepted", () => {
    expect(
      validateCommandSemantics("RegisterViolation", publishedOnly, { rule_key: "nope", reason: "x" }, ACTOR),
    ).toMatchObject({ allowed: true });
  });
});

describe("U-04 parity: retire is terminal", () => {
  it("row 9: retiring an already Retired policy is rejected", () => {
    expect(validateCommandSemantics("RetirePolicy", retired, undefined, ACTOR)).toMatchObject({ allowed: false });
  });

  it("row 7: evaluate on a Retired policy is rejected", () => {
    expect(validateCommandSemantics("EvaluatePolicy", retired, { rule_key: "max_hours" }, ACTOR)).toMatchObject({
      allowed: false,
    });
  });

  it("row 8: create a version on a Retired policy is rejected", () => {
    expect(validateCommandSemantics("CreatePolicyVersion", retired, { rules }, ACTOR)).toMatchObject({
      allowed: false,
    });
  });
});

describe("U-04 parity: known gaps (recorded, not hidden)", () => {
  it.todo("row 10: empty policy name is rejected (Rust MissingField; Worker does not check here)");
  it.todo("optimistic concurrency: expected_version mismatch returns VERSION_CONFLICT on both sides");
});
