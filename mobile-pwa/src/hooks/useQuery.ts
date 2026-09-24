import { useQuery, type UseQueryOptions } from '@tanstack/react-query';
import type { QueryResponse } from '../types/query';

export const observerQueryKeys = {
  dashboard: () => ['observer', 'dashboard'] as const,
  missions: (filters: Record<string, unknown> = {}) =>
    ['observer', 'missions', JSON.stringify(filters)] as const,
  mission: (id: string) => ['observer', 'mission', id] as const,
  tasks: (filters: Record<string, unknown> = {}) =>
    ['observer', 'tasks', JSON.stringify(filters)] as const,
  task: (id: string) => ['observer', 'task', id] as const,
  notifications: (filters: Record<string, unknown> = {}) =>
    ['observer', 'notifications', JSON.stringify(filters)] as const,
  approvals: (filters: Record<string, unknown> = {}) =>
    ['observer', 'approvals', JSON.stringify(filters)] as const,
  auditLog: (id: string) => ['observer', 'audit-log', id] as const,
};

export function useObserverQuery<T>(
  queryKey: readonly string[],
  queryFn: () => Promise<QueryResponse<T>>,
  options?: Partial<UseQueryOptions<QueryResponse<T>>>,
) {
  return useQuery<QueryResponse<T>>({
    queryKey,
    queryFn,
    refetchOnWindowFocus: true,
    staleTime: 60_000,
    retry: 1,
    networkMode: 'always',
    ...options,
  });
}