import { ViewExtensionManager } from '@blocksuite/affine/ext-loader';
import { getInternalViewExtensions } from '@blocksuite/affine/extensions/view';
import {
  CommunityCanvasTextFonts,
  FontConfigExtension,
} from '@blocksuite/affine/shared/services';
import { VenusEditorContainer } from './editor-container.js';

const viewManager = new ViewExtensionManager(getInternalViewExtensions());

/**
 * Mount the page editor into `host`.
 * pageSpecs / edgelessSpecs come from the view manager (slash menu, toolbar,
 * drag-handle); Venus does not add its own chrome.
 * `get('page')` also runs OutlineViewExtension.effect() (step-outline).
 */
export function mountEditor(host, store) {
  const fonts = FontConfigExtension(CommunityCanvasTextFonts);
  const editor = new VenusEditorContainer();
  editor.style.display = 'block';
  editor.style.height = '100%';
  editor.autofocus = true;
  editor.doc = store;
  editor.mode = 'page';
  editor.pageSpecs = [...viewManager.get('page'), fonts];
  editor.edgelessSpecs = [...viewManager.get('edgeless'), fonts];
  host.append(editor);
  return {
    editor,
    unmount() {
      editor.remove();
    },
  };
}
