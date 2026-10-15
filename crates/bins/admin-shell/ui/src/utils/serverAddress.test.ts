import { describe, expect, it } from 'vitest';
import { getBackendAddress, getServerAddress, isSecureEnoughForProduction } from './serverAddress';

const CLOUD_API_BASE = 'https://onyx-framework.soheil-mozaffari.workers.dev';

describe('cloud-only API endpoint', () => {
  it('uses the canonical deployed API endpoint', () => {
    expect(getServerAddress()).toBe(CLOUD_API_BASE);
    expect(getBackendAddress()).toBe(CLOUD_API_BASE);
  });

  it('accepts only the canonical HTTPS endpoint for production', () => {
    expect(isSecureEnoughForProduction(CLOUD_API_BASE)).toBe(true);
    expect(isSecureEnoughForProduction(`${CLOUD_API_BASE}/`)).toBe(true);
    expect(isSecureEnoughForProduction('http://localhost:3000')).toBe(false);
    expect(isSecureEnoughForProduction('http://127.0.0.1:3000')).toBe(false);
    expect(isSecureEnoughForProduction('https://example.com')).toBe(false);
    expect(isSecureEnoughForProduction('not a URL')).toBe(false);
  });
});
