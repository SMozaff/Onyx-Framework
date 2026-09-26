import { expect, test, type Page, type Route } from '@playwright/test';
import { captureRequests, queryResponse, seedAuthenticatedSession } from './fixtures/onyx';

const mission = {
  id: 'mission-1',
  version: 1,
  lifecycle_epoch: 0,
  authority_epoch: 0,
  name: 'Browser mission',
  summary: 'Synthetic mission for deterministic browser validation.',
  status: 'active',
  owner: 'audit.observer',
  priority: 'normal',
  progress: 25,
  updated_at: '2026-09-24T00:00:00.000Z',
};

const approval = {
  id: 'approval-1',
  version: 1,
  lifecycle_epoch: 0,
  authority_epoch: 0,
  title: 'Approve synthetic evidence',
  description: 'Synthetic approval.',
  status: 'pending',
  requested_by: 'audit.observer',
  target_id: 'task-1',
  target_type: 'task',
  created_at: '2026-09-24T00:00:00.000Z',
  decided_at: null,
  decision_reason: null,
  web_action_permitted: true,
};

const notification = {
  id: 'notification-1',
  version: 1,
  lifecycle_epoch: 0,
  authority_epoch: 0,
  title: 'Synthetic alert',
  message: 'Reconfigured.',
  priority: 'normal',
  status: 'unacknowledged',
  source_id: 'mission-1',
  source_type: 'mission',
  created_at: '2026-09-24T00:00:00.000Z',
  acknowledged_at: null,
};

const dashboard = {
  missions: 1,
  active_missions: 1,
  tasks: 0,
  blocked_tasks: 0,
  unread_notifications: 1,
  pending_approvals: 1,
  activity: [{ id: 'mission-1', type: 'mission', title: 'Browser mission', status: 'active', updated_at: '2026-09-24T00:00:00.000Z' }],
};

function decodeEnvelope(envelope: string): { query_type: string } {
  const json = Buffer.from(envelope.replaceAll('-', '+').replaceAll('_', '/'), 'base64url').toString('utf8');
  return JSON.parse(json) as { query_type: string };
}

/** Routes /api/query by the query_type embedded in the base64url envelope. */
async function fulfillQueriesByType(page: Page): Promise<void> {
  await page.route(
    (url) => new URL(url).pathname === '/api/query',
    async (route: Route) => {
      const params = new URL(route.request().url()).searchParams;
      const envelope = params.get('envelope') ?? '';
      let body: unknown = queryResponse([]);
      try {
        const { query_type } = decodeEnvelope(envelope);
        if (query_type === 'dashboard.summary') body = queryResponse([dashboard], 1);
        if (query_type === 'mission.list') body = queryResponse([mission], 1);
        if (query_type === 'approval.list') body = queryResponse([approval], 1);
        if (query_type === 'notification.list') body = queryResponse([notification], 1);
      } catch {
        /* fall through with empty data */
      }
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
    },
  );
}

test('observer login authenticates with the observer client class', async ({ page }) => {
  let capturedBody!: unknown;
  await page.route('**/api/auth/login', async (route: Route) => {
    capturedBody = route.request().postDataJSON();
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        access_token: 'browser-access',
        refresh_token: 'browser-refresh',
        expires_in: 3600,
        user: { id: 'u-1', username: 'audit.observer', organization_id: 'org-1' },
      }),
    });
  });
  await fulfillQueriesByType(page);

  await page.goto('/login');
  await page.getByLabel('Username').fill('audit.observer');
  await page.getByLabel('Password').fill('secret');
  await page.getByRole('button', { name: 'Sign in' }).click();

  await expect(page).toHaveURL(/\/dashboard$/);
  expect(capturedBody).toEqual({
    username: 'audit.observer',
    password: 'secret',
    client_type: 'mobile_observer',
  });
  await expect(page.getByText('read-only', { exact: true })).toBeVisible();
});

test('seeded observer session renders missions list and links to detail', async ({ page }) => {
  await seedAuthenticatedSession(page);
  await fulfillQueriesByType(page);

  await page.goto('/missions');
  await expect(page.getByText('Browser mission')).toBeVisible();
  await expect(page.getByLabel(/Status: Active/)).toBeVisible();
  const link = page.getByRole('link', { name: /Browser mission/ });
  await expect(link).toHaveAttribute('href', '/mission/mission-1');
});

test('observer sees decisions as read-only status, never approve/reject', async ({ page }) => {
  await seedAuthenticatedSession(page);
  await fulfillQueriesByType(page);

  await page.goto('/approvals');
  await expect(page.getByText('Approve synthetic evidence')).toBeVisible();
  await expect(page.getByText(/decision-free by contract/i)).toBeVisible();
  await expect(page.getByRole('button', { name: /approve|reject/i })).toHaveCount(0);

  await page.goto('/notifications');
  await expect(page.getByText('Synthetic alert')).toBeVisible();
  await expect(page.getByRole('button', { name: /acknowledge/i })).toHaveCount(0);
});

test('file download carries the bearer token and reports bytes', async ({ page }) => {
  await seedAuthenticatedSession(page);
  const hash = 'ab12ef'.repeat(9);
  const downloads = await captureRequests(page, `/api/files/${hash}`, {}, 200);

  await page.goto(`/files/${hash}`);
  await expect(page.getByRole('button', { name: 'Download file' })).toBeVisible();
  await page.getByRole('button', { name: 'Download file' }).click();

  await expect(page.getByText(/bytes/)).toBeVisible();
  expect(downloads.length).toBeGreaterThanOrEqual(1);
  expect(downloads[0].headers.authorization).toBe('Bearer browser-test-token');
});

test('unknown routes fall back to the not-found view', async ({ page }) => {
  await seedAuthenticatedSession(page);
  await page.goto('/definitely-not-an-observer-view');
  await expect(page.getByText('Page not found')).toBeVisible();
});