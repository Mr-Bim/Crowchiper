/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly INLINE_CONFIG: boolean;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
