/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_QANTO_NODE_URL?: string;
  readonly VITE_REFRESH_INTERVAL_MS?: string;
  readonly VITE_MEMPOOL_REFRESH_MS?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}