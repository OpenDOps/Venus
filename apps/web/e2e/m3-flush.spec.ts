import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { assertHubOn3000, assertSidecarOn3002 } from './keck-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
const FLUSH = '[data-testid="venus-flush"]';
const here = dirname(fileURLToPath(import.meta.url));
const wikiHome = join(here, '../../../wiki/spec/home.md');

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
  await assertSidecarOn3002();
});

test('typed word lands in wiki/spec/home.md after Flush', async ({ page }) => {
  const pageErrors = await waitForEditor(page);
  const word = `m3-flush-${Date.now()}`;

  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.keyboard.type(word);
  await expect(page.locator(NOTE).first()).toContainText(word);
  await page.waitForTimeout(2000);

  const flush = page.locator(FLUSH);
  await expect(flush).toBeVisible();
  await flush.click();

  await expect
    .poll(
      async () => {
        try {
          return await readFile(wikiHome, 'utf8');
        } catch {
          return '';
        }
      },
      { timeout: 30_000, intervals: [250, 500, 1000] },
    )
    .toContain(word);

  await expect(page.locator(NOTE).first()).toContainText(word);
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});
