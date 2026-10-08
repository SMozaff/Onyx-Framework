import { useCallback, useEffect, useState } from "react";
import { executeQuery } from "@/api/query";
import { normalizeError, type UserFacingError } from "@/utils/errorHandler";

export interface UseQueryResult<TResult> {
  data: TResult | null;
  loading: boolean;
  error: UserFacingError | null;
  refetch: () => Promise<void>;
}

/** HTTP query hook with stable user-safe error output. */
export function useQuery<TResult = unknown>(queryType: string, filters: Record<string, unknown> | null): UseQueryResult<TResult> {
  const [data, setData] = useState<TResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UserFacingError | null>(null);
  const filtersKey = filters === null ? null : JSON.stringify(filters);

  const refetch = useCallback(async () => {
    if (filtersKey === null) {
      setData(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const result = await executeQuery<TResult>(queryType, JSON.parse(filtersKey));
      setData(result as TResult);
    } catch (caught) {
      setError(normalizeError(caught));
    } finally {
      setLoading(false);
    }
  }, [queryType, filtersKey]);

  useEffect(() => { void refetch(); }, [refetch]);
  return { data, loading, error, refetch };
}
