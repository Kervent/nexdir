import type { DriveInfo } from "../../types/fs";
import type { NetworkStatus } from "../../types/network";

/** `\\server\share…` (also with forward slashes); not device paths like `\\.\` or `\\?\C:`. */
export function isUncPath(path: string): boolean {
  const p = path.replace(/\//g, "\\");
  return /^\\\\[^\\.?][^\\]*\\[^\\]+/.test(p) || /^\\\\\?\\UNC\\/i.test(p);
}

/** `\\server` alone (with an optional trailing backslash): a server, not a folder. */
export function serverOnly(input: string): string | null {
  const m = /^\s*\\\\([^\\/.?][^\\/]*)\\?\s*$/.exec(input.replace(/\//g, "\\"));
  return m ? m[1]! : null;
}

/** True for paths on a network share: UNC paths and drive letters mapped to a share. */
export function isNetworkPath(path: string, drives: DriveInfo[]): boolean {
  if (isUncPath(path)) return true;
  const letter = /^([A-Za-z]):/.exec(path)?.[1]?.toUpperCase();
  return !!letter && drives.some((d) => d.letter.toUpperCase() === letter && d.kind === "network");
}

export const STATUS_LABELS: Record<NetworkStatus, string> = {
  connected: "Connected",
  notConnected: "Not connected",
  unreachable: "Not reachable",
  signInRequired: "Sign-in required",
  accessDenied: "Access denied",
  notFound: "Not available",
  connectionLost: "Connection lost",
  blockedByPolicy: "Blocked by Windows",
  credentialConflict: "Other account in use",
  cancelled: "Sign-in cancelled",
  timedOut: "No answer",
  error: "Error",
};

/** Visual tone of a status chip. */
export function statusTone(status: NetworkStatus | null): "ok" | "warn" | "error" | "muted" {
  if (status === "connected") return "ok";
  if (status === null || status === "notConnected" || status === "cancelled") return "muted";
  if (status === "signInRequired" || status === "timedOut" || status === "credentialConflict") return "warn";
  return "error";
}

/** Statuses after which signing in with other credentials can help. */
export function needsSignIn(status: NetworkStatus): boolean {
  return status === "signInRequired" || status === "accessDenied";
}
