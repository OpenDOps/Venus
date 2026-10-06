import type { Doc } from 'yjs';

export function installCatalogDocShell(workspace: object): void;

export function projectCatalogDocs(
  catalog: Doc,
  workspace: object,
  options?: { keepId?: string; beforeRemove?: (id: string) => void },
): void;
