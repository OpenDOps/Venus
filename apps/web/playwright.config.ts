import { COLLABORATION_PATH } from './src/host/ids.js';
import { defineConfig, devices } from '@playwright/test';

const isM1 = process.env.PLAYWRIGHT_M1 === '1';
const isM3 = process.env.PLAYWRIGHT_M3 === '1';
const isM4 = process.env.PLAYWRIGHT_M4 === '1';
const composeWeb = process.env.PLAYWRIGHT_BASE_URL;
const syncUrl =
  process.env.VITE_SYNC_URL ??
  `ws://127.0.0.1:28710${COLLABORATION_PATH}`;
const sidecarUrl =
  process.env.VITE_SIDECAR_URL ?? 'http://127.0.0.1:28720';

if (composeWeb && !isM1 && !isM3 && !isM4) {
  throw new Error(
    'PLAYWRIGHT_BASE_URL is Compose web (sync). Use PLAYWRIGHT_M1=1, PLAYWRIGHT_M3=1, or PLAYWRIGHT_M4=1.',
  );
}

const gated = isM4 ? 'm4' : isM3 ? 'm3' : isM1 ? 'm1' : null;

export default defineConfig({
  testDir: './e2e',
  testMatch: isM4
    ? 'm4-*.spec.ts'
    : isM3
      ? 'm3-*.spec.ts'
      : isM1
        ? 'm1-*.spec.ts'
        : '{m0,m2}-*.spec.ts',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 0,
  timeout: 180_000,
  workers: gated ? 1 : undefined,
  use: {
    baseURL:
      composeWeb ??
      (isM3 || isM1 || isM4
        ? 'http://127.0.0.1:5174'
        : 'http://127.0.0.1:5173'),
    trace: 'on-first-retry',
    viewport: { width: 1280, height: 800 },
  },
  webServer: composeWeb
    ? undefined
    : isM3
      ? {
          command: 'vite --host 127.0.0.1 --port 5174 --strictPort',
          url: 'http://127.0.0.1:5174',
          reuseExistingServer: false,
          timeout: 180_000,
          env: {
            VITE_SYNC_URL: syncUrl,
            VITE_SIDECAR_URL: sidecarUrl,
            VITE_DEBUG: '1',
          },
        }
      : isM4
        ? {
            command: 'vite --host 127.0.0.1 --port 5174 --strictPort',
            url: 'http://127.0.0.1:5174',
            reuseExistingServer: false,
            timeout: 180_000,
            env: {
              VITE_SYNC_URL: syncUrl,
              VITE_SIDECAR_URL: sidecarUrl,
              VITE_DEBUG: '1',
              VITE_TESTIDS: '1',
            },
          }
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
  projects: [
    {
      name: gated ?? 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
