import type { VenusEditorContainer } from './editor-container.js';

export function mountEditor(
  host: HTMLElement,
  store: unknown,
): { editor: VenusEditorContainer; unmount: () => void };
