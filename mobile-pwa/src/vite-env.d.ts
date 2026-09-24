/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE?: string;
  readonly VITE_ENABLE_SW_DEV?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}