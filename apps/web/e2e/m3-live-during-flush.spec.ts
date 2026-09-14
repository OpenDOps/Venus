import { expect, test, type Page } from '@playwright/test';
import { assertHubOn3000, assertSidecarOn3002 } from './keck-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
const FLUSH = '[data-testid="venus-flush"]';
const DURING = 'during-flush';

/** Must match sidecar `SNAPSHOT_CONVERT_SLEEP_MS` (≥2s). */
const convertSleepMs = Number(process.env.PLAYWRIGHT_CONVERT_SLEEP_MS ?? '0');

test.describe.configure({ mode: 'serial' });

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

async function focusNote(page: Page) {
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
}

test.describe('live during flush', () => {
  test.skip(
    convertSleepMs < 2000,
    'sidecar SNAPSHOT_CONVERT_SLEEP_MS must be ≥2000; set PLAYWRIGHT_CONVERT_SLEEP_MS to the same value (pnpm test:e2e:m3:live).',
  );

  test.beforeAll(async () => {
    await assertHubOn3000();
    await assertSidecarOn3002();
  });

  test('A→B during convert: during-flush appears in B without reload; editors not readonly', async ({
    page,
    context,
  }) => {
    const pageA = page;
    const errorsA = await waitForEditor(pageA);
    const pageB = await context.newPage();
    const errorsB = await waitForEditor(pageB);

    await focusNote(pageA);
    await pageA.keyboard.type(`pre-${Date.now()}`);
    await pageA.waitForTimeout(2000);

    const flush = pageA.locator(FLUSH);
    await expect(flush).toBeVisible();
    await flush.click();

    await focusNote(pageA);
    const during = `${DURING}-${Date.now()}`;
    await pageA.keyboard.type(during);
    await expect(pageA.locator(NOTE).first()).toContainText(during);

    // Tighter than 10s: B must land before convert sleep ends (not wait for git).
    const bTimeout = Math.min(10_000, Math.max(500, convertSleepMs - 500));
    await expect(pageB.locator(NOTE).first()).toContainText(during, {
      timeout: bTimeout,
    });

    // Shared wiki CRDT may not keep the caret at the end of `during`; uniqueness
    // still proves Flush did not set readonly.
    await focusNote(pageA);
    const still = `still-edit-${Date.now()}`;
    await pageA.keyboard.type(still);
    await expect(pageA.locator(NOTE).first()).toContainText(still);

    expect(errorsA, errorsA.join('\n')).toEqual([]);
    expect(errorsB, errorsB.join('\n')).toEqual([]);
  });
});
