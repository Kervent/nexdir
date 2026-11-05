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

/** Same-volume key of a network path: `\\server\share` (UNC) or the drive letter of a mapped drive. */
function rootKey(path: string): string {
  const p = path.replace(/\//g, "\\");
  const unc = /^\\\\(?:\?\\UNC\\)?([^\\]+)\\([^\\]+)/i.exec(p);
  if (unc) return `\\\\${unc[1]}\\${unc[2]}`.toLowerCase();
  return p.slice(0, 2).toUpperCase();
}

export const NETWORK_MOVE_REFUSED =
  "Moving between a network share and another location is not available yet: network shares have no Recycle Bin, and Nexdir cannot confirm that the server has stored the copy before the original is deleted. Copy the items instead, check the copy, then delete the original yourself.";

/** Why a move must not start, or null. Moves inside one share are renames and allowed. */
export function moveBlockedReason(sources: string[], destination: string, drives: DriveInfo[]): string | null {
  const all = [...sources, destination];
  if (!all.some((p) => isNetworkPath(p, drives))) return null;
  const roots = new Set(all.map(rootKey));
  return roots.size === 1 ? null : NETWORK_MOVE_REFUSED;
}
