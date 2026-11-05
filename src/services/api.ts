// Typed wrappers around the backend commands. The UI never calls `invoke` directly.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ApiError,
  ClipboardFiles,
  ConflictCheck,
  ConflictPolicy,
  DirListing,
  DriveInfo,
  KnownFolder,
  OperationProgress,
  OperationReport,
  Preview,
  RecycleResult,
  ResolvedPath,
  TransferKind,
} from "../types/fs";
import type { SystemInfo } from "../features/help/report";
import type { CheckResult, UpdateInfo, UpdateProgress, UpdateSnapshot, UserUpdateSettings } from "../features/updates/types";
import type { ConnectOutcome, NetworkFavorite, NetworkOverview, ParsedNetworkPath, SharesResult, StatusReport } from "../types/network";
import type { DuplicateRemoval, IndexSettings, IndexStatus, Plan, Rule, SearchQuery, SearchResult, TaskProgress } from "../types/tools";

export function isApiError(value: unknown): value is ApiError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

/** Turns anything thrown by a command into a readable message. */
export function errorMessage(err: unknown): string {
  if (isApiError(err)) return err.message;
  if (err instanceof Error) return err.message;
  return String(err);
}

export const api = {
  listDir: (path: string, showHidden: boolean) => invoke<DirListing>("list_dir", { path, showHidden }),
  listDrives: () => invoke<DriveInfo[]>("list_drives"),
  knownFolders: () => invoke<KnownFolder[]>("known_folders"),
  resolvePath: (input: string) => invoke<ResolvedPath>("resolve_path", { input }),
  startupLocation: () => invoke<ResolvedPath | null>("startup_location"),
  createFolder: (parent: string, name: string) => invoke<string>("create_folder", { parent, name }),
  renameEntry: (path: string, newName: string) => invoke<string>("rename_entry", { path, newName }),
  recycleEntries: (paths: string[]) => invoke<RecycleResult>("recycle_entries", { paths }),
  /** Duplicate removal: the backend keeps at least one verified copy per group. */
  recycleDuplicates: (groups: DuplicateRemoval[]) => invoke<RecycleResult>("recycle_duplicates", { groups }),
  /** Opens a project web page (GitHub, Discord) in the browser; other addresses are refused. */
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  /** Opens a third-party project page listed in the bundled license index. */
  openProjectUrl: (url: string) => invoke<void>("open_project_url", { url }),
  systemInfo: () => invoke<SystemInfo>("system_info"),
  openPath: (path: string, allowExecutable = false) => invoke<void>("open_path", { path, allowExecutable }),
  showProperties: (path: string) => invoke<void>("show_properties", { path }),
  findConflicts: (sources: string[], destination: string) =>
    invoke<ConflictCheck>("find_conflicts", { sources, destination }),
  startOperation: (kind: TransferKind, sources: string[], destination: string, policy: ConflictPolicy) =>
    invoke<number>("start_operation", { kind, sources, destination, policy }),
  cancelOperation: (id: number) => invoke<boolean>("cancel_operation", { id }),
  listOperations: () => invoke<OperationProgress[]>("list_operations"),
  operationReport: (id: number) => invoke<OperationReport | null>("operation_report", { id }),
  clipboardSetFiles: (paths: string[], cut: boolean) => invoke<void>("clipboard_set_files", { paths, cut }),
  clipboardGetFiles: () => invoke<ClipboardFiles | null>("clipboard_get_files"),
  previewPath: (path: string) => invoke<Preview>("preview_path", { path }),
  // Phase 3 tools: background tasks report through "task-progress" events.
  startStorageScan: (path: string) => invoke<number>("start_storage_scan", { path }),
  startDuplicateScan: (roots: string[], minSize: number, includeHidden: boolean) =>
    invoke<number>("start_duplicate_scan", { roots, minSize, includeHidden }),
  startSearch: (query: SearchQuery) => invoke<number>("start_search", { query }),
  indexSearch: (query: SearchQuery) => invoke<SearchResult>("index_search", { query }),
  cancelTask: (id: number) => invoke<boolean>("cancel_task", { id }),
  taskStatus: (id: number) => invoke<TaskProgress | null>("task_status", { id }),
  taskResult: <T>(id: number) => invoke<T | null>("task_result", { id }),
  onTaskProgress: (handler: (p: TaskProgress) => void): Promise<UnlistenFn> =>
    listen<TaskProgress>("task-progress", (event) => handler(event.payload)),
  indexGetSettings: () => invoke<IndexSettings>("index_get_settings"),
  indexSaveSettings: (settings: IndexSettings) => invoke<void>("index_save_settings", { settings }),
  indexStatus: () => invoke<IndexStatus>("index_status"),
  indexBuild: () => invoke<number>("index_build"),
  indexDelete: () => invoke<void>("index_delete"),
  rulesGet: () => invoke<Rule[]>("rules_get"),
  rulesSave: (rules: Rule[]) => invoke<void>("rules_save", { rules }),
  organizePreview: (rule: Rule) => invoke<Plan>("organize_preview", { rule }),
  organizeApply: (rule: Rule, sources: string[]) => invoke<number>("organize_apply", { rule, sources }),
  // Network storage: Windows' own connections and SMB client; servers are contacted only
  // after a user action (open, search, connect, check).
  networkParse: (input: string) => invoke<ParsedNetworkPath>("network_parse", { input }),
  networkOverview: () => invoke<NetworkOverview>("network_overview"),
  networkProbe: (path: string) => invoke<StatusReport>("network_probe", { path }),
  networkListShares: (server: string) => invoke<SharesResult>("network_list_shares", { server }),
  networkDiscover: () => invoke<number>("network_discover"),
  /** `signIn` shows Windows' own sign-in dialog; Nexdir never sees the credentials. */
  networkConnect: (path: string, signIn: boolean) => invoke<ConnectOutcome>("network_connect", { path, signIn }),
  networkDisconnect: (path: string) => invoke<StatusReport>("network_disconnect", { path }),
  networkFavoritesGet: () => invoke<NetworkFavorite[]>("network_favorites_get"),
  networkFavoritesSave: (favorites: NetworkFavorite[]) => invoke<NetworkFavorite[]>("network_favorites_save", { favorites }),
  /** Closes Nexdir after cancelling running copy/move jobs (the user confirmed it). */
  confirmClose: () => invoke<void>("confirm_close"),
  onCloseBlocked: (handler: (running: number) => void): Promise<UnlistenFn> => listen<number>("close-blocked", (event) => handler(event.payload)),
  // Updates: checks contact GitHub only when the user clicks or enabled automatic checks.
  updateInfo: () => invoke<UpdateInfo>("update_info"),
  updateAckLastInstall: () => invoke<void>("update_ack_last_install"),
  updateSaveSettings: (settings: UserUpdateSettings) => invoke<UpdateInfo>("update_save_settings", { settings }),
  updateCheck: (automatic: boolean) => invoke<CheckResult>("update_check", { automatic }),
  updateDownload: () => invoke<UpdateSnapshot>("update_download"),
  updateCancel: () => invoke<UpdateSnapshot>("update_cancel"),
  updateSnooze: (version: string) => invoke<void>("update_snooze", { version }),
  updateInstall: () => invoke<void>("update_install"),
  updateOpenRelease: () => invoke<void>("update_open_release"),
  onUpdateState: (handler: (s: UpdateSnapshot) => void): Promise<UnlistenFn> => listen<UpdateSnapshot>("update-state", (event) => handler(event.payload)),
  onUpdateProgress: (handler: (p: UpdateProgress) => void): Promise<UnlistenFn> =>
    listen<UpdateProgress>("update-progress", (event) => handler(event.payload)),
  onOperationProgress: (handler: (p: OperationProgress) => void): Promise<UnlistenFn> =>
    listen<OperationProgress>("operation-progress", (event) => handler(event.payload)),
};
