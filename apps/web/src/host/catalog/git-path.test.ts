import { expect, test } from 'vitest';
import { filenameFromDocname, gitNameFromStem, joinGitNames, joinGitPath, sanitizeDocname, stemOfGitName } from './git-path.js';

test('sanitizeDocname strips controls and keeps slash', () => {
  expect(sanitizeDocname('foo\u0000/bar\n')).toBe('foo/bar');
});

test('foo/bar becomes file foo_bar; empty stem uses fallback', () => {
  expect(
    filenameFromDocname('foo/bar', { fallback: 'uuid', taken: [] }),
  ).toBe('foo_bar');
  expect(
    filenameFromDocname('foo\\bar', { fallback: 'uuid', taken: [] }),
  ).toBe('foo_bar');
  expect(filenameFromDocname('\u0001', { fallback: 'uuid', taken: [] })).toBe(
    'uuid',
  );
  expect(filenameFromDocname('..', { fallback: 'uuid', taken: [] })).toBe(
    'uuid',
  );
  expect(filenameFromDocname('trick.md', { fallback: 'uuid', taken: [] })).toBe(
    'uuid',
  );
});

test('sibling clash appends _1 then _2', () => {
  expect(
    filenameFromDocname('protocol', { fallback: 'x', taken: ['protocol'] }),
  ).toBe('protocol_1');
  expect(
    filenameFromDocname('protocol', {
      fallback: 'x',
      taken: ['protocol', 'protocol_1'],
    }),
  ).toBe('protocol_2');
});

test('joinGitPath appends .md only for docs', () => {
  expect(joinGitPath('spec', 'home', 'doc')).toBe('spec/home.md');
  expect(joinGitPath('spec', 'crdt', 'folder')).toBe('spec/crdt');
  expect(joinGitPath('', 'design', 'folder')).toBe('design');
  expect(joinGitPath(null, 'uuid', 'doc')).toBe('uuid.md');
});

test('gitName is the POSIX leaf; docs include .md', () => {
  expect(gitNameFromStem('home', 'doc')).toBe('home.md');
  expect(gitNameFromStem('SPEC', 'folder')).toBe('SPEC');
  expect(stemOfGitName('home.md', 'doc')).toBe('home');
  expect(stemOfGitName('crdt', 'folder')).toBe('crdt');
  expect(joinGitNames('SPEC/crdt', 'protocol.md')).toBe('SPEC/crdt/protocol.md');
  expect(joinGitNames('', 'SPEC')).toBe('SPEC');
});
