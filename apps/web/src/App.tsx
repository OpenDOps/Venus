import { useCallback, useEffect, useRef, useState } from 'react';
import { CatalogTree } from './host/catalog/CatalogTree';
import {
  applyCatalogHostChrome,
  listenCatalogHost,
} from './host/catalog/listen.js';
import { resolveOpenDocId } from './host/catalog/open-doc.js';
import {
  attachCatalogTestHooks,
  createCatalogDoc,
  detachCatalogTestHooks,
  disposeCatalog,
  openCatalog,
} from './host/catalog/open.js';
import { rename } from './host/catalog/ops.js';
import { VenusHeader } from './host/chrome/VenusHeader';
import { VenusDebugBar } from './host/chrome/VenusDebugBar';
import { CATALOG_GUID, PAGE_DOC_ID } from './host/ids.js';
import { mountEditor } from './host/mount-editor.js';
import { mountOutline, waitForEditorHost } from './host/mount-outline.js';
import { mountMdPane } from './host/mdgate/mount-md-pane.js';
import {
  debugFromEnv,
  providerFromEnv,
  blobSourcesFromEnv,
  sidecarUrlFromEnv,
} from './host/providers/from-env.js';
import { seedMarkdownDemo } from './host/seed.js';
import { createM0Workspace, openPageStore } from './host/workspace.js';

type Session = Awaited<ReturnType<typeof createM0Workspace>> & {
  catalog: import('yjs').Doc;
};
type PageStore = Session['store'];

const SIDECAR_URL = sidecarUrlFromEnv();
const SHOW_DEBUG = debugFromEnv() && Boolean(SIDECAR_URL);

