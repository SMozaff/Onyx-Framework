import { defineConfig } from '@playwright/test';

/* MIGRATION_PLAN Phase 5.2: standalone/offline acceptance.
 * Serves the PRODUCTION build (dist/) — the shell is precached into the
 * service worker at install time by scripts/render-sw.mjs, so an offline
 * relaunch works without a prior online visit. Kept as a separate config
 * so the dev-suite (playwright.config.ts) is unaffected.
 *
 * Requires `npm run build` first (npm script `test:browser:offline`
 * chains it via the `pretest:` lifecycle hook). */
export default defineConfig({
  testDir: './tests/offline',
  fullyParallel: false,
  reporter: 'html',
  timeout: 30_000,
  use: {
    baseURL: 'http://localhost:5175',
    channel: 'chrome',
    trace: 'on-first-retry',
  },
  webServer: {
    command: 'npm run preview -- --port 5175 --strictPort',
    url: 'http://localhost:5175',
    reuseExistingServer: !process.env.CI,
    env: {
      VITE_ENABLE_SW_DEV: 'true',
      VITE_API_BASE: 'http://localhost:5175',
    },
  },
});