import { expect, test } from 'vitest';
import { nextGitLogPoll, parseGitLog, sameGitLogShas } from './git-log.js';

test('parseGitLog keeps subject+sha rows and drops junk', () => {
  expect(parseGitLog(null)).toEqual([]);
  expect(parseGitLog({})).toEqual([]);
  expect(
    parseGitLog([
      { subject: 'snapshot', sha: 'abc' },
      { subject: 1, sha: 'nope' },
      { sha: 'missing-subject' },
      { subject: 'ok', sha: 'def' },
    ]),
  ).toEqual([
    { subject: 'snapshot', sha: 'abc' },
    { subject: 'ok', sha: 'def' },
  ]);
});

test('git log refreshes when flush status sha changes after the first observation', () => {
  const first = nextGitLogPoll({ seen: false, sha: null }, 'aaa');
  expect(first).toEqual({ seen: true, sha: 'aaa', refresh: false });
  const same = nextGitLogPoll(first, 'aaa');
  expect(same.refresh).toBe(false);
  const next = nextGitLogPoll(same, 'bbb');
  expect(next).toEqual({ seen: true, sha: 'bbb', refresh: true });
  const cleared = nextGitLogPoll(next, null);
  expect(cleared.refresh).toBe(true);
});

test('sameGitLogShas compares sha order, not subjects', () => {
  const a = [{ subject: 'one', sha: 'aaa' }, { subject: 'two', sha: 'bbb' }];
  expect(sameGitLogShas(a, [{ subject: 'ONE', sha: 'aaa' }, { subject: 'TWO', sha: 'bbb' }])).toBe(
    true,
  );
  expect(sameGitLogShas(a, [{ subject: 'one', sha: 'aaa' }])).toBe(false);
  expect(sameGitLogShas(a, [{ subject: 'one', sha: 'bbb' }, { subject: 'two', sha: 'aaa' }])).toBe(
    false,
  );
});
