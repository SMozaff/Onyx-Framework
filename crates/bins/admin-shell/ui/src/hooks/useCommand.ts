import { useCallback, useState } from "react";
import { apiClient } from "@/api/client";

/**
 * HTTP equivalent of `desktop-shell`'s `useCommand` hook — same
 * call-signature shape (so `Settings.tsx`, ported from `desktop-shell`,
 * needed minimal changes beyond `Id16` → `string`), but posts to
 * `api-server`'s real `/api/command` route directly instead of going
 * through Tauri's `execute_command` IPC command, since this app is a
 * thin HTTP client with no embedded `client-composition` (see
 * `admin_shell_lib`'s crate doc comment).
 *
 * Envelope shape confirmed against `web-ui/src/api/command.ts`'s
 * working `buildCommandEnvelope` (the established, real reference for
 * what `/api/command` actually expects) rather than reconstructed from
 * memory.
 */
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
  error: { message: string } | null;
}

export function useCommand<TPayload = unknown>(): UseCommandResult<TPayload> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{ message: string } | null>(null);

  const execute = useCallback<UseCommandResult<TPayload>["execute"]>(async (params) => {
    setLoading(true);
    setError(null);
    try {
      const envelope = {
        command_id: crypto.randomUUID(),
        operation_id: crypto.randomUUID(),
        command_type: params.commandType,
        schema_version: "1.0",
        target: {
          id: params.targetId,
          type: params.targetType,
          organization_id: params.organizationId,
        },
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
    } catch (e) {
      const message =
        (e as { response?: { data?: { message?: string } } }).response?.data?.message ??
        String(e);
      const wrapped = { message };
      setError(wrapped);
      throw wrapped;
    } finally {
      setLoading(false);
    }
  }, []);

  return { execute, loading, error };
}
