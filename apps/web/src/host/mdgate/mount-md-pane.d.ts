export function mountMdPane(
  host: HTMLElement,
  store: unknown,
  workspace: { id: string; meta: { docMetas: unknown[] } },
): () => void;
