import type { Page, Route } from '@playwright/test';

export async function seedAuthenticatedSession(page: Page): Promise<void> {
  await page.addInitScript(() => {
    sessionStorage.setItem('onyx_access_token', 'browser-test-token');
    sessionStorage.setItem('onyx_refresh_token', 'browser-test-refresh');
    sessionStorage.setItem('onyx_user', JSON.stringify({
      id: 'browser-test-user',
      username: 'audit.operator',
      organization_id: 'browser-test-org',
      organization_display_name: 'Browser Test Operations',
    }));
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
      last_updated_at: '2026-08-20T00:00:00.000Z',
      is_stale: false,
    },
  };
}

export async function fulfillQuery(page: Page, body: unknown): Promise<void> {
  await page.route((url) => new URL(url).pathname === '/api/query', async (route: Route) => {
    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
  });
}

export async function failQuery(page: Page, status = 500): Promise<void> {
  await page.route((url) => new URL(url).pathname === '/api/query', async (route: Route) => {
    await route.fulfill({ status, contentType: 'application/json', body: JSON.stringify({ error: { code: 'TEST_FAILURE' } }) });
  });
}
