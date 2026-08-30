import { useEffect, useRef, useState } from 'react';
import { mountEditor } from './host/mount-editor.js';
import { mountOutline, waitForEditorHost } from './host/mount-outline.js';
import { providerFromEnv, blobSourcesFromEnv } from './host/providers/from-env.js';
import { createM0Workspace } from './host/workspace.js';

type Session = Awaited<ReturnType<typeof createM0Workspace>>;

export function App() {
  const [session, setSession] = useState<Session | null>(null);
  const [error, setError] = useState<string | null>(null);
  const editorRef = useRef<HTMLDivElement>(null);
  const outlineRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<Session | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    void createM0Workspace(providerFromEnv(), {
      signal: ac.signal,
      blobSources: blobSourcesFromEnv(),
    })
      .then((created) => {
        if (ac.signal.aborted) {
          created.provider.disconnect(created.docId);
          return;
        }
        window.__VENUS_PROVIDER_KIND__ = created.provider.kind;
        window.__VENUS_PAGE_FLAVOUR__ = created.store.root?.flavour;
        sessionRef.current = created;
        setSession(created);
      })
      .catch((err) => {
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
      const current = sessionRef.current;
      if (current) {
        current.provider.disconnect(current.docId);
        sessionRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    const editorEl = editorRef.current;
    const outlineEl = outlineRef.current;
    if (!session || !editorEl || !outlineEl) return;

    const { store } = session;
    const { editor, unmount } = mountEditor(editorEl, store);
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
      unmountOutline();
      unmount();
    };
  }, [session]);

  if (error) {
    return <div className="m0-shell">{error}</div>;
  }

  if (!session) {
    return <div className="m0-shell" />;
  }

  return (
    <div className="m0-shell">
      <div className="editor-host" ref={editorRef} />
      <aside className="outline-host" ref={outlineRef} />
    </div>
  );
}
