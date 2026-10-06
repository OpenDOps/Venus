import type { Doc } from 'yjs';

export function keepSelectedIds(catalog: Doc, selectedIds: string[]): string[];
export function isRenamingGone(
  catalog: Doc,
  renamingId: string | null | undefined,
): boolean;
export function pruneGoneTreeItems(
  tree: {
    getState: () => {
      selectedItems?: string[];
      renamingItem?: string | null;
    };
    setSelectedItems: (ids: string[]) => void;
    abortRenaming: () => void;
  },
  catalog: Doc,
): void;
