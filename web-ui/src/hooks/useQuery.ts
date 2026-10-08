import { useQuery as useReactQuery, type UseQueryOptions } from '@tanstack/react-query';
import { executeQuery } from '../api/query';
import type { QueryResponse } from '../types/query';

export function useOnyxQuery<T>(
  queryType: string,
  filters: Record<string, unknown> = {},
  options: Partial<UseQueryOptions<QueryResponse<T>>> = {},
) {
  return useReactQuery({
    queryKey: [queryType, filters],
    queryFn: () => executeQuery<T>(queryType, filters),
    staleTime: 60_000,
    retry: 1,
    networkMode: 'always',
    refetchOnReconnect: true,
    ...options,
  });
}
