import { useCallback, useEffect, useState } from "react";
import { executeQuery } from "@/api/query";

export interface UseQueryResult<TResult> {
  data: TResult | null;
  loading: boolean;
  error: { message: string } | null;
  /** Manually re-runs the query — this app has no cache invalidation,
   * so callers refetch explicitly after a command that changed the
   * underlying aggregate. */
  refetch: () => Promise<void>;
}

/**
 * HTTP equivalent of `desktop-shell`'s `useQuery` hook — same
 * loading/error/refetch shape, but goes through `api/query.ts`'s
 * `executeQuery` (copied verbatim from `web-ui`, the established
 * working reference) instead of Tauri's `execute_query` IPC command.
 *
 * Note the real `/api/query` contract, which is **not** a pair of plain
 * `query_type`/`target_id` query params (an earlier draft of this hook
 * assumed that and was wrong — caught by reading
 * `api_server::routes::query::query_route` directly): it takes a single
 * `envelope` param containing base64url-encoded JSON matching
 * `QueryEnvelopeDto`, and filters by fields inside `filters`, not by a
 * top-level target id.
 *
 * Ids here are plain UUID strings, **not** `desktop-shell`'s `Id16`
 * (`number[]`) byte-array form — that format exists because
 * desktop-shell talks to Rust directly over Tauri IPC, where
 * `ObjectId`'s raw serde representation is a 16-number array;
 * `api-server`'s HTTP routes speak UUID strings.
 */
export function useQuery<TResult = unknown>(
  queryType: string,
  filters: Record<string, unknown> | null,
): UseQueryResult<TResult> {
  const [data, setData] = useState<TResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{ message: string } | null>(null);

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
    } catch (e) {
      const message =
        (e as { response?: { data?: { message?: string } } }).response?.data?.message ??
        String(e);
      setError({ message });
    } finally {
      setLoading(false);
    }
  }, [queryType, filtersKey]);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  return { data, loading, error, refetch };
}
