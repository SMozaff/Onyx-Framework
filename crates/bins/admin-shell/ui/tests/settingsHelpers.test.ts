import { describe, expect, it, vi } from "vitest";
import {
  detailProjection,
  detailRequestFilters,
  isAmbiguousCreateError,
  legalHoldReleaseCommand,
  newCreateOperation,
  policyCommand,
  postIdempotentCreation,
  projectionConcurrency,
  type ApiClientLike,
} from "../src/pages/settingsHelpers";

const POLICY_A = "11111111-1111-4111-8111-111111111111";
const POLICY_B = "22222222-2222-4222-8222-222222222222";
const HOLD_A = "33333333-3333-4333-8333-333333333333";
const ORG = "44444444-4444-4444-8444-444444444444";

function nestedPolicy(id: string, version = 7, lifecycleEpoch = 11, authorityEpoch = 13) {
  return {
    id,
    version,
    lifecycle_epoch: lifecycleEpoch,
    authority_epoch: authorityEpoch,
    state: { id, name: `Policy ${id}`, status: "Active", versions: [] },
  };
}

describe("Settings detail projection helpers", () => {
  it("uses Worker target_id filters and selects the requested nested state when several IDs are returned", () => {
    expect(detailRequestFilters(POLICY_B)).toEqual({ target_id: POLICY_B });

    const projection = detailProjection({ data: [nestedPolicy(POLICY_A), nestedPolicy(POLICY_B)] }, POLICY_B);
    expect(projection).toMatchObject({
      id: POLICY_B,
      name: `Policy ${POLICY_B}`,
      version: 7,
      lifecycle_epoch: 11,
      authority_epoch: 13,
    });
    expect(detailProjection({ data: [nestedPolicy(POLICY_A)] }, POLICY_B)).toBeNull();
  });

  it("accepts a returned direct projection state and does not render a row for a different requested ID", () => {
    const direct = {
      id: HOLD_A,
      status: "Applied",
      version: 3,
      lifecycle_epoch: 5,
      authority_epoch: 8,
    };
    expect(detailProjection({ data: [direct] }, HOLD_A)).toEqual(direct);
    expect(detailProjection({ data: [direct] }, POLICY_A)).toBeNull();
  });
});

describe("Settings Worker command helpers", () => {
  it("uses the projection version and both epochs in Worker-native policy payloads", () => {
    const projection = detailProjection({ data: [nestedPolicy(POLICY_A)] }, POLICY_A)!;
    const command = policyCommand(
      "CreatePolicyVersion",
      POLICY_A,
      ORG,
      projection,
      { rules: [{ rule_type: "FeatureToggle", key: "messaging.enabled", value: true }] },
    );

    expect(command).toEqual({
      commandType: "policy.CreatePolicyVersion",
      targetId: POLICY_A,
      targetType: "policy",
      organizationId: ORG,
      expectedVersion: 7,
      expectedLifecycleEpoch: 11,
      expectedAuthorityEpoch: 13,
      payload: { rules: [{ rule_type: "FeatureToggle", key: "messaging.enabled", value: true }] },
    });
  });

  it("passes all legal-hold projection epochs and refuses missing or malformed epoch values", () => {
    const hold = {
      id: HOLD_A,
      status: "Applied",
      version: 4,
      lifecycle_epoch: 6,
      authority_epoch: 9,
    };
    expect(legalHoldReleaseCommand(HOLD_A, ORG, hold, "Released by administrator")).toEqual({
      commandType: "legal_hold.ReleaseLegalHold",
      targetId: HOLD_A,
      targetType: "legal_hold",
      organizationId: ORG,
      expectedVersion: 4,
      expectedLifecycleEpoch: 6,
      expectedAuthorityEpoch: 9,
      payload: { reason: "Released by administrator" },
    });

    // A stale/partial detail response must not be converted into a command
    // with zero epochs, because that would bypass the Worker conflict check.
    const missingAuthorityEpoch = { ...hold, authority_epoch: undefined };
    expect(projectionConcurrency(missingAuthorityEpoch)).toBeNull();
    expect(policyCommand("RetirePolicy", POLICY_A, ORG, missingAuthorityEpoch, {})).toBeNull();
    expect(projectionConcurrency({ ...hold, lifecycle_epoch: -1 })).toBeNull();
  });
});

describe("Settings idempotent creation", () => {
  it("reuses one Worker idempotency key and exact request payload for a duplicate/retry", async () => {
    const post = vi.fn()
      .mockResolvedValueOnce({ data: { policy_id: POLICY_A } })
      .mockResolvedValueOnce({ data: { policy_id: POLICY_A } });
    const mockApiClient = { post: post as ApiClientLike["post"] };
    const operation = newCreateOperation({ name: "Operations Policy" }, () => "policy-create-1");

    await expect(postIdempotentCreation(mockApiClient, "/api/admin/policies", operation))
      .resolves.toEqual({ policy_id: POLICY_A });
    await expect(postIdempotentCreation(mockApiClient, "/api/admin/policies", operation))
      .resolves.toEqual({ policy_id: POLICY_A });

    expect(post).toHaveBeenCalledTimes(2);
    expect(post.mock.calls).toEqual([
      ["/api/admin/policies", { name: "Operations Policy" }, { headers: { "Idempotency-Key": "policy-create-1" } }],
      ["/api/admin/policies", { name: "Operations Policy" }, { headers: { "Idempotency-Key": "policy-create-1" } }],
    ]);
  });

  it("sends the Worker-native legal-hold payload with its own stable retry key", async () => {
    const post = vi.fn().mockResolvedValue({ data: { legal_hold_id: HOLD_A } });
    const mockApiClient = { post: post as ApiClientLike["post"] };
    const operation = newCreateOperation({
      target_id: POLICY_A,
      target_type: "policy",
      reason: "Preserve governance evidence.",
    }, () => "legal-hold-create-1");

    await postIdempotentCreation(mockApiClient, "/api/admin/legal-holds", operation);
    await postIdempotentCreation(mockApiClient, "/api/admin/legal-holds", operation);

    expect(post).toHaveBeenCalledTimes(2);
    expect(post.mock.calls[0]).toEqual([
      "/api/admin/legal-holds",
      { target_id: POLICY_A, target_type: "policy", reason: "Preserve governance evidence." },
      { headers: { "Idempotency-Key": "legal-hold-create-1" } },
    ]);
    expect(post.mock.calls[1][2]).toEqual({ headers: { "Idempotency-Key": "legal-hold-create-1" } });
  });

  it("marks transport and transient server failures as ambiguous so the UI retains the key for retry", () => {
    expect(isAmbiguousCreateError(new Error("socket closed after request write"))).toBe(true);
    expect(isAmbiguousCreateError({ response: { status: 503, data: { retryability: "TRANSIENT" } } })).toBe(true);
    expect(isAmbiguousCreateError({ response: { status: 409, data: { error: "IDEMPOTENCY_CONFLICT" } } })).toBe(false);
  });
});
