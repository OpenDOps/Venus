import 'highlight.js/styles/github.css';
import { fromDoc } from './from-doc.js';
import { highlight } from './highlight-md.js';
import { createMdPaneLoop } from './md-pane-loop.js';
import { incrementalFromDoc } from './splice.js';

function subscribeBlockUpdates(store, onId) {
  const slot = store?.slots?.blockUpdated;
  if (slot && typeof slot.subscribe === 'function') {
    const sub = slot.subscribe((payload) => {
      if (payload?.id) onId(payload.id);
    });
    return () => {
      if (typeof sub === 'function') sub();
      else if (sub && typeof sub.unsubscribe === 'function') sub.unsubscribe();
    };
  }
  const ydoc = store?.spaceDoc;
  if (ydoc && typeof ydoc.on === 'function') {
    const onUpdate = () => onId();
    ydoc.on('update', onUpdate);
    return () => ydoc.off('update', onUpdate);
  }
  return () => {};
}

function paintMarkdown(code, markdown) {
  code.innerHTML = highlight(markdown);
}

function paintError(code, err) {
  console.error(err);
  code.textContent = err instanceof Error ? err.message : String(err);
}

/**
 * Read-only source pane. First paint is full `fromDoc`. While mounted,
 * Store updates run single-flight splice or full export, then re-highlight
 * the whole string.
 */
export function mountMdPane(host, store, workspace) {
  const pre = document.createElement('pre');
  const code = document.createElement('code');
  code.dataset.testid = 'venus-md-pane';
  code.className = 'hljs';
  code.setAttribute('contenteditable', 'false');
  pre.append(code);
  host.append(pre);

  const loop = createMdPaneLoop({
    async exportRun(previous, ids, { forceFull }) {
      if (!previous) {
        const full = await fromDoc(store, workspace);
        return { ...full, mode: 'full' };
      }
      return incrementalFromDoc(store, workspace, previous, ids, {
        forceFull,
      });
    },
    onMarkdown(markdown) {
      paintMarkdown(code, markdown);
    },
    onError(err) {
      paintError(code, err);
    },
  });

  if (typeof window !== 'undefined' && window.__VENUS_E2E__) {
    window.__VENUS_FROM_DOC__ = () => fromDoc(store, workspace);
  }

  const unsubscribe = subscribeBlockUpdates(store, (id) => {
    loop.mark(id ? [id] : []);
  });
  loop.start();

  return () => {
    loop.dispose();
    unsubscribe();
    if (typeof window !== 'undefined' && window.__VENUS_FROM_DOC__) {
      delete window.__VENUS_FROM_DOC__;
    }
    pre.remove();
  };
}
