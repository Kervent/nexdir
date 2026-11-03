/// <reference types="vite/client" />

/** App version from package.json, injected at build time (see vite.config.ts). */
declare const __APP_VERSION__: string;
declare const __APP_COMMIT__: string;
declare const __APP_COMMIT_DATE__: string;

interface ImportMetaEnv {
  /** Optional Discord invite shown in Help & Support (see src/config/links.ts). */
  readonly VITE_NEXDIR_DISCORD_INVITE?: string;
}
