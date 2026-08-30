import { OutlinePanel } from '@blocksuite/affine/fragments/outline';

/**
 * Outline takes EditorHost (`editor.host`), not the container.
 * OutlineViewExtension.effect() already ran in mountEditor via
 * viewManager.get('page') (registers affine-outline-panel and children).
 *
 * Lit `updateComplete` can resolve before `std.render()` has connected
 * `editor-host`. Poll until the host exists.
 */
export async function waitForEditorHost(editor) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    await editor.updateComplete;
    const host = editor.host ?? editor.querySelector('editor-host');
    if (host) return host;
    await new Promise((resolve) => {
      requestAnimationFrame(resolve);
    });
  }
  throw new Error('EditorHost not ready after updateComplete');
}

export function mountOutline(host, editorHost) {
  const panel = new OutlinePanel();
  panel.editor = editorHost;
  panel.fitPadding = [20, 20, 20, 20];
  panel.style.display = 'block';
  panel.style.height = '100%';
  panel.style.width = '100%';
  host.append(panel);
  return () => {
    panel.remove();
  };
}
