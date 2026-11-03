// Mirrors src-tauri/src/updater (serde camelCase).

export type Channel = "stable" | "preview";
export type InstallKind = "nsis" | "msi" | "portable";
export type UpdatePhase = "idle" | "checking" | "upToDate" | "available" | "downloading" | "readyToInstall" | "installing" | "error";

export interface UpdateError {
  code: string;
  message: string;
}

export interface UpdateOffer {
  version: string;
  prerelease: boolean;
  pubDate: string | null;
  notes: string;
  releaseUrl: string;
  size: number | null;
}

export interface UpdateSnapshot {
  phase: UpdatePhase;
  channel: Channel;
  offer: UpdateOffer | null;
  latest: string | null;
  noRelease: boolean;
  downloaded: number;
  total: number | null;
  error: UpdateError | null;
}

export interface UpdateInfo {
  currentVersion: string;
  installKind: InstallKind;
  canInstall: boolean;
  defaultChannel: Channel;
  autoCheck: boolean;
  channel: Channel;
  notify: boolean;
  intervalHours: number;
  lastSuccess: number | null;
  lastAttempt: number | null;
  releasesUrl: string;
  lastInstall: { version: string; installed: boolean } | null;
  state: UpdateSnapshot;
}

export interface UserUpdateSettings {
  autoCheck: boolean;
  channel: Channel;
  notify: boolean;
  intervalHours: number;
}

export type CheckResult = { result: "skipped" } | { result: "done"; state: UpdateSnapshot; notify: boolean };

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}
