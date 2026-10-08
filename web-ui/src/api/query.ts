import { apiClient } from './client';
import { useAuthStore } from '../stores/authStore';
import type { QueryEnvelope, QueryResponse } from '../types/query';

export function encodeEnvelope(envelope: QueryEnvelope): string {
  const bytes = new TextEncoder().encode(JSON.stringify(envelope));
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
}

export function buildQueryEnvelope(queryType: string, filters: Record<string, unknown> = {}, options: Partial<QueryEnvelope> = {}): QueryEnvelope {
  const user = useAuthStore.getState().user;
  if (!user) throw new Error('Authenticated user is required for queries.');
  return {
    query_id: crypto.randomUUID(),
    query_type: queryType,
    schema_version: '1.0',
    organization_id: user.organization_id,
    filters,
    limit: options.limit ?? 100,
    cursor: options.cursor,
    sort_by: options.sort_by,
    sort_order: options.sort_order ?? 'desc',
  };
}

export async function executeQuery<T>(queryType: string, filters: Record<string, unknown> = {}, options: Partial<QueryEnvelope> = {}): Promise<QueryResponse<T>> {
  const envelope = buildQueryEnvelope(queryType, filters, options);
  const response = await apiClient.get<QueryResponse<T>>('/api/query', {
    params: { envelope: encodeEnvelope(envelope) },
  });
  return response.data;
}
