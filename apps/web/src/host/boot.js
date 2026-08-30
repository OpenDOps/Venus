import '@blocksuite/affine/effects';
import '@toeverything/theme/fonts.css';
import '@toeverything/theme/style.css';
import { VenusEditorContainer } from './editor-container.js';

if (!customElements.get('affine-editor-container')) {
  customElements.define('affine-editor-container', VenusEditorContainer);
}
