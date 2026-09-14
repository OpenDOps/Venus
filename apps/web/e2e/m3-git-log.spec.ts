import { expect, test, type Page } from '@playwright/test';
import { assertHubOn3000, assertSidecarOn3002 } from './keck-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
const FLUSH = '[data-testid="venus-flush"]';
const GIT_LOG = '[data-testid="venus-git-log"]';

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

test('git log chrome shows snapshot autocomment, not Yjs undo', async ({
  page,
}) => {
  const pageErrors = await waitForEditor(page);
  const log = page.locator(GIT_LOG);
  await expect(log).toBeVisible();

  const flush = page.locator(FLUSH);
  await expect(flush).toBeVisible();
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.keyboard.type(`m3-log-${Date.now()}`);
  await page.waitForTimeout(2000);
  await flush.click();

  await expect(log).toContainText(/snapshot:\s+\S/, { timeout: 30_000 });
  const text = (await log.innerText()).toLowerCase();
  expect(text, 'git log must not be Yjs undo labels').not.toMatch(/\bundo\b/);
  expect(text).not.toMatch(/store\.history/);
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});
