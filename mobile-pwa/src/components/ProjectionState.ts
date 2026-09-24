import type { QueryResponse } from '../types/query';

/** Narrow typing of the projection-state union used by the pages. */
export interface ProjectionState {
  kind: 'loading' | 'unavailable' | 'stale' | 'empty' | 'ready';
}

export function deriveProjectionState(query: { data?: QueryResponse; isPending?: boolean; isError?: boolean; isFetching?: boolean }): ProjectionState {
  if (query.isPending) return { kind: 'loading' };
  if (query.isError && !query.data) return { kind: 'unavailable' };
  if (!query.data?.data?.length) return { kind: 'empty' };
  if (query.data.data.length > 0 && query.data.freshness?.is_stale) return { kind: 'stale' };
  return { kind: 'ready' };
}