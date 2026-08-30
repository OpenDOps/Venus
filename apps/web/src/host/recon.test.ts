import { expect, test } from 'vitest';

/** Specifiers from the Actual column in docs/design/api-map.md. */
const ACTUAL_IMPORTS = [
  '@blocksuite/affine/store',
  '@blocksuite/affine/store/test',
  '@blocksuite/affine/schemas',
  '@blocksuite/affine/ext-loader',
  '@blocksuite/affine/extensions/store',
  '@blocksuite/affine/extensions/view',
  '@blocksuite/affine/effects',
  '@blocksuite/affine/std',
  '@blocksuite/affine/std/effects',
  '@blocksuite/affine/fragments/outline',
  '@blocksuite/affine/shared/services',
  '@blocksuite/affine/sync',
];

test.each(ACTUAL_IMPORTS)('resolves %s', (spec) => {
  // Resolve only: executing view/outline/schemas in Node pulls Peekable
  // (TC39 decorators) and vanilla-extract `.css.ts`.
  expect(import.meta.resolve(spec)).toMatch(/node_modules/);
});
