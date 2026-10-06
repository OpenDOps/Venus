import type { Doc } from 'yjs';

export const WIKI_ROOT_ID: 'wiki:root';

export function catalogParentId(itemId: string | null | undefined): string | null;

export function wouldCycle(
  catalog: Doc,
  id: string,
  newParentId: string | null,
): boolean;

export function destFromDrop(
  catalog: Doc,
  draggedIds: string[],
  target: { parentId: string | null; childIndex?: number },
): {
  parentId: string | null;
  afterId?: string | null;
  beforeId?: string | null;
};

export function canCatalogDrop(
  catalog: Doc,
  draggedIds: string[],
  destParentId: string | null,
): boolean;

export function applyCatalogDrop(
  catalog: Doc,
  draggedIds: string[],
  dest: {
    parentId: string | null;
    afterId?: string | null;
    beforeId?: string | null;
  },
): boolean;
