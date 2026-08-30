/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Absolute `ws://…` (Vite) or `same-origin` (Compose/k8s web). */
  readonly VITE_SYNC_URL?: string;
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
  }
}
