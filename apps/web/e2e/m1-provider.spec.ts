import { expect, test, type Page } from '@playwright/test';
import { expectedCollaborationWs, assertHubOn3000 } from './keck-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
const HUB_WS = expectedCollaborationWs();

async function waitForEditor(page: Page) {
  const pageErrors: string[] = [];
  const failOnError = new Promise<never>((_, reject) => {
    page.on('pageerror', (err) => {
      pageErrors.push(String(err));
      reject(err);
    });
  });
  await Promise.race([
    (async () => {
      await page.goto('/', { waitUntil: 'domcontentloaded' });
      await page.locator('affine-editor-container').waitFor({
        timeout: 120_000,
      });
    })(),
    failOnError,
  ]).catch((err) => {
    throw new Error(
      `affine-editor-container did not mount.\n${pageErrors.join('\n') || String(err)}`,
    );
  });
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });
  return pageErrors;
}

test.beforeAll(async () => {
  await assertHubOn3000();
});

test('octobase kind (hub wire alias) and AFFiNE websocket when VITE_SYNC_URL is set', async ({
  page,
}) => {
  const wsUrls: string[] = [];
  page.on('websocket', (ws) => {
    wsUrls.push(ws.url());
  });
  await page.addInitScript(() => {
    const Orig = window.WebSocket;
    window.WebSocket = class extends Orig {
      constructor(url: string | URL, protocols?: string | string[]) {
        super(url, protocols);
        window.__VENUS_WS_PROTOCOLS__ = protocols;
      }
    };
  });

  await waitForEditor(page);

  const kind = await page.evaluate(() => window.__VENUS_PROVIDER_KIND__);
  expect(kind, 'must not stay on memory while VITE_SYNC_URL is set').toBe(
    'octobase',
  );

  await expect
    .poll(() => wsUrls.find((u) => u.includes(HUB_WS)))
    .toBe(HUB_WS);

  const protocols = await page.evaluate(() => window.__VENUS_WS_PROTOCOLS__);
  expect(protocols).toEqual(['AFFiNE']);
});
