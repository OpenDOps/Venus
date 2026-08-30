import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';

const SEED_TITLE = 'Venus';
const SEED_H1 = 'Why Venus';
const NOTE = 'affine-note affine-paragraph rich-text';
const OUTLINE = 'affine-outline-panel';
const OUTLINE_H1 = '[data-testid="outline-block-preview-h1"]';
const IMAGE = 'affine-image .affine-image-container img';
const DOT_PNG = join(dirname(fileURLToPath(import.meta.url)), 'fixtures/dot.png');

test.describe.configure({ mode: 'serial' });

async function waitForHydrated(page: Page) {
  await page.addInitScript(() => {
    // Playwright intercepts showOpenFilePicker; BlockSuite then returns
    // null instead of falling back to <input type="file">.
    const w = window as Window & { showOpenFilePicker?: unknown };
    w.showOpenFilePicker = undefined;
  });
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
}

async function expectImagePixels(page: Page, timeout = 15_000) {
  const img = page.locator(IMAGE).first();
  await expect(img).toBeVisible({ timeout });
  await expect
    .poll(async () => img.evaluate((el: HTMLImageElement) => el.naturalWidth), {
      timeout,
    })
    .toBeGreaterThan(0);
}

async function insertSlashImage(page: Page, filePath: string) {
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.waitForTimeout(200);
  await page.keyboard.type('/image');
  await expect(page.locator('affine-slash-menu .slash-menu')).toBeVisible({
    timeout: 15_000,
  });
  // showOpenFilePicker is stubbed off; the <input> fallback still goes
  // through Playwright's filechooser interceptor.
  const chooserPromise = page.waitForEvent('filechooser', { timeout: 15_000 });
  await page
    .locator('affine-slash-menu .slash-menu')
    .getByText('Image', { exact: true })
    .click();
  const chooser = await chooserPromise;
  await chooser.setFiles({
    name: 'dot.png',
    mimeType: 'image/png',
    buffer: readFileSync(filePath),
  });
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

test('upload posts the PNG and shows pixels', async ({ page }) => {
  const posts: { url: string; status: number }[] = [];
  page.on('response', (res) => {
    if (
      res.request().method() === 'POST' &&
      res.url().includes('/api/blobs/venus-m0')
    ) {
      posts.push({ url: res.url(), status: res.status() });
    }
  });

  await waitForHydrated(page);
  await insertSlashImage(page, DOT_PNG);
  await expectImagePixels(page, 30_000);
  // keck batches doc writes (~1s). Stay connected so the image block is on
  // the server before this test's page closes.
  await page.waitForTimeout(2000);

  expect(posts.length, 'expected POST /api/blobs/venus-m0').toBeGreaterThan(0);
  expect(
    posts.some((p) => p.status === 413),
    '413: raise keck/proxy body size, do not stub the image',
  ).toBe(false);
  expect(posts.some((p) => p.status >= 200 && p.status < 300)).toBe(true);
});

test('second tab sees the image without picking a file', async ({
  page,
  context,
}) => {
  await waitForHydrated(page);
  await expectImagePixels(page, 30_000);
  const pageB = await context.newPage();
  await waitForHydrated(pageB);
  await expectImagePixels(pageB, 15_000);
});

test('reload keeps the image pixels', async ({ page }) => {
  await waitForHydrated(page);
  await expectImagePixels(page, 15_000);

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.locator('affine-editor-container').waitFor({ timeout: 30_000 });
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });
  await expect(page.locator('doc-title')).toContainText(SEED_TITLE);
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
  await expectImagePixels(page, 30_000);
});
