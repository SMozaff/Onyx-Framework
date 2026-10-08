import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Id16 } from "@/types/onyx";
import { toUserFacingError, type UserFacingError } from "@/utils/userFacingError";

export interface UseQueryResult<TResult> {
  data: TResult | null;
  loading: boolean;
  error: UserFacingError | null;
  /** Manually re-runs the query after a changed operational object. */
  refetch: () => Promise<void>;
}

export function useQuery<TResult = unknown>(queryType: string, targetId: Id16 | null): UseQueryResult<TResult> {
  const [data, setData] = useState<TResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UserFacingError | null>(null);

  const refetch = useCallback(async () => {
    if (targetId === null) {
      setData(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<TResult>("execute_query", { queryType, targetId });
      setData(result);
    } catch (caught) {
      setError(toUserFacingError(caught));
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
