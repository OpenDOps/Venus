import type { Doc } from 'yjs';

export function resolveOpenDocId(
  catalog: Doc,
  requestedId: string | null | undefined,
): string;
