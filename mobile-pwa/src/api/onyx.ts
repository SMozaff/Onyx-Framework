/**
 * ObserverHttpGateway — the only network entry point the PWA uses
 * (`pwa-observer-contract.md` "Intended HTTP gateway"). Read-only by
 * construction: there is deliberately no generic command/mutation helper.
 *
 * Every method maps to a contract-listed route. `query` takes an
 * *encoded* envelope (see `encodeEnvelope`); `registerPush`/`unregisterPush`
 * and `downloadFile` are the Phase 1.2 routes.
 */

import { apiClient } from './client';
import { useAuthStore } from '../stores/authStore';
import type { QueryEnvelope, QueryResponse } from '../types/query';
import type {
  AuthUser,
  FileDownload,
  LoginResponse,
  LogoutRequest,
  PushSubscriptionPayload,
  PushSubscriptionResponse,
} from '../types/api';
import { isContentHash } from '../utils/validation';

/** Base64url-encodes a JSON envelope the way web-ui does. */
export function encodeEnvelope(envelope: QueryEnvelope): string {
  const bytes = new TextEncoder().encode(JSON.stringify(envelope));
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
}

/** Builds an envelope carrying the authenticated user's organization. */
export function buildQueryEnvelope(
  queryType: string,
  filters: Record<string, unknown> = {},
  options: Partial<QueryEnvelope> = {},
): QueryEnvelope {
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

export const observerApi = {
  /** `POST /api/auth/login` — always authenticates as the observer class. */
  authenticate: (username: string, password: string) =>
    apiClient.post<LoginResponse>('/api/auth/login', {
      username,
      password,
      client_type: 'mobile_observer',
    }),

  /** `POST /api/auth/refresh` — preserves the observer classification. */
  refresh: (refreshToken: string) =>
    apiClient.post<LoginResponse>('/api/auth/refresh', { refresh_token: refreshToken }),

  /** `POST /api/auth/logout` — revokes the presented refresh token. */
  logout: (body: LogoutRequest) => apiClient.post('/api/auth/logout', body),

  /** `GET /api/query?envelope=<base64url>` — observer projections. */
  query: async <T>(queryType: string, filters: Record<string, unknown> = {}, options: Partial<QueryEnvelope> = {}): Promise<QueryResponse<T>> => {
    const envelope = encodeEnvelope(buildQueryEnvelope(queryType, filters, options));
    const response = await apiClient.get<QueryResponse<T>>('/api/query', {
      params: { envelope },
    });
    return response.data;
  },

  /** `GET /api/files/:content_hash` — authorized content-addressed download. */
  downloadFile: async (contentHash: string): Promise<FileDownload> => {
    if (!isContentHash(contentHash)) {
      throw new Error('content hash must be a lowercase hex string');
    }
    const response = await apiClient.get<Blob>(`/api/files/${contentHash}`, {
      responseType: 'blob',
    });
    return {
      contentHash,
      blob: response.data,
      fileName: contentHash,
      size: Number(response.headers['content-length'] ?? 0),
    };
  },

  /** `POST /api/push/subscriptions` — register/upsert the caller's subscription. */
  registerPush: (subscription: PushSubscriptionPayload) =>
    apiClient.post<PushSubscriptionResponse>('/api/push/subscriptions', subscription),

  /** `DELETE /api/push/subscriptions/:id` — remove the caller's subscription. */
  unregisterPush: (subscriptionId: string) =>
    apiClient.delete<void>(`/api/push/subscriptions/${subscriptionId}`),
};

// Contract-frozen name: the desktop client calls its dialect
// "HttpGateway"; the PWA contract calls this one ObserverHttpGateway.
export type ObserverHttpGateway = typeof observerApi;
export type { AuthUser };