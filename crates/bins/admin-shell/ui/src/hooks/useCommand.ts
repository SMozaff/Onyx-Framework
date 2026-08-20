import { useCallback, useState } from "react";
import { apiClient } from "@/api/client";
import { normalizeError, type UserFacingError } from "@/utils/errorHandler";

/** HTTP equivalent of the Staff command hook, with user-safe errors. */
export interface UseCommandResult<TPayload> {
  execute: (params: {
    commandType: string;
    targetId: string;
    targetType: string;
    organizationId: string;
    expectedVersion: number;
    expectedLifecycleEpoch?: number;
    expectedAuthorityEpoch?: number;
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
      const envelope = {
        command_id: crypto.randomUUID(),
        operation_id: crypto.randomUUID(),
        command_type: params.commandType,
        schema_version: "1.0",
        target: { id: params.targetId, type: params.targetType, organization_id: params.organizationId },
        expected_version: params.expectedVersion,
        expected_lifecycle_epoch: params.expectedLifecycleEpoch ?? 0,
        expected_authority_epoch: params.expectedAuthorityEpoch ?? 0,
        issued_at: new Date().toISOString(),
        vector_clock: { entries: {} },
        correlation_id: crypto.randomUUID(),
        causation_id: null,
        payload: params.payload,
      };
      const response = await apiClient.post("/api/command", envelope);
      return response.data;
    } catch (caught) {
      const safeError = normalizeError(caught);
      setError(safeError);
      throw safeError;
    } finally {
      setLoading(false);
    }
  }, []);

  return { execute, loading, error };
}
