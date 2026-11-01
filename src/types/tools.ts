// Types for Phase 3 tools, mirrored from src-tauri/src/{tasks,analytics,search,operations/organize}.rs.
import type { ConflictPolicy, FileEntry, TransferKind } from "./fs";

export type TaskKind = "storageScan" | "duplicates" | "search" | "indexBuild";
export type TaskState = "running" | "completed" | "cancelled" | "failed";

export interface TaskProgress {
  id: number;
  kind: TaskKind;
  state: TaskState;
  phase: string;
  items: number;
  bytes: number;
  total: number | null;
  done: number;
  current: string | null;
  error: string | null;
  startedAt: number;
  finishedAt: number | null;
}

export interface SizeNode {
  name: string;
  path: string;
  bytes: number;
  files: number;
  isDir: boolean;
}

export interface LargeFile {
  name: string;
  path: string;
  bytes: number;
  modified: number | null;
}

export interface TypeShare {
  key: string;
  bytes: number;
  files: number;
}

export interface StorageReport {
  root: string;
  totalBytes: number;
  fileCount: number;
  folderCount: number;
  unreadable: number;
  skippedLinks: number;
  children: SizeNode[];
  otherChildren: number;
  otherChildrenBytes: number;
  largestFiles: LargeFile[];
  largestFolders: SizeNode[];
  categories: TypeShare[];
  extensions: TypeShare[];
  volume: { totalBytes: number; freeBytes: number } | null;
  complete: boolean;
}

export interface DuplicateFile {
  path: string;
  name: string;
  modified: number | null;
}

export interface DuplicateGroup {
  size: number;
  hash: string;
  files: DuplicateFile[];
}

/** A duplicate group as resolved by the user: which copies stay, which go to the Recycle Bin. */
export interface DuplicateRemoval {
  size: number;
  hash: string;
  keep: string[];
  remove: string[];
}

export interface DuplicateReport {
  groups: DuplicateGroup[];
  wastedBytes: number;
  filesScanned: number;
  bytesHashed: number;
  unreadable: number;
  truncated: boolean;
  complete: boolean;
}

export type SearchKind = "any" | "files" | "folders";

export interface SearchQuery {
  root: string;
  recursive: boolean;
  includeHidden: boolean;
  text: string | null;
  extensions: string[];
  categories: string[];
  minSize: number | null;
  maxSize: number | null;
  modifiedAfter: number | null;
  modifiedBefore: number | null;
  kind: SearchKind;
  limit: number | null;
}

export interface SearchResult {
  entries: FileEntry[];
  scanned: number;
  truncated: boolean;
  complete: boolean;
  source: "live" | "index";
}

export interface IndexSettings {
  folders: string[];
  includeContent: boolean;
  updateOnStartup: boolean;
}

export interface IndexStatus {
  exists: boolean;
  files: number;
  lastBuilt: number | null;
  sizeBytes: number;
  folders: string[];
  includeContent: boolean;
}

export interface Rule {
  id: string;
  name: string;
  source: string;
  includeSubfolders: boolean;
  extensions: string[];
  nameContains: string | null;
  olderThanDays: number | null;
  minSize: number | null;
  action: TransferKind;
  destination: string;
  policy: ConflictPolicy;
}

export interface PlanItem {
  source: string;
  target: string;
  size: number;
  conflict: boolean;
}

export interface Plan {
  items: PlanItem[];
  scanned: number;
  truncated: boolean;
  destinationExists: boolean;
}
