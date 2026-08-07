import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Id16, ShellError } from "@/types/onyx";
import { isShellError } from "@/types/onyx";

export interface UseQueryResult<TResult> {
  data: TResult | null;
  loading: boolean;
  error: ShellError | null;
  /** Manually re-runs the query (e.g. after a command that changed the
   * underlying aggregate — this app has no automatic cache invalidation,
   * so callers refetch explicitly, or rely on the live `onyx:event`
   * stream via `useEventStream` to know when to). */
  refetch: () => Promise<void>;
}

/**
 * React hook wrapping the `execute_query` Tauri command
 * (`desktop_shell_lib::execute_query`, which forwards to
 * `client-composition`'s real `QueryRegistry::dispatch`). Runs
 * automatically on mount and whenever `queryType`/`targetId` change;
 * pass `targetId: null` to skip running until a real id is available
 * (e.g. before the user has selected anything).
 */
export function useQuery<TResult = unknown>(
  queryType: string,
  targetId: Id16 | null,
): UseQueryResult<TResult> {
  const [data, setData] = useState<TResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<ShellError | null>(null);

  const refetch = useCallback(async () => {
    if (targetId === null) {
      setData(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      // Argument keys (`queryType`, `targetId`) must match
      // `execute_query`'s Rust parameter names in camelCase per Tauri's
      // default JS<->Rust argument name conversion (Rust `query_type` /
      // `target_id` are snake_case; Tauri's IPC layer accepts the
      // camelCase form from JS by convention) — verified against the
      // real, current Tauri v2 docs before relying on this rather than
      // assuming snake_case would also work.
      const result = await invoke<TResult>("execute_query", {
        queryType,
        targetId,
      });
      setData(result);
    } catch (e) {
      const shellError: ShellError = isShellError(e)
        ? e
        : { kind: "query", message: String(e) };
      setError(shellError);
    } finally {
      setLoading(false);
    }
  }, [queryType, targetId]);

  useEffect(() => {
    void refetch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [queryType, JSON.stringify(targetId)]);

  return { data, loading, error, refetch };
}
