import { defineConfig, devices } from '@playwright/test';

const isM1 = process.env.PLAYWRIGHT_M1 === '1';
const composeWeb = process.env.PLAYWRIGHT_BASE_URL;
const syncUrl =
  process.env.VITE_SYNC_URL ??
  'ws://127.0.0.1:3000/collaboration/venus-m0';

if (composeWeb && !isM1) {
  throw new Error(
    'PLAYWRIGHT_BASE_URL is Compose web (sync). Use PLAYWRIGHT_M1=1.',
  );
}

export default defineConfig({
  testDir: './e2e',
  testMatch: isM1 ? 'm1-*.spec.ts' : 'm0-*.spec.ts',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 0,
  timeout: 180_000,
  workers: isM1 ? 1 : undefined,
  use: {
    baseURL:
      composeWeb ??
      (isM1 ? 'http://127.0.0.1:5174' : 'http://127.0.0.1:5173'),
    trace: 'on-first-retry',
    viewport: { width: 1280, height: 800 },
  },
  webServer: composeWeb
    ? undefined
    : isM1
      ? {
          command: 'vite --host 127.0.0.1 --port 5174 --strictPort',
          url: 'http://127.0.0.1:5174',
          reuseExistingServer: false,
          timeout: 180_000,
          env: {
            VITE_SYNC_URL: syncUrl,
          },
        }
      : {
          command: 'vite --host 127.0.0.1 --port 5173 --strictPort',
          url: 'http://127.0.0.1:5173',
          reuseExistingServer: !process.env.CI,
          timeout: 180_000,
        },
  projects: [{ name: isM1 ? 'm1' : 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
