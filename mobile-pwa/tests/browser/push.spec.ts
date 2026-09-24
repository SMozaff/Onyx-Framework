import { expect, test, type Page, type Route } from '@playwright/test';
import { queryResponse, seedAuthenticatedSession } from './fixtures/onyx';

const ENDPOINT = 'https://push.example.test/deterministic-endpoint';
const P256DH = 'cGFyZW50a2V5';
const AUTH = 'YXV0aHNlY3JldA';

/** Returns a deterministic PushSubscription from the browser push APIs. */
async function stubPushManager(page: Page): Promise<void> {
  await page.addInitScript(({ endpoint }) => {
    const decode = (base64Url: string) => {
      const bin = atob(base64Url.replaceAll('-', '+').replaceAll('_', '/'));
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
      return bytes;
    };
    const ctor = window.PushManager as unknown as {
      prototype: { subscribe: unknown; getSubscription: unknown };
    };
    if (!ctor) return;
    ctor.prototype.getSubscription = async () => null;
    ctor.prototype.subscribe = async () => ({
      endpoint,
      getKey: (kind: string) => decode(kind === 'p256dh' ? 'cGFyZW50a2V5' : 'YXV0aHNlY3JldA'),
      unsubscribe: async () => true,
    });
  }, { endpoint: ENDPOINT });
}

async function stubEmptyQueries(page: Page): Promise<void> {
  await page.route('**/api/query', async (route: Route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(queryResponse([])),
    });
  });
}

test('observer enables push delivery with VAPID keys and disables it again', async ({ page, context }) => {
  await seedAuthenticatedSession(page);
  await stubPushManager(page);
  await context.grantPermissions(['notifications'], { origin: 'http://localhost:5174' });
  await stubEmptyQueries(page);

  let registered: unknown = null;
  await page.route('**/api/push/subscriptions', async (route: Route) => {
    if (route.request().method() === 'POST') {
      registered = route.request().postDataJSON();
      await route.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'sub-1',
          user_id: 'browser-test-user',
          organization_id: 'browser-test-org',
          endpoint: ENDPOINT,
          platform: 'pwa',
          created_at: 1,
        }),
      });
      return;
    }
    await route.abort();
  });

  const deleted: string[] = [];
  await page.route('**/api/push/subscriptions/*', async (route: Route) => {
    deleted.push(route.request().url());
    await route.fulfill({ status: 204, body: '' });
  });

  await page.goto('/notifications');
  await expect(page.getByRole('button', { name: 'Enable notifications' })).toBeEnabled();
  await page.getByRole('button', { name: 'Enable notifications' }).click();

  await expect(page.getByRole('button', { name: 'Disable' })).toBeVisible();
  await expect(page.getByText(ENDPOINT)).toBeVisible();
  await expect.poll(() => registered).toEqual({
    endpoint: ENDPOINT,
    p256dh: P256DH,
    auth: AUTH,
    platform: 'pwa',
  });

  await page.getByRole('button', { name: 'Disable' }).click();
  await expect(page.getByRole('button', { name: 'Enable notifications' })).toBeVisible();
  expect(deleted).toHaveLength(1);
  expect(deleted[0]).toContain('/api/push/subscriptions/sub-1');
});

test('service worker displays a simulated push and routes the click', async ({ page, context }) => {
  await seedAuthenticatedSession(page);
  await context.grantPermissions(['notifications'], { origin: 'http://localhost:5174' });
  await stubEmptyQueries(page);

  const pageErrors: string[] = [];
  page.on('console', (message) => {
    if (message.type() === 'error') pageErrors.push(message.text());
  });

  const workerPromise = context.waitForEvent('serviceworker');
  await page.goto('/missions');
  const firstWorker = await workerPromise;

  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.reload();
  await page.evaluate(() => navigator.serviceWorker.ready);

  // Only after a reload does the page have a controller, which the SW
  // navigates when the notification is clicked.
  await expect
    .poll(() => page.evaluate(() => Boolean(navigator.serviceWorker.controller)), { timeout: 10_000 })
    .toBe(true);
  const worker = context.serviceWorkers()[0] ?? firstWorker;

  await worker.evaluate(
    (payload) => {
      const scope = self as unknown as ServiceWorkerGlobalScope;
      scope.dispatchEvent(new PushEvent('push', { data: JSON.stringify(payload) }));
    },
    { title: 'Simulated alert', message: 'Deterministic body.', url: '/notifications' },
  );

  await expect
    .poll(
      () =>
        page.evaluate(async () =>
          (await navigator.serviceWorker.ready).getNotifications().then((ns) =>
            ns.map((n) => ({ title: n.title, body: n.body })),
          ),
        ),
      { timeout: 10_000 },
    )
    .toContainEqual({ title: 'Simulated alert', body: 'Deterministic body.' });

  await worker.evaluate(async () => {
    const scope = self as unknown as ServiceWorkerGlobalScope;
    const [notification] = await scope.registration.getNotifications();
    scope.dispatchEvent(new NotificationEvent('notificationclick', { notification, action: '' }));
  });

  await expect(page).toHaveURL(/\/notifications$/);
  await expect(page.getByText('Web Push delivery')).toBeVisible();
});