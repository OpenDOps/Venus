/// <reference types="vitest/config" />
import { readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { transform as transformVanillaFile } from '@vanilla-extract/integration';
import { vanillaExtractPlugin } from '@vanilla-extract/vite-plugin';
import react from '@vitejs/plugin-react';
import * as esbuild from 'esbuild';
import { defineConfig, transformWithOxc, type Plugin } from 'vite';

const webRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(webRoot, '../..');

function blocksuiteOptimizeExclude(): string[] {
  const pnpmDir = join(repoRoot, 'node_modules/.pnpm');
  const names = new Set<string>([
    // Same source graph as BlockSuite so Lit context / Y.Doc are not duplicated.
    'lit',
    'lit-html',
    'lit-element',
    '@lit/context',
    '@lit/reactive-element',
    '@preact/signals-core',
    'yjs',
    'y-protocols',
    'lib0',
  ]);
  for (const dir of readdirSync(pnpmDir)) {
    if (!dir.startsWith('@blocksuite+')) continue;
    const pkg = dir.slice('@blocksuite+'.length).split('@')[0];
    names.add(`@blocksuite/${pkg}`);
  }
  return [...names];
}

/**
 * BlockSuite 0.22.4 needs oxc `decorator.legacy` so `@requiredProperties`
 * and similar class factories become `__decorate` calls (otherwise the
 * browser sees `export @requiredProperties(...) class` and throws).
 *
 * Lit `@provide` / `@consume` on `accessor` fields mutate the property
 * descriptor and return undefined. The stock `__decorate` helper then
 * re-applies the original accessor, wiping the context wrapper. We
 * replace that helper so a decorator that returns undefined is left alone.
 */
const blocksuiteOxc = {
  decorator: { legacy: true },
  assumptions: { setPublicClassFields: true },
};

function blocksuiteDecorators(): Plugin {
  return {
    name: 'venus-blocksuite-decorators',
    enforce: 'pre',
    async transform(code, id) {
      const [filepath] = id.split('?');
      if (!filepath.includes('/@blocksuite/')) return;
      if (filepath.endsWith('.css.ts')) return;
      if (!/\.tsx?$/.test(filepath)) return;
      const result = await transformWithOxc(code, filepath, {
        ...blocksuiteOxc,
        sourcemap: true,
      });
      // oxc + setPublicClassFields turns `this[symbol] = …` into a class
      // field `[null];`, which rolldown rejects as a duplicate identifier.
      const rewritten = result.code
        .replace(/^\s*\[null\];\s*$/gm, '')
        .replace(
          /from\s*(['"])[^'"]*oxc-project[^'"]*decorate[^'"]*\1/g,
          "from 'virtual:venus-oxc-decorate'",
        );
      return { code: rewritten, map: result.map };
    },
  };
}

/**
 * Wrap `*.css.ts` with fileScope before the dep optimizer bundles them.
 * Without this, prebundled `style()` calls throw "Styles were unable to be
 * assigned to a file".
 */
function vanillaExtractCssTs(): Plugin {
  return {
    name: 'venus-ve-css-ts',
    async transform(code, id) {
      const [filepath] = id.split('?');
      if (!filepath.endsWith('.css.ts')) return;
      const out = await transformVanillaFile({
        source: code,
        filePath: filepath,
        rootPath: webRoot,
        packageName: '@venus/web',
        identOption: 'debug',
      });
      return { code: out, map: null };
    },
  };
}

/**
 * `@Peekable()` is a TC39 decorator (`context.kind`). Legacy lowering calls it
 * as `__decorate([Peekable(...)], Class)` with no context, which throws.
 * M0 does not need peek; tolerate the legacy call so attachment/image/embed
 * blocks can load.
 */
function peekableLegacyDecoratorCompat(): Plugin {
  const needle = "if (context.kind !== 'class') {";
  const insert = "if (context && context.kind !== 'class') {";
  return {
    name: 'venus-peekable-legacy-compat',
    enforce: 'pre',
    transform(code, id) {
      const norm = id.replace(/\\/g, '/');
      if (
        !norm.includes('/affine-components/') ||
        !norm.includes('/peek/peekable.ts')
      ) {
        return;
      }
      if (!code.includes(needle)) {
        throw new Error(
          `venus-peekable-legacy-compat: expected ${needle} in ${norm}`,
        );
      }
      return { code: code.replace(needle, insert), map: null };
    },
  };
}

/**
 * Lit `@provide` (legacy) calls `Object.defineProperty` itself and returns
 * undefined. tslib/`__decorate` then re-applies the pre-decorator accessor
 * and wipes the context setter, so children see `std === undefined`.
 *
 * Oxc injects `@oxc-project+runtime/.../decorate.js` as a Vite virtual id
 * that does not go through `resolveId`. Rewrite imports and the helper body.
 */
function oxcDecoratorRuntime(): Plugin {
  const specifier = 'virtual:venus-oxc-decorate';
  const resolved = `\0${specifier}`;
  const helper = `
export default function decorate(decorators, target, key, desc) {
  var c = arguments.length,
    r = c < 3 ? target : desc === null ? desc = Object.getOwnPropertyDescriptor(target, key) : desc,
    d, v, skipRestore = false;
  for (var i = decorators.length - 1; i >= 0; i--) {
    if ((d = decorators[i])) {
      v = c < 3 ? d(r) : c > 3 ? d(target, key, r) : d(target, key);
      if (v !== undefined) r = v;
      else skipRestore = true;
    }
  }
  if (c < 3) return r;
  if (c > 3 && r && !skipRestore) Object.defineProperty(target, key, r);
  return r;
}
`;
  const isOxcDecorateId = (id: string) => {
    const n = id.replace(/\\/g, '/');
    return n.includes('oxc-project') && n.includes('decorate');
  };
  return {
    name: 'venus-oxc-decorator-runtime',
    enforce: 'pre',
    resolveId(id) {
      if (id === specifier || isOxcDecorateId(id)) return resolved;
    },
    load(id) {
      if (id === resolved) return helper;
    },
    transform(code, id) {
      if (id === resolved) return;
      if (isOxcDecorateId(id) && code.includes('__decorate')) {
        return { code: helper, map: null };
      }
    },
  };
}

/**
 * BlockSuite is excluded from optimizeDeps so Lit context stays one object.
 * Nested CJS (`bytes`, `lodash.*`, `bind-event-listener`, …) is then served
 * as source and browsers reject `import x from 'cjs'`. Convert those files
 * to ESM instead of listing every package in optimizeDeps.include.
 */
function nodeCjsToEsm(): Plugin {
  const marker = '/* venus-cjs-esm */';
  return {
    name: 'venus-node-cjs-to-esm',
    enforce: 'pre',
    async transform(code, id) {
      const filepath = id.split('?')[0];
      if (!filepath.includes('/node_modules/')) return;
      if (filepath.includes('/node_modules/.vite/')) return;
      // Vite must prebundle React; converting it here leaves `require("scheduler")`.
      if (/\/node_modules\/(?:react|react-dom|scheduler)\//.test(filepath)) {
        return;
      }
      if (code.includes(marker)) return;
      if (!/\.(?:cjs|js)$/.test(filepath)) return;
      const isCjs =
        /\bmodule\.exports\b/.test(code) ||
        /\bObject\.defineProperty\(\s*exports\s*,/.test(code) ||
        /(?:^|[\n;])\s*exports\.\w+\s*=/.test(code);
      if (!isCjs) return;

      const names = new Set<string>();
      for (const m of code.matchAll(
        /(?:module\.)?exports\.(\w+)\s*=/g,
      )) {
        names.add(m[1]);
      }
      for (const m of code.matchAll(
        /Object\.defineProperty\(\s*exports\s*,\s*['"](\w+)['"]/g,
      )) {
        names.add(m[1]);
      }
      names.delete('__esModule');

      const result = await esbuild.build({
        stdin: {
          contents: code,
          resolveDir: dirname(filepath),
          sourcefile: filepath,
          loader: 'js',
        },
        bundle: true,
        format: 'esm',
        write: false,
        platform: 'neutral',
        packages: 'external',
        logLevel: 'silent',
      });
      let out = result.outputFiles[0].text;
      const named = [...names]
        .filter((n) => /^[A-Za-z_$][\w$]*$/.test(n))
        .map((n) => `export const ${n} = __cjs.${n};`)
        .join('\n');
      out = out.replace(
        /export default (require_\w+)\(\);/,
        `const __cjs = $1();\nexport default __cjs;\n${named}`,
      );
      return { code: `${marker}\n${out}`, map: null };
    },
  };
}

const peekableCompat = peekableLegacyDecoratorCompat();
const veCssTs = vanillaExtractCssTs();
const decorateRuntime = oxcDecoratorRuntime();
const cjsToEsm = nodeCjsToEsm();

export default defineConfig({
  plugins: [
    cjsToEsm,
    decorateRuntime,
    peekableCompat,
    veCssTs,
    blocksuiteDecorators(),
    vanillaExtractPlugin({ unstable_mode: 'transform' }),
    react(),
  ],
  build: {
    // esbuild minify emits invalid JS from lit-html regexes that contain backticks.
    minify: false,
  },
  oxc: {
    assumptions: { setPublicClassFields: true },
    exclude: [/\.js$/, /\.css\.ts/, /\/@blocksuite\//],
  },
  optimizeDeps: {
    // One module graph: prebundling some @blocksuite entries but not others
    // duplicates `stdContext` and page blocks never receive `std`.
    exclude: blocksuiteOptimizeExclude(),
    include: [
      'extend',
      'bind-event-listener',
      'lodash.ismatch',
      'lodash.clonedeep',
      'lodash.merge',
      'debug',
    ],
    rolldownOptions: {
      plugins: [decorateRuntime, peekableCompat, veCssTs],
      transform: blocksuiteOxc,
    },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
