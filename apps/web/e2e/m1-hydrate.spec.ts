import { expect, test, type Page } from '@playwright/test';

const SEED_TITLE = 'Venus';
const SEED_H1 = 'Why Venus';
const NOTE = 'affine-note affine-paragraph rich-text';
const OUTLINE = 'affine-outline-panel';
const OUTLINE_H1 = '[data-testid="outline-block-preview-h1"]';

test.describe.configure({ mode: 'serial' });

async function waitForHydrated(page: Page, editorTimeout = 120_000) {
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
        timeout: editorTimeout,
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
  await expect(page.locator('doc-title')).toContainText(SEED_TITLE);
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
  return pageErrors;
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

test('typed hello is still there after refresh', async ({ page }) => {
  await waitForHydrated(page);
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.keyboard.type('hello');
  await expect(page.locator(NOTE).first()).toContainText('hello');
  // keck batches doc writes (~1s). Stay connected so the update is on the
  // server before reload closes the socket.
  await page.waitForTimeout(2000);

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.locator('affine-editor-container').waitFor({ timeout: 30_000 });
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });
  await page.locator(OUTLINE).waitFor({ timeout: 30_000 });

  await expect(page.locator(NOTE).first()).toContainText('hello');
  await expect(page.locator('doc-title')).toContainText(SEED_TITLE);
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
});

test('second session does not create a second page', async ({ browser }) => {
  const context = await browser.newContext();
  const page = await context.newPage();
  try {
    await waitForHydrated(page);
    await expect(page.locator('doc-title')).toHaveCount(1);
    await expect(page.locator('doc-title')).toContainText(SEED_TITLE);
    await expect(page.locator(OUTLINE_H1)).toHaveCount(1);
    await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
    await expect(page.locator(OUTLINE_H1)).not.toContainText(
      'Why VenusWhy Venus',
    );
    const flavour = await page.evaluate(() => window.__VENUS_PAGE_FLAVOUR__);
    expect(flavour).toBe('affine:page');
  } finally {
    await context.close();
  }
});
