/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Absolute `ws://…` (Vite) or `same-origin` (Compose/k8s web). */
  readonly VITE_SYNC_URL?: string;
  /** Sidecar origin for Flush + git log (`http://127.0.0.1:3002`). Unset hides chrome. */
  readonly VITE_SIDECAR_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

export {};

declare global {
  interface Window {
    __VENUS_PROVIDER_KIND__?: string;
    __VENUS_WS_PROTOCOLS__?: string | string[];
    __VENUS_PAGE_FLAVOUR__?: string;
    /** Playwright e2e only (`addInitScript`). */
    __VENUS_E2E__?: boolean;
    __VENUS_FROM_DOC__?: () => Promise<{ markdown: string }>;
  }
}
