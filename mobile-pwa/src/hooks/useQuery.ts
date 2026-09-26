import { useQuery as useReactQuery, type UseQueryOptions } from '@tanstack/react-query';
import { observerApi } from '../api/onyx';
import type { QueryResponse } from '../types/query';

/**
 * Observer flavor of web-ui's `useOnyxQuery`. Query types and filters are
 * the backend's dotted names (`mission.list`, filter keys match top-level
 * projection state fields — see `query_handler.rs::matches_filters`).
 */
export function useObserverQuery<T>(
  queryType: string,
  filters: Record<string, unknown> = {},
  options: Partial<UseQueryOptions<QueryResponse<T>>> = {},
) {
  return useReactQuery({
    queryKey: [queryType, filters],
    queryFn: () => observerApi.query<T>(queryType, filters),
    staleTime: 60_000,
    retry: 1,
    networkMode: 'always',
    refetchOnReconnect: true,
    refetchOnWindowFocus: false,
    ...options,
  });
}