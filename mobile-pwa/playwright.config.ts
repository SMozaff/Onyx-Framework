import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/browser',
  fullyParallel: true,
  reporter: 'html',
  timeout: 30_000,
  use: {
    baseURL: 'http://localhost:5174',
    channel: 'chrome',
    trace: 'on-first-retry',
  },
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:5174',
    reuseExistingServer: !process.env.CI,
    env: {
      VITE_ENABLE_SW_DEV: 'true',
      VITE_VAPID_PUBLIC_KEY: 'BLtIKRuighx_STM2AmutDDs6P7q2iS2hKLKLoix9PUL_MNsw_mYE7dhVbM1ac6VqgP1aa3ODVUIoKdLth74AzmA',
    },
  },
});