// Network storage (SMB shares, NAS) as reported by the backend (src-tauri/src/network).

export type NetworkStatus =
  | "connected"
  | "notConnected"
  | "unreachable"
  | "signInRequired"
  | "accessDenied"
  | "notFound"
  | "connectionLost"
  | "blockedByPolicy"
  | "credentialConflict"
  | "cancelled"
  | "timedOut"
  | "error";

export interface StatusReport {
  status: NetworkStatus;
  message: string;
  /** Win32 error code, for the diagnostics line. */
  code: number | null;
}

export type NetworkSource = "drive" | "connection" | "remembered" | "favorite";

export interface NetworkLocation {
  path: string;
  server: string;
  share: string | null;
  label: string;
  drive: string | null;
  sources: NetworkSource[];
  /** What Windows reports locally; null for favourites that were not checked. */
  status: NetworkStatus | null;
  managed: boolean;
}

export interface NetworkOverview {
  locations: NetworkLocation[];
  warning: string | null;
}

export interface DiscoveredShare {
  path: string;
  name: string;
  comment: string | null;
}

export interface DiscoveredServer {
  path: string;
  name: string;
  announced: boolean;
  shares: DiscoveredShare[] | null;
  status: StatusReport | null;
}

export interface Discovery {
  servers: DiscoveredServer[];
  networkListAvailable: boolean;
  cancelled: boolean;
}

export type UncKind = "server" | "share" | "folder";

export interface ParsedNetworkPath {
  path: string;
  kind: UncKind;
  server: string;
  share: string | null;
}

export interface ConnectOutcome {
  path: string;
  shareRoot: string;
  report: StatusReport;
  managed: boolean;
  existing: boolean;
}

export interface NetworkFavorite {
  path: string;
  label: string;
}

/** Shares of one server, or the reason they could not be listed (Rust `Result`). */
export type SharesResult = { Ok: DiscoveredShare[] } | { Err: StatusReport };
