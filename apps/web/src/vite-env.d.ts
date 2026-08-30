/// <reference types="vite/client" />

interface ImportMetaEnv {
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
  }
}
