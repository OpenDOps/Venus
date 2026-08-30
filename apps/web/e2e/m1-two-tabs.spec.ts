import { expect, test, type Page } from '@playwright/test';
import { expectedCollaborationWs } from './keck-ws';

const SEED_TITLE = 'Venus';
const SEED_H1 = 'Why Venus';
const NOTE = 'affine-note affine-paragraph rich-text';
const OUTLINE = 'affine-outline-panel';
const OUTLINE_H1 = '[data-testid="outline-block-preview-h1"]';
const MD_PANE = '[data-testid="venus-md-pane"]';
const KECK_WS = expectedCollaborationWs();

test.describe.configure({ mode: 'serial' });

async function waitForHydrated(page: Page) {
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
  await page.locator(OUTLINE).waitFor({ timeout: 30_000 });
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
  await expect(page.locator(MD_PANE)).toContainText(SEED_H1, {
    timeout: 30_000,
  });
  return pageErrors;
}

async function focusNote(page: Page) {
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
}

function collectKeckSockets(page: Page) {
  const urls: string[] = [];
  page.on('websocket', (ws) => {
    urls.push(ws.url());
  });
  return urls;
}

test.beforeAll(async () => {
  try {
    await fetch('http://127.0.0.1:3000/', {
      signal: AbortSignal.timeout(3000),
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (/ECONNREFUSED|fetch failed|AbortError|TimeoutError/i.test(msg)) {
      throw new Error(
        `keck is not up on :3000 (${msg}). Start with pnpm sync:up from the repo root.`,
      );
    }
  }
});

test('A typing appears in B without reload', async ({ page, context }) => {
  const pageA = page;
  const wsA = collectKeckSockets(pageA);
  await waitForHydrated(pageA);

  const pageB = await context.newPage();
  const wsB = collectKeckSockets(pageB);
  await waitForHydrated(pageB);

  expect(
    wsA.find((u) => u.includes('/collaboration/venus-m0')),
    'tab A must open a keck websocket',
  ).toBe(KECK_WS);
  expect(
    wsB.find((u) => u.includes('/collaboration/venus-m0')),
    'tab B must open a keck websocket',
  ).toBe(KECK_WS);

  await focusNote(pageA);
  const fromA = `from-a-${Date.now()}`;
  await pageA.keyboard.type(fromA);
  await expect(pageA.locator(NOTE).first()).toContainText(fromA);

  // Do not `getByText` the whole page: the markdown pane repeats the note.
  await expect(pageB.locator(NOTE).first()).toContainText(fromA, {
    timeout: 10_000,
  });
});

test('both tabs show the same seed once', async ({ page, context }) => {
  const pageA = page;
  await waitForHydrated(pageA);
  const pageB = await context.newPage();
  await waitForHydrated(pageB);

  for (const tab of [pageA, pageB]) {
    await expect(tab.locator('doc-title')).toContainText(SEED_TITLE);
    await expect(tab.locator(OUTLINE_H1)).toHaveCount(1);
    await expect(tab.locator(OUTLINE_H1)).toContainText(SEED_H1);
  }
});

test('B typing appears in A without reload', async ({ page, context }) => {
  const pageA = page;
  await waitForHydrated(pageA);
  const pageB = await context.newPage();
  await waitForHydrated(pageB);

  await focusNote(pageB);
  const fromB = `from-b-${Date.now()}`;
  await pageB.keyboard.type(fromB);
  await expect(pageB.locator(NOTE).first()).toContainText(fromB);

  await expect(pageA.locator(NOTE).first()).toContainText(fromB, {
    timeout: 10_000,
  });
});
