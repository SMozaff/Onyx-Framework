import { describe, expect, it } from 'vitest';
import { isBase64Url, isContentHash, isHttpsEndpoint } from '../../src/utils/validation';

describe('isBase64Url', () => {
  it('accepts URL-safe base64 without padding', () => {
    expect(isBase64Url('I-1jMf3W9VqkQ6BbVzY0Nw')).toBe(true);
    expect(isBase64Url('BNcRdreALRFXTkOOUHK1EtK2wtaz5Ry4YfYCA_0QTpQtUbVlUls0VJXg7A8u-Ts1XbjhazAkj7I99e8QcYP7DkM')).toBe(true);
  });

  it('rejects padding, standard-base64 chars, and empty strings', () => {
    expect(isBase64Url('abcd==')).toBe(false);
    expect(isBase64Url('abc+def')).toBe(false);
    expect(isBase64Url('abc/def')).toBe(false);
    expect(isBase64Url('')).toBe(false);
  });
});

describe('isHttpsEndpoint', () => {
  it('accepts https endpoints with a dotted host', () => {
    expect(isHttpsEndpoint('https://fcm.googleapis.com/fcm/send/test-device-1')).toBe(true);
    expect(isHttpsEndpoint('  https://example.com/  ')).toBe(true);
  });

  it('rejects http, missing host, and hostless schemes', () => {
    expect(isHttpsEndpoint('http://fcm.googleapis.com/x')).toBe(false);
    expect(isHttpsEndpoint('https://')).toBe(false);
    expect(isHttpsEndpoint('https:///path')).toBe(false);
    expect(isHttpsEndpoint('')).toBe(false);
  });
});

describe('isContentHash', () => {
  it('accepts lowercase hex', () => {
    expect(isContentHash('ab12ef')).toBe(true);
    expect(isContentHash('0'.repeat(64))).toBe(true);
  });

  it('rejects uppercase, non-hex, and empty strings', () => {
    expect(isContentHash('AB12EF')).toBe(false);
    expect(isContentHash('not-hex')).toBe(false);
    expect(isContentHash('')).toBe(false);
  });
});