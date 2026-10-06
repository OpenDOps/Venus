import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import {
  buildFromPages,
  dropId,
  inbound,
  outbound,
  parse,
  rebuildFromWiki,
  serialize,
  targetsInMarkdown,
  upsertOutbound,
} from './link-index.js';

const here = dirname(fileURLToPath(import.meta.url));
const catalogDir = join(here, '../catalog');

/** Target linked from home + second; third has no `venus:doc` comment. */
export const TARGET = 'a1b2c3d4-e5f6-4780-abcd-ef1234567890';
export const HOME = 'doc:home';
export const SECOND = 'eeeeeeee-e5f6-4780-abcd-ef1234567890';
export const THIRD = 'ffffffff-e5f6-4780-abcd-ef1234567890';

export const HOME_MD = `See [x](spec/${TARGET}.md) and [plain](https://example.com).\n<!-- venus:doc:${TARGET} -->\n`;
export const SECOND_MD = `Also <!-- venus:doc:${TARGET} -->\n`;
export const THIRD_MD = `No comment, just [link](whatever.md)\n`;

export const SCENARIO_1_JSON = `{
  "inbound": {
    "a1b2c3d4-e5f6-4780-abcd-ef1234567890": [
      "doc:home",
      "eeeeeeee-e5f6-4780-abcd-ef1234567890"
    ]
  },
  "outbound": {
    "doc:home": [
      "a1b2c3d4-e5f6-4780-abcd-ef1234567890"
    ],
    "eeeeeeee-e5f6-4780-abcd-ef1234567890": [
      "a1b2c3d4-e5f6-4780-abcd-ef1234567890"
    ]
  }
}
`;

export const PAGES_YAML = `pages:
  spec/home.md:
    uuid: 395cd07b-bdb1-5f54-ada8-e9a3fabb6a20
    docId: "doc:home"
    name: home
    tags: []
  spec/${SECOND}.md:
    uuid: ${SECOND}
    docId: ${SECOND}
    name: second
    tags: []
  spec/${THIRD}.md:
    uuid: ${THIRD}
    docId: ${THIRD}
    name: third
    tags: []
folders:
  spec:
    id: "folder:spec"
    name: spec
`;

function scenarioPages() {
  return [
    { id: HOME, markdown: HOME_MD },
    { id: SECOND, markdown: SECOND_MD },
    { id: THIRD, markdown: THIRD_MD },
  ];
}

function writeWiki(dir: string) {
  mkdirSync(join(dir, 'spec'), { recursive: true });
  mkdirSync(join(dir, '.venus'), { recursive: true });
  writeFileSync(join(dir, 'spec/home.md'), HOME_MD);
  writeFileSync(join(dir, `spec/${SECOND}.md`), SECOND_MD);
  writeFileSync(join(dir, `spec/${THIRD}.md`), THIRD_MD);
  writeFileSync(join(dir, '.venus/pages.yaml'), PAGES_YAML);
}

function walkJs(dir: string, acc: string[] = []) {
  for (const ent of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, ent.name);
    if (ent.isDirectory()) walkJs(p, acc);
    else if (/\.(js|ts|tsx|d\.ts)$/.test(ent.name)) acc.push(p);
  }
  return acc;
}

test('build + query: inbound/outbound from venus:doc comments only', () => {
  expect(targetsInMarkdown(HOME_MD)).toEqual([TARGET]);
  expect(
    targetsInMarkdown(`~~protocol~~\n<!-- venus:doc:${TARGET} missing -->\n`),
  ).toEqual([TARGET]);
  expect(targetsInMarkdown(THIRD_MD)).toEqual([]);
  expect(targetsInMarkdown(`[x](spec/${TARGET}.md)\n`)).toEqual([]);

  const index = buildFromPages(scenarioPages());
  expect(inbound(index, TARGET)).toEqual([HOME, SECOND]);
  expect(outbound(index, HOME)).toEqual([TARGET]);
  expect(outbound(index, SECOND)).toEqual([TARGET]);
  expect(inbound(index, THIRD)).toEqual([]);
  expect(outbound(index, THIRD)).toEqual([]);
  expect(index.inbound[THIRD]).toBeUndefined();
  expect(index.outbound[THIRD]).toBeUndefined();
  expect(index.inbound[HOME]).toBeUndefined();
  expect(serialize(index)).toBe(SCENARIO_1_JSON);
});

test('inbound/outbound lookup does not rescan files after build', () => {
  expect(inbound.toString()).not.toMatch(/readFile|readdir|rebuildFromWiki|targetsInMarkdown/);
  expect(outbound.toString()).not.toMatch(/readFile|readdir|rebuildFromWiki|targetsInMarkdown/);
});

test('merge + delete: upsert empty comments then drop id', () => {
  const index = buildFromPages(scenarioPages());
  upsertOutbound(index, HOME, []);
  expect(inbound(index, TARGET)).toEqual([SECOND]);
  expect(outbound(index, HOME)).toEqual([]);
  expect(index.outbound[HOME]).toBeUndefined();

  dropId(index, TARGET);
  expect(index.inbound[TARGET]).toBeUndefined();
  expect(index.outbound[TARGET]).toBeUndefined();
  expect(inbound(index, TARGET)).toEqual([]);
  expect(outbound(index, SECOND)).toEqual([]);
  expect(JSON.stringify(index)).not.toMatch(TARGET);
});

test('rebuild from HEAD when links.json missing or corrupt', () => {
  const dir = mkdtempSync(join(tmpdir(), 'venus-link-index-'));
  try {
    writeWiki(dir);
    const rebuilt = rebuildFromWiki(dir);
    expect(serialize(rebuilt)).toBe(SCENARIO_1_JSON);

    writeFileSync(join(dir, '.venus/links.json'), '{');
    const fromCorrupt = rebuildFromWiki(dir);
    expect(serialize(fromCorrupt)).toBe(SCENARIO_1_JSON);

    writeFileSync(join(dir, '.venus/links.json'), serialize(fromCorrupt));
    const again = rebuildFromWiki(dir);
    expect(serialize(again)).toBe(serialize(fromCorrupt));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('rebuild does not hydrate Yjs or fromDoc', () => {
  const src = readFileSync(join(here, 'link-index.js'), 'utf8');
  expect(src).not.toMatch(/from ['"]yjs['"]/);
  expect(src).not.toMatch(/fromDoc\(/);
  expect(src).not.toMatch(/hydrate_v1|Y\.Doc|createM0Workspace/);
});

test('graph is not written onto catalog nodes or docMetas', () => {
  const files = walkJs(catalogDir);
  expect(files.length).toBeGreaterThan(0);
  for (const file of files) {
    const src = readFileSync(file, 'utf8');
    expect(src, file).not.toMatch(/\binbound\b/);
    expect(src, file).not.toMatch(/\boutbound\b/);
    expect(src, file).not.toMatch(/links\.json/);
  }
  const ops = readFileSync(join(catalogDir, 'ops.js'), 'utf8');
  expect(ops).not.toMatch(/docMetas/);
});

test('parse rejects corrupt JSON', () => {
  expect(parse('{')).toBeNull();
  expect(parse(SCENARIO_1_JSON)?.inbound[TARGET]).toEqual([HOME, SECOND]);
});
