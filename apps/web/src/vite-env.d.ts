/// <reference types="vite/client" />
/// <reference types="node" />
/// <reference types="vitest/globals" />

interface ImportMetaEnv {
  /** Absolute `ws://…` (Vite) or `same-origin` (Compose/k8s web). */
  readonly VITE_SYNC_URL?: string;
  /** Sidecar origin for Flush + git log (`http://127.0.0.1:28720`). Unset hides chrome. */
  readonly VITE_SIDECAR_URL?: string;
  /** Show Flush / git-log debug bar (with `VITE_SIDECAR_URL`). Playwright m3/m4. */
  readonly VITE_DEBUG?: string;
  /** Emit catalog/header `data-testid`s. Playwright m4. */
  readonly VITE_TESTIDS?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

export {};

declare global {
  interface Window {
    __VENUS_PROVIDER_KIND__?: string;
    /** Live hub only: `shared-worker` holds wire A sockets, or `tab`. */
    __VENUS_HUB_TRANSPORT__?: 'shared-worker' | 'tab';
    /** Worker socket table; `null` on the per-tab path. */
    __VENUS_HUB_STATS__?: () => Promise<
      import('./host/providers/shared-worker-socket.js').HubWorkerStats | null
    >;
    __VENUS_WS_PROTOCOLS__?: string | string[];
    __VENUS_PAGE_FLAVOUR__?: string;
    /** Playwright e2e only (`addInitScript`). */
    __VENUS_E2E__?: boolean;
    __VENUS_FROM_DOC__?: () => Promise<{ markdown: string }>;
    /** Set after catalog seed (M4 step 3; tree UI is later). */
    __VENUS_CATALOG_READY__?: boolean;
    __VENUS_CATALOG_OPS__?: {
      snapshot: () => Array<{
        id: string;
        kind: string;
        name: string;
        parentId: string | null;
        gitPath: string;
        docId?: string;
      }>;
      getNode: (id: string) => {
        id: string;
        gitPath: string;
        parentId: string | null;
        docId?: string;
      } | null;
      createDoc: (createAt: string | null) => Promise<{
        id: string;
        docId?: string;
        gitPath: string;
      }>;
      createFolder: (
        createAt: string | null,
        name: string,
      ) => { id: string; gitPath: string };
      rename: (id: string, name: string) => { gitPath: string; name: string };
      reparent: (
        id: string,
        parentId: string | null,
      ) => { gitPath: string; parentId: string | null };
      drop: (
        id: string,
        parentId: string | null,
      ) => { ok: boolean; gitPath: string | null; parentId: string | null };
      canDrop: (id: string, parentId: string | null) => boolean;
      deleteNode: (id: string) => void;
    };
    /** Open a catalog doc in this tab (home if missing). */
    __VENUS_OPEN_DOC__?: (docId: string) => void;
    __VENUS_OPEN_DOC_ID__?: string;
    /** State vector of the open page, for idle-clock checks. */
    __VENUS_OPEN_VECTOR__?: () => string;
    __VENUS_INSERT_LINKED_DOC__?: (pageId: string) => string;
  }
}
