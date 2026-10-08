import { afterEach, describe, expect, it, vi } from 'vitest';
import { normalizeError } from '../../src/utils/errorHandler';
import { API_BASE } from '../../src/api/client';

function axiosErrorLike(response: { status?: number; data?: unknown } | undefined) {
  return {
    isAxiosError: true,
    response,
  } as never;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('normalizeError', () => {
  it('maps no-response to NETWORK_UNAVAILABLE and notifies', () => {
    const dispatched = vi.fn();
    window.addEventListener('onyx:network-error', dispatched);
    const normalized = normalizeError(axiosErrorLike(undefined));
    expect(normalized.code).toBe('NETWORK_UNAVAILABLE');
    expect(normalized.action).toBe('retry');
    expect(dispatched).toHaveBeenCalledTimes(1);
    window.removeEventListener('onyx:network-error', dispatched);
  });

  it('pulls code, correlation_id, and safe message from a CommandError body', () => {
    const normalized = normalizeError(
      axiosErrorLike({
        status: 401,
        data: {
          error: {
            code: 'SESSION_EXPIRED',
            category: 'AUTHORITY',
            retryability: 'NON_RETRYABLE',
            safe_details: { message: 'Session expired' },
            correlation_id: 'corr-123',
          },
        },
      }),
    );
    expect(normalized.code).toBe('SESSION_EXPIRED');
    expect(normalized.action).toBe('sign-in');
    expect(normalized.message).toBe('Session expired');
    expect(normalized.diagnosticId).toBe('corr-123');
  });

  it('maps 500 to retryable SERVICE_UNAVAILABLE', () => {
    const normalized = normalizeError(axiosErrorLike({ status: 500 }));
    expect(normalized.code).toBe('SERVICE_UNAVAILABLE');
    expect(normalized.retryable).toBe(true);
    expect(normalized.action).toBe('retry');
  });

  it('falls back to UNEXPECTED for non-axios errors', () => {
    expect(normalizeError(new Error('boom')).code).toBe('UNEXPECTED');
  });
});

describe('api base url', () => {
  it('defaults to the shared web-ui default', () => {
    expect(API_BASE).toBe('http://127.0.0.1:3000');
  });
});