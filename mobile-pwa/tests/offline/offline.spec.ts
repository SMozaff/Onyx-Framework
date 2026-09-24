import { expect, test } from '@playwright/test';
import { queryResponse, seedAuthenticatedSession } from '../browser/fixtures/onyx';

const mission = {
  id: 'mission-offline-1',
  version: 1,
  lifecycle_epoch: 0,
  authority_epoch: 0,
  name: 'Offline mission',
  summary: 'Served from the service-worker cache while the device is offline.',
  status: 'active',
  owner: 'audit.observer',
  priority: 'normal',
  progress: 25,
  updated_at: '2026-09-24T00:00:00.000Z',
};

/** Deterministic UUID so the query envelope (and thus the cached request URL) is stable. */
const FIXED_UUID = '00000000-0000-4000-8000-00000000ff01';

/** Re-derives the app's envelope for mission.list, mirroring buildQueryEnvelope defaults. */
function missionListQueryUrl(): string {
  const envelope = {
    query_id: FIXED_UUID,
    query_type: 'mission.list',
    schema_version: '1.0',
    organization_id: 'browser-test-org',
    filters: {},
    limit: 100,
    sort_order: 'desc',
  };
  const encoded = Buffer.from(JSON.stringify(envelope)).toString('base64url');
  return `/api/query?envelope=${encoded}`;
}

test('after install the shell and last snapshot render without a network', async ({ page, context }) => {
  await page.addInitScript(() => {
    crypto.randomUUID = () => '00000000-0000-4000-8000-00000000ff01';
  });
  await seedAuthenticatedSession(page);

  const queryUrl = missionListQueryUrl();

  await page.goto('/missions');
  await page.waitForFunction(() => navigator.serviceWorker?.controller != null);
  await page.waitForFunction(async () =>
    (await caches.keys()).some((key) => key.startsWith('onyx-observer-content-')),
  );

  const shellCached = await page.evaluate(async () => {
    const key = (await caches.keys()).find((k) => k.startsWith('onyx-observer-content-'));
    if (!key) return false;
    const cache = await caches.open(key);
    return (await cache.match('/')) != null;
  });
  expect(shellCached).toBe(true);

  const body = JSON.stringify(queryResponse([mission], 1));
  await page.evaluate(async ({ url, body: raw }) => {
    const key = (await caches.keys()).find((k) => k.startsWith('onyx-observer-content-'));
    if (!key) throw new Error('content cache missing');
    const cache = await caches.open(key);
    await cache.put(url, new Response(raw, { headers: { 'content-type': 'application/json' } }));
  }, { url: queryUrl, body });

  await context.setOffline(true);
  await page.reload();

  await expect(page.getByText('ONYX Observer')).toBeVisible();
  await expect(page.getByTestId('offline-banner')).toBeVisible();
  await expect(page.getByText('Offline mission')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
});