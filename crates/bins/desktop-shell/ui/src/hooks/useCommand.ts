import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ActorContext, AuthorityProof, CommandEnvelope, DomainObjectRef, Id16 } from "@/types/onyx";
import { EMPTY_VECTOR_CLOCK, newId16 } from "@/types/onyx";
import { toUserFacingError, type UserFacingError } from "@/utils/userFacingError";

function buildEnvelope<TPayload>(params: {
  commandType: string;
  targetId: Id16;
  targetType: string;
  organizationId: Id16;
  userId: Id16;
  deviceId: Id16;
  expectedVersion: number;
  expectedLifecycleEpoch?: number;
  expectedAuthorityEpoch?: number;
  payload: TPayload;
}): CommandEnvelope<TPayload> {
  const target: DomainObjectRef = { id: params.targetId, type: params.targetType, organization_id: params.organizationId };
  const actor: ActorContext = { user_id: params.userId, device_id: params.deviceId, organization_id: params.organizationId };
  const authorityProof: AuthorityProof = {
    proof_type: "Jwt",
    scope: { organization_id: params.organizationId, object_type: params.targetType, object_id: null, command_types: [params.commandType], delegation_depth: 0 },
    issued_at: 0,
    expires_at: Number.MAX_SAFE_INTEGER,
    signature: null,
  };
  return {
    command_id: newId16(),
    operation_id: newId16(),
    command_type: params.commandType,
    schema_version: "1.0.0",
    target,
    expected_version: params.expectedVersion,
    expected_lifecycle_epoch: params.expectedLifecycleEpoch ?? 0,
    expected_authority_epoch: params.expectedAuthorityEpoch ?? 0,
    actor,
    authority_proof: authorityProof,
    issued_at: Date.now() * 1_000_000,
    vector_clock: EMPTY_VECTOR_CLOCK,
    correlation_id: newId16(),
    causation_id: null,
    payload: params.payload,
  };
}

export interface UseCommandResult<TPayload> {
  execute: (params: {
    commandType: string;
    targetId: Id16;
    targetType: string;
    organizationId: Id16;
    userId: Id16;
    deviceId: Id16;
    expectedVersion: number;
    expectedLifecycleEpoch?: number;
    payload: TPayload;
  }) => Promise<unknown>;
  loading: boolean;
  error: UserFacingError | null;
}

export function useCommand<TPayload = unknown>(): UseCommandResult<TPayload> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UserFacingError | null>(null);

  const execute = useCallback<UseCommandResult<TPayload>["execute"]>(async (params) => {
    setLoading(true);
    setError(null);
    try {
      const envelope = buildEnvelope(params);
      return await invoke("execute_command", { envelope });
    } catch (caught) {
      const safeError = toUserFacingError(caught);
      setError(safeError);
      throw safeError;
    } finally {
      setLoading(false);
    }
  }, []);

  return { execute, loading, error };
}
