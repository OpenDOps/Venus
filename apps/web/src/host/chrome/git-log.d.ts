export const CATALOG_GIT_LOG_PATH: 'spec/home.md';

export type GitLogEntry = { subject: string; sha: string };

export function parseGitLog(data: unknown): GitLogEntry[];

export function sameGitLogShas(a: GitLogEntry[], b: GitLogEntry[]): boolean;
