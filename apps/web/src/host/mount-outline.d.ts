export function waitForEditorHost(editor: {
  updateComplete: Promise<unknown>;
  host: unknown;
  querySelector: (selector: string) => unknown;
}): Promise<unknown>;

export function mountOutline(
  host: HTMLElement,
  editorHost: unknown,
): () => void;
