import { expect, test, type Page } from '@playwright/test';
import { assertHubOn3000 } from './keck-ws';

const SEED_TITLE = 'Venus';
const SEED_H1 = 'Why Venus';
const NOTE = 'affine-note affine-paragraph rich-text';
const OUTLINE = 'affine-outline-panel';
const OUTLINE_H1 = '[data-testid="outline-block-preview-h1"]';

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
  return pageErrors;
}

test.beforeAll(async () => {
  await assertHubOn3000();
});

test('smoke: seed H1, type hello, reload keeps it', async ({ page }) => {
  const pageErrors = await waitForHydrated(page);
  const kind = await page.evaluate(() => window.__VENUS_PROVIDER_KIND__);
  expect(kind, 'smoke must run with sync env').toBe('octobase');
  await expect(page.locator('doc-title')).toContainText(SEED_TITLE);

  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.keyboard.type('hello');
  await expect(page.locator(NOTE).first()).toContainText('hello');
  await page.waitForTimeout(2000);

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.locator('affine-editor-container').waitFor({ timeout: 30_000 });
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });
  await page.locator(OUTLINE).waitFor({ timeout: 30_000 });

  await expect(page.locator(NOTE).first()).toContainText('hello');
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});
