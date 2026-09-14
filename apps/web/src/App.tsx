import { useEffect, useRef, useState } from 'react';
import { mountEditor } from './host/mount-editor.js';
import { mountOutline, waitForEditorHost } from './host/mount-outline.js';
import { mountMdPane } from './host/mdgate/mount-md-pane.js';
import {
  providerFromEnv,
  blobSourcesFromEnv,
  sidecarUrlFromEnv,
} from './host/providers/from-env.js';
import { seedMarkdownDemo } from './host/seed.js';
import { createM0Workspace } from './host/workspace.js';

type Session = Awaited<ReturnType<typeof createM0Workspace>>;
type GitLogEntry = { subject: string; sha: string };

const SIDECAR_URL = sidecarUrlFromEnv();
const CATALOG_PATH = 'spec/home.md';

function parseGitLog(data: unknown): GitLogEntry[] {
  if (!Array.isArray(data)) return [];
  const out: GitLogEntry[] = [];
  for (const row of data) {
    if (!row || typeof row !== 'object') continue;
    const subject = (row as { subject?: unknown }).subject;
    const sha = (row as { sha?: unknown }).sha;
    if (typeof subject !== 'string' || typeof sha !== 'string') continue;
    out.push({ subject, sha });
  }
  return out;
}

export function App() {
  const [session, setSession] = useState<Session | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [gitLog, setGitLog] = useState<GitLogEntry[]>([]);
  const editorRef = useRef<HTMLDivElement>(null);
  const outlineRef = useRef<HTMLDivElement>(null);
  const mdPaneRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<Session | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    void createM0Workspace(providerFromEnv(), {
      signal: ac.signal,
      blobSources: blobSourcesFromEnv(),
    })
      .then(async (created) => {
        if (ac.signal.aborted) {
          created.provider.disconnect(created.docId);
          return;
        }
        if (new URLSearchParams(window.location.search).has('md-demo')) {
          await seedMarkdownDemo(created.store);
        }
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
    const mdEl = mdPaneRef.current;
    if (!session || !editorEl || !outlineEl || !mdEl) return;

    const { store, workspace } = session;
    const { editor, unmount } = mountEditor(editorEl, store);
    const unmountMd = mountMdPane(mdEl, store, workspace);
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
  }, [session]);

  useEffect(() => {
    if (!SIDECAR_URL || !session) return;
    const loadGitLog = () => {
      void fetch(`${SIDECAR_URL}/git/log?path=${CATALOG_PATH}`)
        .then((res) => (res.ok ? res.json() : []))
        .then((data) => setGitLog(parseGitLog(data)))
        .catch((err) => {
          console.error(err);
        });
    };
    loadGitLog();
    const id = window.setInterval(loadGitLog, 2000);
    return () => window.clearInterval(id);
  }, [session]);

  function onFlush() {
    if (!SIDECAR_URL) return;
    void fetch(`${SIDECAR_URL}/flush`, { method: 'POST' }).catch((err) => {
      console.error(err);
    });
  }

  if (error) {
    return <div className="m0-shell">{error}</div>;
  }

  if (!session) {
    return <div className="m0-shell" />;
  }

  return (
    <div className="m0-shell">
      {SIDECAR_URL ? (
        <div className="host-chrome">
          <button
            type="button"
            data-testid="venus-flush"
            onClick={onFlush}
          >
            Flush
          </button>
          <ol className="host-chrome-log" data-testid="venus-git-log">
            {gitLog.map((row) => (
              <li key={row.sha} data-sha={row.sha}>
                {row.subject}
              </li>
            ))}
          </ol>
        </div>
      ) : null}
      <div className="m0-columns">
        <aside className="md-pane-host" ref={mdPaneRef} />
        <div className="editor-host" ref={editorRef} />
        <aside className="outline-host" ref={outlineRef} />
      </div>
    </div>
  );
}
