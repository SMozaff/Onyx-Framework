/**
 * Client-side mirrors of the Phase 1.2 validation rules in
 * `api_server::routes::push` and `routes::files`, so the PWA rejects
 * malformed input before a round trip. The server remains authoritative.
 */

/** URL-safe base64 (no padding) — the Web Crypto key serialization shape. */
const BASE64URL_RE = /^[A-Za-z0-9_-]+$/;

export function isBase64Url(value: string): boolean {
  return value.length > 0 && BASE64URL_RE.test(value) && value.length % 4 !== 1;
}

/** `https://` + a non-empty host — mirrors the server's structural check. */
export function isHttpsEndpoint(endpoint: string): boolean {
  const host = endpoint
    .trim()
    .replace(/^https:\/\//, '')
    .split('/')[0];
  return host.length > 0 && host.indexOf('.') !== -1;
}

/**
 * `GET /api/files/:content_hash` requires a lowercase hex hash — the wire
 * format `file_domain::value::ContentHash` validates server-side.
 */
const HEX_RE = /^[0-9a-f]+$/;

export function isContentHash(value: string): boolean {
  return value.length > 0 && HEX_RE.test(value);
}