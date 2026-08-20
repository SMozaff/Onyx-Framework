import type { UseQueryResult } from '@tanstack/react-query';
import type { QueryResponse } from '../../types/query';
import { normalizeError, type UserFacingError } from '../../utils/errorHandler';

export type ProjectionState =
  | { kind: 'loading' }
  | { kind: 'unavailable'; error: UserFacingError; retry: () => void }
  | { kind: 'stale'; lastUpdatedAt: string; error?: UserFacingError; retry: () => void }
  | { kind: 'empty'; lastUpdatedAt: string }
  | { kind: 'ready'; lastUpdatedAt: string };

export function deriveProjectionState<T>(query: UseQueryResult<QueryResponse<T>, unknown>): ProjectionState {
  if (query.isPending && !query.data) return { kind: 'loading' };

  const error = query.isError ? normalizeError(query.error) : undefined;
  if (!query.data) {
    return {
      kind: 'unavailable',
      error: error ?? {
        code: 'SERVICE_UNAVAILABLE',
        title: 'Projection unavailable',
        message: 'This projection could not be loaded. Retry when the service is reachable.',
        retryable: true,
        action: 'retry',
      },
      retry: () => { void query.refetch(); },
    };
  }

  if (query.data.freshness.is_stale || error) {
    return {
      kind: 'stale',
      lastUpdatedAt: query.data.freshness.last_updated_at,
      error,
      retry: () => { void query.refetch(); },
    };
  }

  return query.data.data.length === 0
    ? { kind: 'empty', lastUpdatedAt: query.data.freshness.last_updated_at }
    : { kind: 'ready', lastUpdatedAt: query.data.freshness.last_updated_at };
}

export function ProjectionStatePanel({
  resource,
  state,
  compact = false,
}: {
  resource: string;
  state: Extract<ProjectionState, { kind: 'unavailable' | 'stale' }>;
  compact?: boolean;
}) {
  const unavailable = state.kind === 'unavailable';
  const title = unavailable ? `${resource} unavailable` : `${resource} may be stale`;
  const message = unavailable
    ? state.error.message
    : state.error?.message ?? 'The last successfully loaded data is still visible. Refresh before acting on time-sensitive information.';

  return (
    <section className={`projection-state projection-state-${state.kind} ${compact ? 'projection-state-compact' : ''}`} role={unavailable ? 'alert' : 'status'} aria-live={unavailable ? 'assertive' : 'polite'}>
      <div>
        <strong>{title}</strong>
        <p>{message}</p>
        {!unavailable && <small>Last updated {formatTimestamp(state.lastUpdatedAt)}</small>}
      </div>
      {state.error?.retryable !== false && <button type="button" className="button-secondary" onClick={state.retry}>Retry</button>}
    </section>
  );
}

export function Freshness({ state }: { state: ProjectionState }) {
  if (state.kind === 'loading' || state.kind === 'unavailable') {
    return <div className="freshness freshness-stale"><span>{state.kind === 'loading' ? 'Loading projection' : 'Projection unavailable'}</span><small>{state.kind === 'loading' ? 'Retrieving current data' : 'No projection has been loaded'}</small></div>;
  }
  const stale = state.kind === 'stale';
  return <div className={`freshness ${stale ? 'freshness-stale' : ''}`}><span>{stale ? 'Stale projection' : 'Current projection'}</span><small>{formatTimestamp(state.lastUpdatedAt)}</small></div>;
}

export function formatTimestamp(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? 'Unknown update time' : date.toLocaleString();
}
