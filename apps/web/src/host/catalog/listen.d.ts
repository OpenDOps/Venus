import type { Doc } from 'yjs';

export function openNodeTitle(catalog: Doc, openDocId: string): string;

export function shouldAutoHome(catalog: Doc, openDocId: string): boolean;

export function applyCatalogHostChrome(
  catalog: Doc,
  openDocId: string,
  hooks: {
    onTitle: (title: string) => void;
    onMissingOpen: () => void;
  },
): void;

export function listenCatalogHost(
  catalog: Doc,
  getOpenDocId: () => string,
  hooks: {
    onTitle: (title: string) => void;
    onMissingOpen: () => void;
    onChange: () => void;
  },
): () => void;
