import type { Doc } from 'yjs';

/** Rewrite duplicate sibling `gitName`s. Display `name` stays. Returns whether it wrote. */
export function repairDuplicateGitNames(catalog: Doc): boolean;
