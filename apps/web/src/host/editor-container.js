/**
 * Copied from AFFiNE v0.22.4
 * `blocksuite/integration-test/src/editors/editor-container.ts`
 * and renamed TestAffineEditorContainer → VenusEditorContainer.
 * Tag stays `affine-editor-container`. Do not import `@affine/core`.
 */
import { SignalWatcher, WithDisposable } from '@blocksuite/affine/global/lit';
import { ThemeProvider } from '@blocksuite/affine/shared/services';
import { BlockStdScope, ShadowlessElement } from '@blocksuite/affine/std';
import { computed, signal } from '@preact/signals-core';
import { css, html } from 'lit';
import { keyed } from 'lit/directives/keyed.js';
import { when } from 'lit/directives/when.js';

export class VenusEditorContainer extends SignalWatcher(
  WithDisposable(ShadowlessElement)
) {
  static styles = css`
    .affine-page-viewport {
      position: relative;
      display: flex;
      flex-direction: column;
      overflow-x: hidden;
      overflow-y: auto;
      container-name: viewport;
      container-type: inline-size;
      font-family: var(--affine-font-family);
    }
    .affine-page-viewport * {
      box-sizing: border-box;
    }

    @media print {
      .affine-page-viewport {
        height: auto;
      }
    }

    .playground-page-editor-container {
      flex-grow: 1;
      font-family: var(--affine-font-family);
      display: block;
    }

    .playground-page-editor-container * {
      box-sizing: border-box;
    }

    @media print {
      .playground-page-editor-container {
        height: auto;
      }
    }

    .edgeless-editor-container {
      font-family: var(--affine-font-family);
      background: var(--affine-background-primary-color);
      display: block;
      height: 100%;
      position: relative;
      overflow: clip;
    }

    .edgeless-editor-container * {
      box-sizing: border-box;
    }

    @media print {
      .edgeless-editor-container {
        height: auto;
      }
    }

    .affine-edgeless-viewport {
      display: block;
      height: 100%;
      position: relative;
      overflow: clip;
      container-name: viewport;
      container-type: inline-size;
    }
  `;

  _doc = signal();

  _edgelessSpecs = signal([]);

  _mode = signal('page');

  _pageSpecs = signal([]);

  _specs = computed(() =>
    this._mode.value === 'page'
      ? this._pageSpecs.value
      : this._edgelessSpecs.value
  );

  _std = computed(() => {
    return new BlockStdScope({
      store: this.doc,
      extensions: this._specs.value,
    });
  });

  _editorTemplate = computed(() => {
    return this._std.value.render();
  });

  get doc() {
    return this._doc.value;
  }

  set doc(doc) {
    this._doc.value = doc;
  }

  set edgelessSpecs(specs) {
    this._edgelessSpecs.value = specs;
  }

  get edgelessSpecs() {
    return this._edgelessSpecs.value;
  }

  get host() {
    try {
      return this.std.host;
    } catch {
      return null;
    }
  }

  get mode() {
    return this._mode.value;
  }

  set mode(mode) {
    this._mode.value = mode;
  }

  set pageSpecs(specs) {
    this._pageSpecs.value = specs;
  }

  get pageSpecs() {
    return this._pageSpecs.value;
  }

  get rootModel() {
    return this.doc.root;
  }

  get std() {
    return this._std.value;
  }

  connectedCallback() {
    super.connectedCallback();

    this._disposables.add(
      this.doc.slots.rootAdded.subscribe(() => this.requestUpdate())
    );
  }

  firstUpdated() {
    if (this.mode === 'page') {
      setTimeout(() => {
        if (this.autofocus && this.mode === 'page') {
          const richText = this.querySelector('rich-text');
          const inlineEditor = richText?.inlineEditor;
          inlineEditor?.focusEnd();
        }
      });
    }
  }

  render() {
    const mode = this._mode.value;
    const themeService = this.std.get(ThemeProvider);
    const appTheme = themeService.app$.value;
    const edgelessTheme = themeService.edgeless$.value;

    return html`${keyed(
      this.rootModel.id + mode,
      html`
        <div
          data-theme=${mode === 'page' ? appTheme : edgelessTheme}
          class=${mode === 'page'
            ? 'affine-page-viewport'
            : 'affine-edgeless-viewport'}
        >
          ${when(
            mode === 'page',
            () => html` <doc-title .doc=${this.doc}></doc-title> `
          )}
          <div
            class=${mode === 'page'
              ? 'page-editor playground-page-editor-container'
              : 'edgeless-editor-container'}
          >
            ${this._editorTemplate.value}
          </div>
        </div>
      `
    )}`;
  }

  switchEditor(mode) {
    this._mode.value = mode;
  }
}
