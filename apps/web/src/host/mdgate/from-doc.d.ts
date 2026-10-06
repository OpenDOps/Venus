import type { Doc } from 'yjs';

export type SidecarBlockRange = {
  id: string;
  start: number;
  end: number;
};

export type Sidecar = {
  docId: string;
  clock: string;
  blocks: SidecarBlockRange[];
};

export function collectRangedBlocks(root: {
  flavour: string;
  children?: unknown[];
} | null): Array<{ model: { id: string; flavour: string }; listDepth: number }>;

export async function blockMarkdownSlice(
  adapter: {
    job: { blockToSnapshot: (model: unknown) => unknown; assetsManager: unknown };
    fromBlockSnapshot: (payload: {
      snapshot: unknown;
      assets?: unknown;
    }) => Promise<{ file?: string } | undefined>;
  },
  model: { flavour: string; props?: { pageId?: string } },
  listDepth: number,
  catalogLinks?: {
    sourceGitPath: string;
    pages: Record<string, { name: string; gitPath: string }>;
    missing?: Record<string, string>;
  },
): Promise<string>;

export function encodeSidecarClock(ydoc: Doc): string;

export function posixRelativeFromFiles(fromFile: string, toFile: string): string;

export function escapeLinkText(name: string): string;

export function missingLinkedDocExport(pageId: string, name?: string): string | null;

export function catalogLinkedDocLink(
  pageId: string,
  catalogLinks: {
    sourceGitPath: string;
    pages: Record<string, { name: string; gitPath: string }>;
    missing?: Record<string, string>;
  },
): { link: string; href: string } | null;

export function fromDoc(
  store: {
    root: {
      id: string;
      flavour: string;
      props?: { title?: { toString: () => string } };
      children?: unknown[];
    } | null;
    spaceDoc: Doc;
    doc?: { id: string };
    id?: string;
    getTransformer: (middlewares?: unknown[]) => unknown;
    provider: unknown;
  },
  workspace: { id: string; meta: { docMetas: unknown[] } },
  catalogLinks?: {
    sourceGitPath: string;
    pages: Record<string, { name: string; gitPath: string }>;
    missing?: Record<string, string>;
  },
): Promise<{ markdown: string; sidecar: Sidecar }>;

export function roundTripFromDoc(
  store: Parameters<typeof fromDoc>[0],
  workspace: Parameters<typeof fromDoc>[1],
): Promise<{
  first: { markdown: string; sidecar: Sidecar };
  imported: {
    root: {
      children: {
        flavour: string;
        children: { flavour: string; props: { type?: string } }[];
      }[];
    } | null;
  };
  second: { markdown: string; sidecar: Sidecar };
}>;