export function App() {
  const [session, setSession] = useState<Session | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openDocId, setOpenDocId] = useState<string>(PAGE_DOC_ID);
  const [openStore, setOpenStore] = useState<PageStore | null>(null);
  const [pageTitle, setPageTitle] = useState('home');
  const editorRef = useRef<HTMLDivElement>(null);
  const outlineRef = useRef<HTMLDivElement>(null);
  const mdPaneRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<Session | null>(null);
  const extraDocIdRef = useRef<string | null>(null);
  const openGenRef = useRef(0);
  const openStoreRef = useRef<PageStore | null>(null);
  openStoreRef.current = openStore;
  const openAbortRef = useRef<AbortController | null>(null);
  const openTargetRef = useRef<string | null>(null);
  const openDocIdRef = useRef(openDocId);
  const rebuildTreeRef = useRef<(() => void) | null>(null);
  openDocIdRef.current = openDocId;

  const openDoc = useCallback(async (requestedId: string) => {
    const current = sessionRef.current;
    if (!current) return;
    const { catalog, workspace, provider, store: homeStore } = current;
    const nextId = resolveOpenDocId(catalog, requestedId);

    if (
      nextId !== PAGE_DOC_ID &&
      extraDocIdRef.current !== nextId &&
      openTargetRef.current === nextId &&
      openAbortRef.current
    ) {
      return;
    }

    const gen = ++openGenRef.current;
    openAbortRef.current?.abort();
    const ac = new AbortController();
    openAbortRef.current = ac;
    openTargetRef.current = nextId;

    if (nextId === PAGE_DOC_ID) {
      const extra = extraDocIdRef.current;
      if (extra) {
        provider.disconnect(extra);
        extraDocIdRef.current = null;
      }
      if (gen !== openGenRef.current) return;
      setOpenDocId(PAGE_DOC_ID);
      setOpenStore(homeStore);
      return;
    }

    if (extraDocIdRef.current === nextId) {
      if (gen !== openGenRef.current) return;
      setOpenDocId(nextId);
      const existing =
        typeof workspace.getDoc === 'function'
          ? workspace.getDoc(nextId)
          : null;
      const store = existing?.getStore?.();
      if (store) setOpenStore(store);
      return;
    }

    try {
      const opened = await openPageStore(workspace, provider, nextId, {
        signal: ac.signal,
      });
      if (gen !== openGenRef.current) {
        if (openTargetRef.current !== nextId) provider.disconnect(nextId);
        return;
      }
      const prev = extraDocIdRef.current;
      if (prev && prev !== nextId) provider.disconnect(prev);
      extraDocIdRef.current = nextId;
      setOpenDocId(nextId);
      setOpenStore(opened.store);
    } catch (err) {
      if (err instanceof Error && err.name === 'AbortError') {
        if (openTargetRef.current !== nextId) provider.disconnect(nextId);
        return;
      }
      console.error(err);
      if (extraDocIdRef.current !== nextId) provider.disconnect(nextId);
      // WS/hydrate failure is not a missing catalog node. Stay on the
      // last good page.
    }
  }, []);

  const onPageTitleChange = useCallback((next: string) => {
    const current = sessionRef.current;
    if (!current) return;
    try {
      rename(current.catalog, current.workspace, openDocIdRef.current, next);
    } catch {
      // Missing node: listenCatalogHost restores the last catalog name.
    }
  }, []);

  useEffect(() => {
    const ac = new AbortController();
    const catalog = createCatalogDoc();
    const provider = providerFromEnv();
    void createM0Workspace(provider, {
      signal: ac.signal,
      blobSources: blobSourcesFromEnv(),
      connectDocs: [{ docId: CATALOG_GUID, ydoc: catalog }],
    })
      .then(async (created) => {
        if (ac.signal.aborted) {
          disposeCatalog(created.provider, catalog, created.docId);
          return;
        }
        try {
          await openCatalog(created.provider, created.workspace, {
            catalog,
            alreadyConnected: true,
            signal: ac.signal,
          });
        } catch (err) {
          disposeCatalog(created.provider, catalog, created.docId);
          throw err;
        }
        if (ac.signal.aborted) {
          disposeCatalog(created.provider, catalog, created.docId);
          return;
        }
        if (new URLSearchParams(window.location.search).has('md-demo')) {
          await seedMarkdownDemo(created.store);
        }
        if (ac.signal.aborted) {
          disposeCatalog(created.provider, catalog, created.docId);
          return;
        }
        window.__VENUS_PROVIDER_KIND__ = created.provider.kind;
        window.__VENUS_PAGE_FLAVOUR__ = created.store.root?.flavour;
        attachCatalogTestHooks(catalog, created.workspace);
        const next = { ...created, catalog };
        sessionRef.current = next;
        setOpenDocId(PAGE_DOC_ID);
        setOpenStore(created.store);
        setSession(next);
      })
      .catch((err) => {
        disposeCatalog(provider, catalog, PAGE_DOC_ID);
        if (
          ac.signal.aborted ||
          (err instanceof Error && err.name === 'AbortError')
        ) {
          return;
        }
        console.error(err);
        setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      ac.abort();
      openAbortRef.current?.abort();
      detachCatalogTestHooks();
      const current = sessionRef.current;
      if (current) {
        const extra = extraDocIdRef.current;
        if (extra) current.provider.disconnect(extra);
        extraDocIdRef.current = null;
        disposeCatalog(current.provider, current.catalog, current.docId);
        sessionRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (!session) return;
    window.__VENUS_OPEN_DOC__ = (id) => {
      void openDoc(id);
    };
    window.__VENUS_OPEN_DOC_ID__ = openDocId;
    window.__VENUS_INSERT_LINKED_DOC__ = (pageId) => {
      const store = openStoreRef.current;
      const note = store?.root?.children?.find(
        (c: { flavour: string }) => c.flavour === 'affine:note',
      );
      if (!store || !note) {
        throw new Error('no open note');
      }
      return store.addBlock('affine:embed-linked-doc', { pageId }, note.id);
    };
    return () => {
      delete window.__VENUS_OPEN_DOC__;
      delete window.__VENUS_OPEN_DOC_ID__;
      delete window.__VENUS_INSERT_LINKED_DOC__;
    };
  }, [session, openDoc, openDocId]);

  useEffect(() => {
    if (!session) return;
    applyCatalogHostChrome(session.catalog, openDocId, {
      onTitle: (title) => {
        setPageTitle((prev) => (prev === title ? prev : title));
      },
      onMissingOpen: () => {
        void openDoc(PAGE_DOC_ID);
      },
    });
  }, [session, openDocId, openDoc]);

  useEffect(() => {
    if (!session) return;
    return listenCatalogHost(session.catalog, () => openDocIdRef.current, {
      onTitle: (title) => {
        setPageTitle((prev) => (prev === title ? prev : title));
      },
      onMissingOpen: () => {
        void openDoc(PAGE_DOC_ID);
      },
      onChange: () => {
        rebuildTreeRef.current?.();
      },
    });
  }, [session, openDoc]);

  useEffect(() => {
    const editorEl = editorRef.current;
    const outlineEl = outlineRef.current;
    const mdEl = mdPaneRef.current;
    if (!session || !openStore || !editorEl || !outlineEl || !mdEl) return;

    const { workspace } = session;
    const { editor, unmount } = mountEditor(editorEl, openStore);
    const unmountMd = mountMdPane(mdEl, openStore, workspace);
    let unmountOutline = () => {};
    let cancelled = false;

    void waitForEditorHost(editor)
      .then((host) => {
        if (cancelled || !host) return;
        unmountOutline = mountOutline(outlineEl, host);
      })
      .catch((err) => {
        if (!cancelled) console.error(err);
      });

    return () => {
      cancelled = true;
      unmountMd();
      unmountOutline();
      unmount();
    };
  }, [session, openStore]);

  if (error) {
    return <div className="m0-shell">{error}</div>;
  }

  if (!session || !openStore) {
    return <div className="m0-shell" />;
  }

  return (
    <div className="m0-shell">
      <VenusHeader
        store={openStore}
        title={pageTitle}
        onTitleChange={onPageTitleChange}
      />
      {SHOW_DEBUG ? <VenusDebugBar sidecarUrl={SIDECAR_URL} /> : null}
      <div className="m0-columns">
        <aside className="tree-host">
          <CatalogTree
            catalog={session.catalog}
            workspace={session.workspace}
            selectedDocId={openDocId}
            rebuildRef={rebuildTreeRef}
            onOpenDoc={openDoc}
          />
        </aside>
        <div className="editor-stack">
          <div className="editor-host" ref={editorRef} />
          <aside className="md-pane-host" ref={mdPaneRef} />
        </div>
        <aside className="outline-host" ref={outlineRef} />
      </div>
    </div>
  );
}
