// Types mirrored from the Rust backend (src-tauri/src). Keep in sync with the serde models.

export type EntryKind = "file" | "dir" | "fileLink" | "dirLink" | "junction";

export interface FileEntry {
  name: string;
  path: string;
  kind: EntryKind;
  size: number | null;
  modified: number | null;
  created: number | null;
  extension: string | null;
  hidden: boolean;
  system: boolean;
  readonly: boolean;
}

export interface DirListing {
  path: string;
  parent: string | null;
  entries: FileEntry[];
  unreadable: number;
  hiddenCount: number;
}

export type DriveKind = "fixed" | "removable" | "network" | "optical" | "ram" | "unknown";

export interface DriveInfo {
  path: string;
  letter: string;
  label: string;
  kind: DriveKind;
  fileSystem: string | null;
  totalBytes: number | null;
  freeBytes: number | null;
  available: boolean;
}

export type KnownFolderId = "home" | "desktop" | "downloads" | "documents" | "pictures" | "music" | "videos";

export interface KnownFolder {
  id: KnownFolderId;
  label: string;
  path: string;
}

export interface ResolvedPath {
  path: string;
  select: string | null;
}

export type ErrorCode =
  | "notFound"
  | "permissionDenied"
  | "alreadyExists"
  | "invalidName"
  | "invalidPath"
  | "notADirectory"
  | "inUse"
  | "deviceNotReady"
  | "unsupported"
  | "cancelled"
  | "io";

export interface ApiError {
  code: ErrorCode;
  message: string;
  path: string | null;
}

export interface ItemFailure {
  path: string;
  message: string;
}

export interface RecycleResult {
  recycled: string[];
  failed: ItemFailure[];
}

export type TransferKind = "copy" | "move";
export type ConflictPolicy = "skip" | "keepBoth" | "replace";
export type OperationState = "queued" | "scanning" | "running" | "completed" | "cancelled";

export interface ConflictCheck {
  conflicts: string[];
  sameFolder: boolean;
}

export interface OperationProgress {
  id: number;
  kind: TransferKind;
  state: OperationState;
  destination: string;
  sourceCount: number;
  totalBytes: number;
  doneBytes: number;
  totalFiles: number;
  doneFiles: number;
  current: string | null;
  failureCount: number;
  skippedCount: number;
  startedAt: number;
  finishedAt: number | null;
}

export interface OperationReport {
  completed: string[];
  failures: ItemFailure[];
  skipped: ItemFailure[];
  truncated: boolean;
}

export interface ClipboardFiles {
  paths: string[];
  cut: boolean;
}

export type PreviewContent =
  | { type: "image"; mime: string; data: string }
  | { type: "text"; text: string; truncated: boolean }
  | { type: "folder"; items: number; truncated: boolean }
  | { type: "none"; reason: string };

export interface Preview {
  entry: FileEntry;
  content: PreviewContent;
}