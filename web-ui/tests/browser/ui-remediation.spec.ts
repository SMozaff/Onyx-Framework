import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import { failQuery, fulfillQuery, queryResponse, seedAuthenticatedSession } from './fixtures/onyx';

const mission = {
  id: 'mission-1',
  version: 1,
  lifecycle_epoch: 0,
  authority_epoch: 0,
  name: 'Browser test mission',
  summary: 'Synthetic mission for deterministic browser validation.',
  status: 'active',
  owner: 'audit.operator',
  priority: 'normal',
  progress: 25,
  updated_at: '2026-08-20T00:00:00.000Z',
};

const approval = {
  id: 'approval-1',
  version: 1,
  lifecycle_epoch: 0,
  authority_epoch: 0,
  title: 'Approve synthetic evidence',
  description: 'Synthetic approval for browser validation.',
  status: 'pending',
  requested_by: 'audit.operator',
  target_id: 'task-1',
  target_type: 'task',
  created_at: '2026-08-20T00:00:00.000Z',
  decided_at: null,
  decision_reason: null,
  web_action_permitted: true,
};

test('desktop login hero keeps its explicit accessible foreground and visual baseline', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop-chromium', 'The hero is intentionally hidden at the mobile breakpoint.');
  await page.goto('/login');
  const hero = page.getByRole('region', { name: 'ONYX product introduction' });
  const heading = hero.getByRole('heading', { name: /Operational clarity/ });
  await expect(heading).toHaveCSS('color', 'rgb(255, 255, 255)');
  const results = await new AxeBuilder({ page }).include('[aria-label="ONYX product introduction"]').analyze();
  expect(results.violations).toEqual([]);
  // Font metrics vary across developer and hosted Linux images. Text is verified
  // semantically above; the visual baseline intentionally covers only the hero
  // background and layout, which must remain pixel-stable across environments.
  await page.addStyleTag({ content: '[aria-label="ONYX product introduction"] > * { visibility: hidden !important; }' });
  await expect(hero).toHaveScreenshot('login-hero-background-desktop.png', { animations: 'disabled' });
});

test('mobile drawer keeps hidden navigation out of the tab order and restores focus on Escape', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'mobile-chromium', 'This flow validates the mobile drawer only.');
  await seedAuthenticatedSession(page);
  await fulfillQuery(page, queryResponse([mission]));
  await page.goto('/missions');

  const menu = page.locator('.menu-button');
  const navigation = page.locator('#primary-navigation');
  await expect(menu).toHaveAccessibleName('Open navigation');
  await expect(navigation).toHaveAttribute('aria-hidden', 'true');
  await menu.click();
  await expect(menu).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('link', { name: 'Overview' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(navigation).toHaveAttribute('aria-hidden', 'true');
  await expect(menu).toBeFocused();
});

test('failed mission query is unavailable rather than an empty zero portfolio', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop-chromium', 'The state assertion is viewport-independent and runs once on desktop.');
  await seedAuthenticatedSession(page);
  await failQuery(page);
  await page.goto('/missions');
  await expect(page.getByRole('alert')).toContainText('Missions unavailable');
  await expect(page.getByRole('alert')).not.toContainText('0 total');
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

test('approval dialog has named modal semantics and preserves the aligned reason policy', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop-chromium', 'The dialog contract is checked once on desktop.');
  await seedAuthenticatedSession(page);
  await fulfillQuery(page, queryResponse([approval]));
  await page.goto('/approvals');
  await page.getByRole('button', { name: 'Approve' }).click();
  const dialog = page.getByRole('dialog', { name: 'Approve request' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel('Decision note (optional)')).toBeFocused();
  await expect(dialog.getByRole('button', { name: 'Confirm approval' })).toBeEnabled();
  const results = await new AxeBuilder({ page }).include('[role="dialog"]').analyze();
  expect(results.violations).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
});
