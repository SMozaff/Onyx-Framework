import type { Page, Route } from '@playwright/test';

export async function seedAuthenticatedSession(page: Page): Promise<void> {
  await page.addInitScript(() => {
    sessionStorage.setItem('onyx_observer_access_token', 'browser-test-token');
    sessionStorage.setItem('onyx_observer_refresh_token', 'browser-test-refresh');
    sessionStorage.setItem(
      'onyx_observer_user',
      JSON.stringify({
        id: 'browser-test-user',
        username: 'audit.observer',
        organization_id: 'browser-test-org',
      }),
    );
  });
}

export function queryResponse(data: unknown[], totalCount = data.length) {
  return {
    query_id: 'browser-query',
    data,
    has_more: false,
    next_cursor: null,
    total_count: totalCount,
    freshness: {
      projection_version: 1,
      last_updated_at: '2026-09-24T00:00:00.000Z',
      is_stale: false,
    },
  };
}

export async function fulfillQuery(page: Page, body: unknown): Promise<void> {
  await page.route(
    (url) => new URL(url).pathname === '/api/query',
    async (route: Route) => {
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
    },
  );
}

export interface CapturedRequest {
  url: string;
  headers: Record<string, string>;
}

/** Intercepts a path and records the requests for later assertions. */
export async function captureRequests(page: Page, pathname: string, body: unknown, status = 200): Promise<CapturedRequest[]> {
  const captured: CapturedRequest[] = [];
  await page.route(
    (url) => new URL(url).pathname === pathname,
    async (route: Route) => {
      captured.push({ url: route.request().url(), headers: route.request().headers() });
      await route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
    },
  );
  return captured;
}