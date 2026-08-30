import { useEffect, useMemo, useRef } from 'react';
import { mountEditor } from './host/mount-editor.js';
import { mountOutline, waitForEditorHost } from './host/mount-outline.js';
import { providerFromEnv } from './host/providers/from-env.js';
import { createM0Workspace } from './host/workspace.js';

export function App() {
  const { store, docId, provider } = useMemo(() => {
    const created = createM0Workspace(providerFromEnv());
    window.__VENUS_PROVIDER_KIND__ = created.provider.kind;
    return created;
  }, []);
  const editorRef = useRef<HTMLDivElement>(null);
  const outlineRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const editorEl = editorRef.current;
    const outlineEl = outlineRef.current;
    if (!editorEl || !outlineEl) return;

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
      provider.disconnect(docId);
    };
  }, [store, docId, provider]);

  return (
    <div className="m0-shell">
      <div className="editor-host" ref={editorRef} />
      <aside className="outline-host" ref={outlineRef} />
    </div>
  );
}