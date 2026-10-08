/**
 * HTTP surface for the ObserverClient; mirrors the frozen
 * `pwa-observer-contract.md` gateway list. Types follow the wire shape of
 * `docs/api/openapi.json` (same derivation rule as `web-ui`).
 */

import type { CommandError } from './query';

export interface AuthUser {
  id: string;
  username: string;
  organization_id: string;
  /** Optional while older API servers are rolling out the additive field. */
  organization_display_name?: string;
}

export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: AuthUser;
}

export interface LogoutRequest {
  refresh_token: string;
}

/** `POST /api/push/subscriptions` request body — browser `PushSubscription`. */
export interface PushSubscriptionPayload {
  endpoint: string;
  p256dh: string;
  auth: string;
  platform: string;
}

/** `POST /api/push/subscriptions` response body. */
export interface PushSubscriptionResponse {
  id: string;
  user_id: string;
  organization_id: string;
  endpoint: string;
  platform: string;
  created_at: number;
}

/** `GET /api/files/:content_hash` metadata we negotiate in the client. */
export interface FileDownload {
  contentHash: string;
  blob: Blob;
  fileName: string;
  size: number;
}

export interface ApiErrorBody {
  error: CommandError;
}