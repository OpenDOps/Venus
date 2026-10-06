export function sanitizeDocname(name: unknown): string;
export function filenameOfGitPath(
  gitPath: string,
  kind: 'folder' | 'doc',
): string;
export function gitNameFromStem(stem: string, kind: 'folder' | 'doc'): string;
export function stemOfGitName(gitName: string, kind: 'folder' | 'doc'): string;
export function joinGitPath(
  parentGitPath: string | null | undefined,
  filename: string,
  kind: 'folder' | 'doc',
): string;
export function joinGitNames(
  parentGitPath: string | null | undefined,
  gitName: string,
): string;
export function caseFoldName(name: unknown): string;
export function filenameFromDocname(
  name: unknown,
  options: { fallback: string; taken?: Iterable<string> },
): string;
