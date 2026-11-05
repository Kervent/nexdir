import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  ArrowRightLeft,
  BadgeInfo,
  ArrowUpDown,
  BookOpen,
  Bug,
  ChartPie,
  CircleHelp,
  CodeXml,
  LifeBuoy,
  Lightbulb,
  MessagesSquare,
  Sparkles,
  Files,
  Search,
  Clipboard,
  ClipboardPaste,
  Columns2,
  Copy,
  Eye,
  EyeOff,
  FolderInput,
  FolderOpen,
  FolderPlus,
  Info,
  Keyboard,
  LayoutGrid,
  List,
  ListChecks,
  Monitor,
  Moon,
  PanelRight,
  PanelsTopLeft,
  Pin,
  RefreshCw,
  Scissors,
  Settings,
  ArrowUpCircle,
  Sun,
  TextCursorInput,
  Trash2,
} from "lucide-react";
import { api, errorMessage, isApiError } from "../services/api";
import { ContextMenu, type MenuEntry } from "../components/ContextMenu";
import { useNotify } from "../components/Toasts";
import { ExplorerPane, type ExplorerPaneHandle } from "../features/explorer/ExplorerPane";
import { ConfirmDialog, ConflictDialog, NameDialog, ReportDialog } from "../features/explorer/dialogs";
import { Sidebar } from "../features/navigation/Sidebar";
import { usePreferences, type ThemePreference } from "../features/settings/preferences";
import { ShortcutsDialog } from "../features/settings/ShortcutsDialog";
import { AboutDialog, type AboutSection } from "../features/settings/AboutDialog";
import { SettingsDialog, type SettingsSection } from "../features/settings/SettingsDialog";
import { UpdateDialog } from "../features/updates/UpdateDialog";
import { UpdateNotice } from "../features/updates/UpdateNotice";
import { useUpdates } from "../features/updates/useUpdates";
import { HelpCenter, type HelpAction } from "../features/help/HelpCenter";
import { IssueDialog } from "../features/help/IssueDialog";
import { userNamesFrom, type IssueKind } from "../features/help/report";
import { Tour, WelcomeDialog } from "../features/onboarding/Tour";
import { loadOnboarding, recordTour, shouldOfferTour, TOUR_STEPS } from "../features/onboarding/onboarding";
import { discordInvite, LINKS } from "../config/links";
import { OperationsPanel } from "../features/transfers/OperationsPanel";
import { useOperations } from "../features/transfers/useOperations";
import { PreviewPanel } from "../features/preview/PreviewPanel";
import { TabBar } from "../features/workspace/TabBar";
import { useVisibleEntries, useWorkspace } from "../features/workspace/useWorkspace";
import { currentLocation, newId, parentPath, type Location, type PaneState } from "../features/workspace/workspace";
import type { NetworkActions } from "../features/network/NetworkView";
import { moveBlockedReason, serverOnly } from "../features/network/networkPaths";
import type { NetworkFavorite } from "../types/network";
import { asEntry, type ToolContext } from "../features/tools/shared";
import type { DropEffect } from "../features/dnd/dnd";
import { formatSize, percent, pluralize } from "../lib/format";
import { isExecutable, isNavigable } from "../lib/fileTypes";
import { baseName, samePath, uniqueName } from "../lib/paths";
import { selectAll } from "../lib/selection";
import type { SortKey } from "../lib/sort";
import type { DuplicateRemoval } from "../types/tools";
import type { ConflictPolicy, DriveInfo, FileEntry, KnownFolder, OperationProgress, OperationReport, TransferKind } from "../types/fs";

type DialogState =
  | { type: "newFolder"; parent: string; suggested: string; paneId: string }
  | { type: "rename"; entry: FileEntry; paneId: string }
  | { type: "recycle"; entries: FileEntry[]; paneId: string; onDone?: (recycled: string[]) => void; duplicates?: DuplicateRemoval[] }
  | { type: "conflict"; kind: TransferKind; sources: string[]; destination: string; conflicts: string[] }
  | { type: "runExecutable"; entry: FileEntry }
  | { type: "report"; title: string; report: OperationReport | null }
  | { type: "shortcuts" }
  | { type: "about"; section?: AboutSection }
  | { type: "settings"; section?: SettingsSection }
  | { type: "update" }
  | { type: "closeBlocked"; running: number }
  | { type: "welcome" }
  | { type: "help" }
  | { type: "issue"; kind: IssueKind };

interface ClipboardState {
  mode: TransferKind;
  paths: string[];
}

interface MenuState {
  x: number;
  y: number;
  items: MenuEntry[];
}

/** Only renames are undoable: they can be reverted reliably as long as the old name is free. */
interface UndoableRename {
  paneId: string;
  from: string;
  to: string;
}

const THEME_ORDER: ThemePreference[] = ["system", "light", "dark"];
const THEME_ICON = { system: Monitor, light: Sun, dark: Moon };
const SORT_LABELS: Record<SortKey, string> = { name: "Name", modified: "Date modified", type: "Type", size: "Size" };

function isTextInput(el: EventTarget | null): boolean {
  return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || (el instanceof HTMLElement && el.isContentEditable);
}

const EMPTY_SET: ReadonlySet<string> = new Set();
const DISCORD_INVITE = discordInvite();

export function App() {
  const notify = useNotify();
  const [prefs, updatePrefs] = usePreferences();
  const { ws, dispatch, tab, pane, up, navigate, openTab, closeTab, toggleSplit } = useWorkspace();
  const entries = useVisibleEntries(pane, prefs.sort);
  const location = currentLocation(pane);
  const currentFolder = location.type === "folder" ? location.path : null;
  const selectedEntries = useMemo(() => entries.filter((e) => pane.selection.selected.has(e.path)), [entries, pane.selection.selected]);
  const otherPane = tab.panes.length === 2 ? ws.panes[tab.panes.find((id) => id !== pane.id)!]! : null;
  const otherFolder = otherPane && currentLocation(otherPane).type === "folder" ? (currentLocation(otherPane) as { path: string }).path : null;

  const [knownFolders, setKnownFolders] = useState<KnownFolder[]>([]);
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [dialog, setDialog] = useState<DialogState | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [clipboard, setClipboard] = useState<ClipboardState | null>(null);
  const [opsOpen, setOpsOpen] = useState(false);
  const [lastRename, setLastRename] = useState<UndoableRename | null>(null);
  const [touring, setTouring] = useState(false);
  const [networkFavorites, setNetworkFavorites] = useState<NetworkFavorite[]>([]);
  const paneRefs = useRef(new Map<string, ExplorerPaneHandle>());
  const activeHandle = () => paneRefs.current.get(pane.id);

  const wsRef = useRef(ws);
  wsRef.current = ws;

  // ---- Saved network locations (read from Nexdir's settings; no network access) ----

  useEffect(() => {
    api
      .networkFavoritesGet()
      .then(setNetworkFavorites)
      .catch(() => {});
  }, []);

  // ---- Drives and known folders -------------------------------------------------

  const drivesFailed = useRef(false);
  const loadDrives = useCallback(() => {
    api
      .listDrives()
      .then((list) => {
        drivesFailed.current = false;
        setDrives(list);
      })
      .catch((e) => {
        // Drives reload on every window focus; report a failure once, not on each retry.
        if (!drivesFailed.current) notify("error", "Could not list drives.", errorMessage(e));
        drivesFailed.current = true;
      });
  }, [notify]);

  useEffect(() => {
    api.knownFolders().then(setKnownFolders).catch(() => {});
    api
      .startupLocation()
      .then((start) => start && navigate(pane.id, { type: "folder", path: start.path }, start.select ?? undefined))
      .catch(() => {});
    // First start (or a new tour version): offer the tour. Only skip or completion stop it.
    if (shouldOfferTour(loadOnboarding())) {
      recordTour("offered");
      setDialog({ type: "welcome" });
    }
  }, []); // once at startup

  useEffect(() => {
    loadDrives();
    const onFocus = () => loadDrives();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [loadDrives]);

  useEffect(() => {
    if (location.type === "home") loadDrives();
  }, [location, loadDrives]);

  /** Reloads every pane that shows one of the given folders. */
  const refreshFolders = useCallback(
    (folders: string[]) => {
      for (const p of Object.values(wsRef.current.panes)) {
        const loc = currentLocation(p);
        if (loc.type === "folder" && folders.some((f) => samePath(f, loc.path))) dispatch({ type: "refresh", pane: p.id });
      }
    },
    [dispatch],
  );

  const refreshAllFolders = useCallback(() => {
    for (const p of Object.values(wsRef.current.panes)) {
      if (currentLocation(p).type === "folder") dispatch({ type: "refresh", pane: p.id });
    }
  }, [dispatch]);

  // ---- Operations ---------------------------------------------------------------

  const onOperationFinished = useCallback(
    (p: OperationProgress) => {
      // Sources and destination may be visible in any pane.
      refreshAllFolders();
      const problems = p.failureCount + p.skippedCount;
      const verb = p.kind === "copy" ? "Copy" : "Move";
      if (p.state === "cancelled") notify("info", "Operation cancelled.", `${pluralize(p.doneFiles, "file")} had already been processed.`);
      else if (problems > 0) {
        notify("error", `${verb} finished with problems.`, `${pluralize(p.failureCount, "error")}, ${p.skippedCount} skipped. Open File operations for details.`);
        setOpsOpen(true);
      } else notify("success", `${p.kind === "copy" ? "Copied" : "Moved"} ${pluralize(p.doneFiles, "file")}.`);
    },
    [notify, refreshAllFolders],
  );
  const ops = useOperations(onOperationFinished);

  // ---- Updates and closing --------------------------------------------------------

  const updates = useUpdates({
    onLastInstall: (version, installed) =>
      installed
        ? notify("success", `Nexdir was updated to ${version}.`)
        : notify("error", `The update to ${version} was not installed.`, "The installer was cancelled or failed; your current version keeps working. You can try again from Help → Check for Updates."),
  });

  const openUpdates = useCallback(() => {
    setMenu(null);
    setDialog({ type: "update" });
    // Opening the window starts a check unless one already has a result or a download runs.
    if (!updates.state || updates.state.phase === "idle" || updates.state.phase === "error" || updates.state.phase === "upToDate") void updates.check();
  }, [updates]);

  useEffect(() => {
    const unlisten = api.onCloseBlocked((running) => setDialog({ type: "closeBlocked", running }));
    return () => void unlisten.then((f) => f()).catch(() => {});
  }, []);

  const startTransfer = useCallback(
    async (kind: TransferKind, sources: string[], destination: string, policy: ConflictPolicy) => {
      try {
        await api.startOperation(kind, sources, destination, policy);
      } catch (e) {
        notify("error", "The operation could not be started.", errorMessage(e));
      }
    },
    [notify],
  );

  /** Shared by paste and drag-and-drop: checks for name conflicts, then starts the job. */
  const transferInto = useCallback(
    async (kind: TransferKind, sources: string[], destination: string) => {
      const blocked = kind === "move" ? moveBlockedReason(sources, destination, drives) : null;
      if (blocked) return notify("error", "This move is not possible yet. Nothing was changed.", blocked);
      try {
        const check = await api.findConflicts(sources, destination);
        if (check.sameFolder) {
          // Copying into the item's own folder makes numbered duplicates; moving there does nothing.
          if (kind === "copy") await startTransfer("copy", sources, destination, "keepBoth");
          return;
        }
        if (check.conflicts.length > 0) setDialog({ type: "conflict", kind, sources, destination, conflicts: check.conflicts });
        else await startTransfer(kind, sources, destination, "skip");
      } catch (e) {
        notify("error", `Cannot ${kind} here.`, errorMessage(e));
      }
    },
    [notify, startTransfer, drives],
  );

  const onDropPaths = useCallback(
    (paths: string[], effect: DropEffect, target: string) => void transferInto(effect, paths, target),
    [transferInto],
  );

  // ---- Clipboard ------------------------------------------------------------------

  const putOnClipboard = useCallback(
    (mode: TransferKind, items: FileEntry[]) => {
      if (!items.length) return;
      const paths = items.map((e) => e.path);
      setClipboard({ mode, paths });
      // Also place the files on the Windows clipboard so Explorer and other apps can paste them.
      api.clipboardSetFiles(paths, mode === "move").catch((e) => notify("error", "Could not use the Windows clipboard.", errorMessage(e)));
    },
    [notify],
  );

  const paste = useCallback(
    async (destination: string | null = currentFolder) => {
      if (!destination) return;
      let source: ClipboardState | null = null;
      try {
        const files = await api.clipboardGetFiles();
        if (files) source = { mode: files.cut ? "move" : "copy", paths: files.paths };
      } catch {
        // Fall back to Nexdir's own clipboard below.
      }
      source ??= clipboard;
      if (!source) return notify("info", "There is nothing to paste.");
      if (source.mode === "move") setClipboard(null);
      await transferInto(source.mode, source.paths, destination);
    },
    [clipboard, currentFolder, notify, transferInto],
  );

  // ---- Item actions ---------------------------------------------------------------

  const openEntry = useCallback(
    (p: PaneState, entry: FileEntry) => {
      if (isNavigable(entry)) return navigate(p.id, { type: "folder", path: entry.path });
      if (isExecutable(entry)) return setDialog({ type: "runExecutable", entry });
      api.openPath(entry.path).catch((e) => notify("error", `Could not open "${entry.name}".`, errorMessage(e)));
    },
    [navigate, notify],
  );

  const copyPaths = useCallback(
    (items: FileEntry[]) => {
      navigator.clipboard
        .writeText(items.map((e) => e.path).join("\r\n"))
        .then(() => notify("success", items.length === 1 ? "Path copied." : `${items.length} paths copied.`))
        .catch(() => notify("error", "Could not access the clipboard."));
    },
    [notify],
  );

  const recycle = useCallback(
    async (items: FileEntry[], duplicates?: DuplicateRemoval[]): Promise<string[]> => {
      let recycled: string[] = [];
      try {
        const result = duplicates ? await api.recycleDuplicates(duplicates) : await api.recycleEntries(items.map((e) => e.path));
        recycled = result.recycled;
        if (result.recycled.length > 0) notify("success", `Moved ${pluralize(result.recycled.length, "item")} to the Recycle Bin.`);
        if (result.failed.length > 0) {
          setDialog({
            type: "report",
            title: duplicates ? "Some copies were not removed" : "Some items were not deleted",
            report: { completed: result.recycled, failures: result.failed, skipped: [], truncated: false },
          });
        }
      } catch (e) {
        notify("error", "Could not move items to the Recycle Bin.", errorMessage(e));
      }
      refreshAllFolders();
      return recycled;
    },
    [notify, refreshAllFolders],
  );

  const newFolder = useCallback(() => {
    if (!currentFolder) return;
    const names = pane.listing?.entries.map((e) => e.name) ?? [];
    setDialog({ type: "newFolder", parent: currentFolder, suggested: uniqueName("New folder", names), paneId: pane.id });
  }, [currentFolder, pane]);

  const pin = useCallback(
    (path: string) => {
      if (prefs.pinned.some((p) => samePath(p.path, path))) return notify("info", `"${baseName(path)}" is already pinned.`);
      updatePrefs({ pinned: [...prefs.pinned, { path, label: baseName(path) }] });
      notify("success", `Pinned "${baseName(path)}" to the sidebar.`);
    },
    [prefs.pinned, updatePrefs, notify],
  );

  const undoRename = useCallback(async () => {
    if (!lastRename) return notify("info", "There is nothing to undo.");
    try {
      await api.renameEntry(lastRename.to, baseName(lastRename.from));
      notify("success", `Renamed back to "${baseName(lastRename.from)}".`);
      dispatch({ type: "refresh", pane: lastRename.paneId, select: baseName(lastRename.from) });
    } catch (e) {
      notify("error", "The rename could not be undone.", errorMessage(e));
    }
    setLastRename(null);
  }, [lastRename, notify, dispatch]);

  const showReport = useCallback(async (p: OperationProgress) => {
    const report = await api.operationReport(p.id).catch(() => null);
    setDialog({ type: "report", title: `${p.kind === "copy" ? "Copy" : "Move"} details`, report });
  }, []);

  const toolContext = useCallback(
    (p: PaneState): ToolContext => ({
      navigate: (to) => navigate(p.id, to),
      openInNewTab: (to) => openTab(to, true),
      openEntry: (entry) => openEntry(p, entry),
      // Opens in a new tab so the tool keeps its results.
      reveal: (path) => {
        const parent = parentPath(path);
        if (parent) openTab({ type: "folder", path: parent }, false, baseName(path));
      },
      requestRecycle: (paths, onDone) => setDialog({ type: "recycle", entries: paths.map((x) => asEntry(x)), paneId: p.id, onDone }),
      requestRecycleDuplicates: (groups, onDone) =>
        setDialog({ type: "recycle", entries: groups.flatMap((g) => g.remove).map((x) => asEntry(x)), paneId: p.id, onDone, duplicates: groups }),
      itemsMenu: (e, items, options) => {
        e.preventDefault();
        const entries = itemMenuRef.current(p, items).filter((m) => options?.recycle !== false || m === "separator" || m.id !== "recycle");
        setMenu({ x: e.clientX, y: e.clientY, items: entries });
      },
      notify,
      knownFolders,
      drives,
      pinned: prefs.pinned,
      prefs,
      updatePrefs,
    }),
    [navigate, openTab, openEntry, notify, knownFolders, drives, prefs, updatePrefs],
  );

  const showProperties = useCallback(
    (path: string) => api.showProperties(path).catch((e) => notify("error", "Could not show properties.", errorMessage(e))),
    [notify],
  );

  const saveNetworkFavorites = useCallback(
    async (list: NetworkFavorite[]) => {
      try {
        setNetworkFavorites(await api.networkFavoritesSave(list));
        return true;
      } catch (e) {
        notify("error", "Could not save the network location.", errorMessage(e));
        return false;
      }
    },
    [notify],
  );

  const networkActions = useCallback(
    (p: PaneState): NetworkActions => ({
      navigate: (to) => navigate(p.id, to),
      openInNewTab: (to) => openTab(to, true),
      openInOtherPane: (path) => {
        const t = wsRef.current.tabs.find((x) => x.panes.includes(p.id));
        const other = t?.panes.find((id) => id !== p.id);
        if (other) return navigate(other, { type: "folder", path });
        // Not split yet: open split view with the share in the new pane.
        const id = newId("p");
        dispatch({ type: "toggleSplit", paneId: id });
        dispatch({ type: "navigate", pane: id, to: { type: "folder", path } });
      },
      showProperties,
      notify,
      favorites: networkFavorites,
      saveFavorites: saveNetworkFavorites,
    }),
    [navigate, openTab, dispatch, showProperties, notify, networkFavorites, saveNetworkFavorites],
  );

  /** "Sign in…" on a network folder that refused access: Windows' own dialog, then reload. */
  const networkSignIn = useCallback(
    async (p: PaneState, path: string) => {
      try {
        const out = await api.networkConnect(path, true);
        if (out.report.status === "connected") dispatch({ type: "refresh", pane: p.id });
        else if (out.report.status !== "cancelled") notify("error", out.report.message, out.report.code !== null ? `Windows error ${out.report.code}` : undefined);
      } catch (e) {
        notify("error", "Could not connect.", errorMessage(e));
      }
    },
    [dispatch, notify],
  );

  const submitAddress = useCallback(
    async (p: PaneState, text: string): Promise<string | null> => {
      // A server alone (\\NAS) is not a folder: show its shares in the Network view.
      const server = serverOnly(text);
      if (server) {
        navigate(p.id, { type: "network", server });
        return null;
      }
      try {
        const resolved = await api.resolvePath(text);
        navigate(p.id, { type: "folder", path: resolved.path }, resolved.select ?? undefined);
        return null;
      } catch (e) {
        return isApiError(e) && e.code === "notFound" ? `Windows can't find "${text}". Check the spelling and try again.` : errorMessage(e);
      }
    },
    [navigate],
  );

  // ---- Help, tour and issue reports ----------------------------------------------

  const openUrl = useCallback(
    (url: string) => api.openUrl(url).catch((e) => notify("error", "Could not open the browser.", errorMessage(e))),
    [notify],
  );

  const startTour = useCallback(() => {
    setDialog(null);
    setMenu(null);
    setTouring(true);
  }, []);

  const helpAction = useCallback(
    (action: HelpAction) => {
      switch (action) {
        case "tour":
          return startTour();
        case "shortcuts":
          return setDialog({ type: "shortcuts" });
        case "about":
          return setDialog({ type: "about" });
        case "bug":
        case "feature":
          return setDialog({ type: "issue", kind: action });
        case "documentation":
          return void openUrl(LINKS.documentation);
        case "repository":
          return void openUrl(LINKS.repository);
        case "discord":
          return DISCORD_INVITE ? void openUrl(DISCORD_INVITE) : notify("info", "The Discord invite is not available in this version yet.");
      }
    },
    [startTour, openUrl, notify],
  );

  const helpMenu = useCallback(
    (): MenuEntry[] => [
      { id: "help", label: "Help & Support", icon: <LifeBuoy size={15} />, onSelect: () => setDialog({ type: "help" }) },
      { id: "tour", label: "Take a Tour", icon: <Sparkles size={15} />, onSelect: () => helpAction("tour") },
      { id: "shortcuts", label: "Keyboard shortcuts", icon: <Keyboard size={15} />, shortcut: "F1", onSelect: () => helpAction("shortcuts") },
      { id: "updates", label: "Check for Updates…", icon: <ArrowUpCircle size={15} />, onSelect: openUpdates },
      "separator",
      { id: "documentation", label: "Documentation", icon: <BookOpen size={15} />, onSelect: () => helpAction("documentation") },
      { id: "bug", label: "Report a Bug…", icon: <Bug size={15} />, onSelect: () => helpAction("bug") },
      { id: "feature", label: "Request a Feature…", icon: <Lightbulb size={15} />, onSelect: () => helpAction("feature") },
      { id: "repository", label: "GitHub Repository", icon: <CodeXml size={15} />, onSelect: () => helpAction("repository") },
      { id: "discord", label: "Discord Community", icon: <MessagesSquare size={15} />, disabled: !DISCORD_INVITE, onSelect: () => helpAction("discord") },
      "separator",
      { id: "about", label: "About Nexdir", icon: <BadgeInfo size={15} />, onSelect: () => helpAction("about") },
    ],
    [helpAction, openUpdates],
  );

  const userNames = useMemo(() => userNamesFrom(knownFolders.find((f) => /[\\/]Users[\\/]/i.test(f.path))?.path), [knownFolders]);

  // ---- Context menu ------------------------------------------------------------

  const itemMenu = useCallback(
    (p: PaneState, items: FileEntry[]): MenuEntry[] => {
      const single = items.length === 1 ? items[0]! : null;
      const folder = single && isNavigable(single) ? single : null;
      const paths = items.map((e) => e.path);
      const entries: MenuEntry[] = [
        { id: "open", label: "Open", icon: <FolderOpen size={15} />, shortcut: "Enter", disabled: !single, onSelect: () => single && openEntry(p, single) },
        { id: "openTab", label: "Open in new tab", icon: <PanelsTopLeft size={15} />, disabled: !folder, onSelect: () => folder && openTab({ type: "folder", path: folder.path }, true) },
      ];
      if (folder) {
        const at = folder.path;
        entries.push(
          { id: "searchHere", label: "Search in this folder", icon: <Search size={15} />, onSelect: () => openTab({ type: "tool", tool: "search", path: at }) },
          { id: "analyze", label: "Analyze storage", icon: <ChartPie size={15} />, onSelect: () => openTab({ type: "tool", tool: "storage", path: at }) },
          { id: "dupes", label: "Find duplicates", icon: <Files size={15} />, onSelect: () => openTab({ type: "tool", tool: "duplicates", path: at }) },
        );
      }
      if (otherPane) {
        entries.push({ id: "openOther", label: "Open in other pane", icon: <Columns2 size={15} />, disabled: !folder, onSelect: () => folder && navigate(otherPane.id, { type: "folder", path: folder.path }) });
      }
      entries.push(
        "separator",
        { id: "cut", label: "Cut", icon: <Scissors size={15} />, shortcut: "Ctrl+X", onSelect: () => putOnClipboard("move", items) },
        { id: "copy", label: "Copy", icon: <Copy size={15} />, shortcut: "Ctrl+C", onSelect: () => putOnClipboard("copy", items) },
      );
      if (otherFolder) {
        entries.push(
          { id: "copyOther", label: "Copy to other pane", icon: <FolderInput size={15} />, onSelect: () => void transferInto("copy", paths, otherFolder) },
          { id: "moveOther", label: "Move to other pane", icon: <ArrowRightLeft size={15} />, onSelect: () => void transferInto("move", paths, otherFolder) },
        );
      }
      entries.push(
        { id: "copyPath", label: items.length > 1 ? "Copy paths" : "Copy path", icon: <Clipboard size={15} />, shortcut: "Ctrl+Shift+C", onSelect: () => copyPaths(items) },
        "separator",
        { id: "rename", label: "Rename", icon: <TextCursorInput size={15} />, shortcut: "F2", disabled: !single, onSelect: () => single && setDialog({ type: "rename", entry: single, paneId: p.id }) },
        { id: "recycle", label: "Move to Recycle Bin", icon: <Trash2 size={15} />, shortcut: "Del", danger: true, onSelect: () => setDialog({ type: "recycle", entries: items, paneId: p.id }) },
        "separator",
        { id: "pin", label: "Pin to sidebar", icon: <Pin size={15} />, disabled: !folder, onSelect: () => folder && pin(folder.path) },
        { id: "properties", label: "Properties", icon: <Info size={15} />, shortcut: "Alt+Enter", disabled: !single, onSelect: () => single && void showProperties(single.path) },
      );
      return entries;
    },
    [openEntry, openTab, otherPane, otherFolder, navigate, putOnClipboard, transferInto, copyPaths, pin, showProperties],
  );

  // Tool views are created before itemMenu is defined; they reach it through this ref.
  const itemMenuRef = useRef(itemMenu);
  itemMenuRef.current = itemMenu;

  const backgroundMenu = useCallback(
    (p: PaneState, folder: string): MenuEntry[] => [
      { id: "paste", label: "Paste", icon: <ClipboardPaste size={15} />, shortcut: "Ctrl+V", onSelect: () => void paste(folder) },
      { id: "newFolder", label: "New folder", icon: <FolderPlus size={15} />, shortcut: "Ctrl+Shift+N", onSelect: newFolder },
      "separator",
      { id: "refresh", label: "Refresh", icon: <RefreshCw size={15} />, shortcut: "F5", onSelect: () => dispatch({ type: "refresh", pane: p.id }) },
      { id: "selectAll", label: "Select all", icon: <ListChecks size={15} />, shortcut: "Ctrl+A", onSelect: () => dispatch({ type: "select", pane: p.id, selection: selectAll(entries.map((e) => e.path)) }) },
      "separator",
      { id: "searchHere", label: "Search in this folder", icon: <Search size={15} />, shortcut: "Ctrl+Shift+F", onSelect: () => openTab({ type: "tool", tool: "search", path: folder }) },
      { id: "analyze", label: "Analyze storage", icon: <ChartPie size={15} />, onSelect: () => openTab({ type: "tool", tool: "storage", path: folder }) },
      { id: "dupes", label: "Find duplicates", icon: <Files size={15} />, onSelect: () => openTab({ type: "tool", tool: "duplicates", path: folder }) },
      "separator",
      { id: "pinHere", label: "Pin this folder", icon: <Pin size={15} />, shortcut: "Ctrl+D", onSelect: () => pin(folder) },
      { id: "copyPath", label: "Copy folder path", icon: <Clipboard size={15} />, onSelect: () => void navigator.clipboard.writeText(folder).catch(() => {}) },
      { id: "properties", label: "Properties", icon: <Info size={15} />, onSelect: () => void showProperties(folder) },
    ],
    [paste, newFolder, dispatch, entries, pin, showProperties],
  );

  const onContextMenu = useCallback(
    (p: PaneState, e: React.MouseEvent, entry: FileEntry | null) => {
      e.preventDefault();
      const loc = currentLocation(p);
      if (loc.type !== "folder") return;
      // The pane may not be active yet (right-click into the other pane); use its own entries.
      const listed = p.listing?.entries ?? [];
      let items: FileEntry[] = [];
      if (entry) items = p.selection.selected.has(entry.path) ? listed.filter((x) => p.selection.selected.has(x.path)) : [entry];
      setMenu({ x: e.clientX, y: e.clientY, items: items.length ? itemMenu(p, items) : backgroundMenu(p, loc.path) });
    },
    [itemMenu, backgroundMenu],
  );

  // ---- Keyboard shortcuts ------------------------------------------------------

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (dialog || menu || touring) return;
      const typing = isTextInput(e.target);
      const key = e.key.toLowerCase();
      const handled = () => e.preventDefault();
      const tabIndex = ws.tabs.findIndex((t) => t.id === ws.activeTab);

      // Tabs, panes and window-level commands work everywhere.
      if (e.ctrlKey && key === "t") return handled(), openTab(currentFolder ? { type: "folder", path: currentFolder } : { type: "home" });
      if (e.ctrlKey && key === "w") return handled(), closeTab(ws.activeTab);
      if (e.ctrlKey && e.key === "Tab") {
        handled();
        const next = (tabIndex + (e.shiftKey ? -1 : 1) + ws.tabs.length) % ws.tabs.length;
        return dispatch({ type: "activateTab", tab: ws.tabs[next]!.id });
      }
      if (e.ctrlKey && /^[1-9]$/.test(e.key)) {
        handled();
        const target = e.key === "9" ? ws.tabs[ws.tabs.length - 1] : ws.tabs[Number(e.key) - 1];
        return target && dispatch({ type: "activateTab", tab: target.id });
      }
      if (e.ctrlKey && e.key === "\\") return handled(), toggleSplit();
      if (e.key === "F6" && otherPane) {
        handled();
        dispatch({ type: "focusPane", pane: otherPane.id });
        return window.setTimeout(() => paneRefs.current.get(otherPane.id)?.focusList(), 0);
      }
      if (e.altKey && key === "p") return handled(), updatePrefs({ showPreview: !prefs.showPreview });
      if (e.ctrlKey && e.shiftKey && key === "f") return handled(), navigate(pane.id, { type: "tool", tool: "search", path: currentFolder ?? undefined });
      if (e.key === "F1" || (e.ctrlKey && e.key === "/")) return handled(), setDialog({ type: "shortcuts" });

      if (e.altKey && e.key === "ArrowLeft") return handled(), dispatch({ type: "back", pane: pane.id });
      if (e.altKey && e.key === "ArrowRight") return handled(), dispatch({ type: "forward", pane: pane.id });
      if (e.altKey && e.key === "ArrowUp") return handled(), up(pane);
      if ((e.ctrlKey && key === "l") || (e.altKey && key === "d")) return handled(), activeHandle()?.editAddress();
      if (e.ctrlKey && (key === "f" || key === "e")) return handled(), activeHandle()?.focusFilter();
      if (e.key === "F5") return handled(), currentFolder ? dispatch({ type: "refresh", pane: pane.id }) : loadDrives();
      if (typing) return;

      if (e.key === "Backspace") return handled(), dispatch({ type: "back", pane: pane.id });
      if (e.ctrlKey && key === "z") return handled(), void undoRename();
      if (!currentFolder) return;
      if (e.ctrlKey && e.shiftKey && key === "n") return handled(), newFolder();
      if (e.ctrlKey && key === "d") return handled(), pin(currentFolder);
      if (e.ctrlKey && e.shiftKey && key === "c" && selectedEntries.length) return handled(), copyPaths(selectedEntries);
      if (e.ctrlKey && key === "c" && selectedEntries.length) return handled(), putOnClipboard("copy", selectedEntries);
      if (e.ctrlKey && key === "x" && selectedEntries.length) return handled(), putOnClipboard("move", selectedEntries);
      if (e.ctrlKey && key === "v") return handled(), void paste();
      if (e.key === "F2" && selectedEntries.length === 1) return handled(), setDialog({ type: "rename", entry: selectedEntries[0]!, paneId: pane.id });
      if (e.key === "Delete" && selectedEntries.length) return handled(), setDialog({ type: "recycle", entries: selectedEntries, paneId: pane.id });
      if (e.altKey && e.key === "Enter" && selectedEntries.length === 1) return handled(), void showProperties(selectedEntries[0]!.path);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Mouse back/forward buttons act on the active pane.
  useEffect(() => {
    const onMouse = (e: MouseEvent) => {
      if (e.button === 3) dispatch({ type: "back", pane: pane.id });
      if (e.button === 4) dispatch({ type: "forward", pane: pane.id });
    };
    window.addEventListener("mouseup", onMouse);
    return () => window.removeEventListener("mouseup", onMouse);
  }, [dispatch, pane.id]);

  // ---- Rendering ----------------------------------------------------------------

  const selectedSize = useMemo(() => selectedEntries.reduce((sum, e) => sum + (e.size ?? 0), 0), [selectedEntries]);
  const cutPaths = useMemo(() => (clipboard?.mode === "move" ? new Set(clipboard.paths) : EMPTY_SET), [clipboard]);
  const ThemeIcon = THEME_ICON[prefs.theme];
  const activeOp = ops.active[0];
  const previewPath = selectedEntries.length === 1 ? selectedEntries[0]!.path : selectedEntries.length === 0 ? currentFolder : null;

  return (
    <div className="nx-app">
      <TabBar
        ws={ws}
        onActivate={(id) => dispatch({ type: "activateTab", tab: id })}
        onClose={closeTab}
        onNew={() => openTab(currentFolder ? { type: "folder", path: currentFolder } : { type: "home" })}
        onMove={(id, toIndex) => dispatch({ type: "moveTab", tab: id, toIndex })}
      />

      <div className="nx-commandbar" role="toolbar" aria-label="Commands">
        <button type="button" className="nx-command" data-tour="file-ops" disabled={!currentFolder} onClick={newFolder} title="New folder (Ctrl+Shift+N)">
          <FolderPlus size={16} /> <span>New folder</span>
        </button>
        <span className="nx-command-sep" />
        <button type="button" className="nx-icon-button" aria-label="Cut (Ctrl+X)" title="Cut (Ctrl+X)" disabled={!selectedEntries.length} onClick={() => putOnClipboard("move", selectedEntries)}>
          <Scissors size={16} />
        </button>
        <button type="button" className="nx-icon-button" aria-label="Copy (Ctrl+C)" title="Copy (Ctrl+C)" disabled={!selectedEntries.length} onClick={() => putOnClipboard("copy", selectedEntries)}>
          <Copy size={16} />
        </button>
        <button type="button" className="nx-icon-button" aria-label="Paste (Ctrl+V)" title="Paste (Ctrl+V)" disabled={!currentFolder} onClick={() => void paste()}>
          <ClipboardPaste size={16} />
        </button>
        <button type="button" className="nx-icon-button" aria-label="Rename (F2)" title="Rename (F2)" disabled={selectedEntries.length !== 1} onClick={() => setDialog({ type: "rename", entry: selectedEntries[0]!, paneId: pane.id })}>
          <TextCursorInput size={16} />
        </button>
        <button type="button" className="nx-icon-button" data-tour="file-ops" aria-label="Move to Recycle Bin (Del)" title="Move to Recycle Bin (Del)" disabled={!selectedEntries.length} onClick={() => setDialog({ type: "recycle", entries: selectedEntries, paneId: pane.id })}>
          <Trash2 size={16} />
        </button>
        <span className="nx-command-sep" />
        <button
          type="button"
          className="nx-command"
          disabled={!currentFolder}
          title="Sort"
          onClick={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            const keys: SortKey[] = ["name", "modified", "type", "size"];
            setMenu({
              x: rect.left,
              y: rect.bottom + 4,
              items: [
                ...keys.map((key) => ({
                  id: key,
                  label: `${SORT_LABELS[key]}${prefs.sort.key === key ? " ✓" : ""}`,
                  onSelect: () => updatePrefs({ sort: { ...prefs.sort, key } }),
                })),
                "separator" as const,
                { id: "asc", label: `Ascending${prefs.sort.direction === "asc" ? " ✓" : ""}`, onSelect: () => updatePrefs({ sort: { ...prefs.sort, direction: "asc" } }) },
                { id: "desc", label: `Descending${prefs.sort.direction === "desc" ? " ✓" : ""}`, onSelect: () => updatePrefs({ sort: { ...prefs.sort, direction: "desc" } }) },
              ],
            });
          }}
        >
          <ArrowUpDown size={16} /> <span>Sort</span>
        </button>
        <div className="nx-segmented" role="group" aria-label="View" data-tour="appearance">
          <button type="button" className={prefs.viewMode === "details" ? "is-active" : ""} aria-pressed={prefs.viewMode === "details"} title="Details view" onClick={() => updatePrefs({ viewMode: "details" })}>
            <List size={16} />
          </button>
          <button type="button" className={prefs.viewMode === "icons" ? "is-active" : ""} aria-pressed={prefs.viewMode === "icons"} title="Icons view" onClick={() => updatePrefs({ viewMode: "icons" })}>
            <LayoutGrid size={16} />
          </button>
        </div>
        <button type="button" className={`nx-icon-button${prefs.showHidden ? " is-active" : ""}`} aria-pressed={prefs.showHidden} aria-label="Show hidden items" title={prefs.showHidden ? "Hide hidden items" : "Show hidden items"} onClick={() => updatePrefs({ showHidden: !prefs.showHidden })}>
          {prefs.showHidden ? <Eye size={16} /> : <EyeOff size={16} />}
        </button>
        <span className="nx-spacer" />
        <button type="button" data-tour="split" className={`nx-icon-button${tab.panes.length > 1 ? " is-active" : ""}`} aria-pressed={tab.panes.length > 1} aria-label="Split view (Ctrl+\)" title="Split view (Ctrl+\)" onClick={toggleSplit}>
          <Columns2 size={16} />
        </button>
        <button type="button" className={`nx-icon-button${prefs.showPreview ? " is-active" : ""}`} aria-pressed={prefs.showPreview} aria-label="Preview pane (Alt+P)" title="Preview pane (Alt+P)" onClick={() => updatePrefs({ showPreview: !prefs.showPreview })}>
          <PanelRight size={16} />
        </button>
        <button
          type="button"
          className="nx-icon-button"
          data-tour="appearance"
          aria-label={`Theme: ${prefs.theme}`}
          title={`Theme: ${prefs.theme} (click to change)`}
          onClick={() => updatePrefs({ theme: THEME_ORDER[(THEME_ORDER.indexOf(prefs.theme) + 1) % THEME_ORDER.length]! })}
        >
          <ThemeIcon size={16} />
        </button>
        <button type="button" className="nx-icon-button" aria-label="Settings" title="Settings" onClick={() => setDialog({ type: "settings" })}>
          <Settings size={16} />
        </button>
        <button
          type="button"
          className="nx-icon-button"
          data-tour="help"
          aria-label="Help"
          aria-haspopup="menu"
          title="Help, tour and support"
          onClick={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            setMenu({ x: rect.right - 230, y: rect.bottom + 4, items: helpMenu() });
          }}
        >
          <CircleHelp size={16} />
        </button>
        <button type="button" data-tour="operations" className={`nx-icon-button${opsOpen ? " is-active" : ""}`} aria-label="File operations" title="File operations" aria-expanded={opsOpen} onClick={() => setOpsOpen((o) => !o)}>
          <Activity size={16} />
          {ops.active.length > 0 && <span className="nx-badge">{ops.active.length}</span>}
        </button>
      </div>

      <div className="nx-body">
        <Sidebar
          location={location}
          knownFolders={knownFolders}
          drives={drives}
          pinned={prefs.pinned}
          networkFavorites={networkFavorites}
          onNavigate={(to) => navigate(pane.id, to)}
          onUnpin={(path) => updatePrefs({ pinned: prefs.pinned.filter((p) => !samePath(p.path, path)) })}
          onOpenInNewTab={(to) => openTab(to, true)}
          onDropPaths={onDropPaths}
        />
        <main className="nx-main">
          {/* Inactive tabs stay mounted (hidden) so running scans and results survive tab switches. */}
          {ws.tabs.map((t) => (
            <div key={t.id} className={`nx-tab-panel${t.panes.length > 1 ? " is-split" : ""}`} hidden={t.id !== ws.activeTab}>
              {t.panes.map((id, i) => {
                const p = ws.panes[id]!;
                return (
                  <Fragment key={id}>
                    {i > 0 && <div className="nx-pane-divider" role="separator" aria-orientation="vertical" />}
                    <ExplorerPane
                      ref={(h) => {
                        if (h) paneRefs.current.set(id, h);
                        else paneRefs.current.delete(id);
                      }}
                      pane={p}
                      active={id === pane.id}
                      split={t.panes.length > 1}
                      prefs={prefs}
                      updatePrefs={updatePrefs}
                      dispatch={dispatch}
                      drives={drives}
                      knownFolders={knownFolders}
                      cutPaths={cutPaths}
                      onUp={up}
                      onOpenEntry={openEntry}
                      onContextMenu={onContextMenu}
                      onDropPaths={onDropPaths}
                      onOpenInNewTab={(to: Location) => openTab(to, true)}
                      onSubmitAddress={submitAddress}
                      onRefreshHome={loadDrives}
                      toolContext={toolContext}
                      networkActions={networkActions}
                      onNetworkSignIn={(pp, path) => void networkSignIn(pp, path)}
                    />
                  </Fragment>
                );
              })}
            </div>
          ))}
        </main>
        {prefs.showPreview && <PreviewPanel path={previewPath} version={pane.reloadToken} onClose={() => updatePrefs({ showPreview: false })} />}
        {opsOpen && (
          <OperationsPanel
            operations={ops.operations}
            onCancel={(id) => void api.cancelOperation(id)}
            onDismiss={ops.dismiss}
            onShowReport={(p) => void showReport(p)}
            onClose={() => setOpsOpen(false)}
            rateOf={ops.rateOf}
          />
        )}
      </div>

      <footer className="nx-statusbar">
        {location.type === "folder" && pane.listing && (
          <>
            <span>{pluralize(entries.length, "item")}</span>
            {selectedEntries.length > 0 && (
              <span>
                {pluralize(selectedEntries.length, "item")} selected{selectedSize > 0 ? ` · ${formatSize(selectedSize)}` : ""}
              </span>
            )}
            {pane.listing.unreadable > 0 && <span className="is-warning">{pane.listing.unreadable} unreadable</span>}
            {clipboard && <span>{pluralize(clipboard.paths.length, "item")} on clipboard ({clipboard.mode === "copy" ? "copy" : "cut"})</span>}
          </>
        )}
        {location.type === "home" && <span>{pluralize(drives.length, "drive")}</span>}
        <span className="nx-spacer" />
        {activeOp && (
          <button type="button" className="nx-status-op" onClick={() => setOpsOpen(true)}>
            {activeOp.kind === "copy" ? "Copying" : "Moving"} · {percent(activeOp.doneBytes, activeOp.totalBytes)}%
            <span className="nx-progress is-inline">
              <span style={{ width: `${percent(activeOp.doneBytes, activeOp.totalBytes)}%` }} />
            </span>
          </button>
        )}
      </footer>

      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}

      {dialog?.type === "newFolder" && (
        <NameDialog
          title="New folder"
          label="Folder name"
          initial={dialog.suggested}
          confirmLabel="Create"
          onClose={() => setDialog(null)}
          onConfirm={async (name) => {
            try {
              await api.createFolder(dialog.parent, name);
              setDialog(null);
              dispatch({ type: "refresh", pane: dialog.paneId, select: name });
              refreshFolders([dialog.parent]);
              return null;
            } catch (e) {
              return errorMessage(e);
            }
          }}
        />
      )}
      {dialog?.type === "rename" && (
        <NameDialog
          title={`Rename "${dialog.entry.name}"`}
          label="New name"
          initial={dialog.entry.name}
          confirmLabel="Rename"
          onClose={() => setDialog(null)}
          onConfirm={async (name) => {
            if (name === dialog.entry.name) {
              setDialog(null);
              return null;
            }
            try {
              const to = await api.renameEntry(dialog.entry.path, name);
              setDialog(null);
              setLastRename({ paneId: dialog.paneId, from: dialog.entry.path, to });
              dispatch({ type: "refresh", pane: dialog.paneId, select: name });
              refreshAllFolders();
              return null;
            } catch (e) {
              return errorMessage(e);
            }
          }}
        />
      )}
      {dialog?.type === "recycle" && (
        <ConfirmDialog
          title="Move to Recycle Bin"
          message={
            dialog.entries.length === 1
              ? `Move "${dialog.entries[0]!.name}" to the Recycle Bin?`
              : `Move ${dialog.entries.length} items to the Recycle Bin?`
          }
          detail={
            <p className="nx-muted">
              {dialog.duplicates
                ? "At least one copy of every file is kept. Nexdir checks each file again before moving it and skips any that changed. "
                : ""}
              You can restore items from the Recycle Bin. Nexdir never deletes files permanently.
            </p>
          }
          confirmLabel="Move to Recycle Bin"
          danger
          onClose={() => setDialog(null)}
          onConfirm={() => void recycle(dialog.entries, dialog.duplicates).then((done) => dialog.onDone?.(done))}
        />
      )}
      {dialog?.type === "conflict" && (
        <ConflictDialog
          kind={dialog.kind}
          conflicts={dialog.conflicts}
          total={dialog.sources.length}
          onClose={() => setDialog(null)}
          onChoose={(policy) => void startTransfer(dialog.kind, dialog.sources, dialog.destination, policy)}
        />
      )}
      {dialog?.type === "runExecutable" && (
        <ConfirmDialog
          title="Run this file?"
          message={`"${dialog.entry.name}" is a program, script or shortcut. Opening it runs code on your computer.`}
          detail={<p className="nx-muted">Only continue if you trust where this file came from.</p>}
          confirmLabel="Run"
          onClose={() => setDialog(null)}
          onConfirm={() => api.openPath(dialog.entry.path, true).catch((e) => notify("error", "Could not open the file.", errorMessage(e)))}
        />
      )}
      {dialog?.type === "report" && <ReportDialog title={dialog.title} report={dialog.report} onClose={() => setDialog(null)} />}
      {dialog?.type === "shortcuts" && <ShortcutsDialog onClose={() => setDialog(null)} />}
      {dialog?.type === "about" && (
        <AboutDialog
          section={dialog.section}
          updateInfo={updates.info}
          onCheckForUpdates={openUpdates}
          openUrl={(url) => void openUrl(url)}
          openProjectUrl={(url) => api.openProjectUrl(url)}
          copyText={(text) => navigator.clipboard.writeText(text)}
          notify={notify}
          discordInvite={DISCORD_INVITE}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.type === "settings" && (
        <SettingsDialog
          prefs={prefs}
          updatePrefs={updatePrefs}
          updates={updates}
          section={dialog.section}
          onCheckForUpdates={openUpdates}
          onOpenUrl={(url) => void openUrl(url)}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.type === "update" && <UpdateDialog updates={updates} onClose={() => setDialog(null)} onOpenSettings={() => setDialog({ type: "settings", section: "updates" })} />}
      {dialog?.type === "closeBlocked" && (
        <ConfirmDialog
          title="Close Nexdir?"
          message={`${pluralize(dialog.running, "file operation")} ${dialog.running === 1 ? "is" : "are"} still running.`}
          detail={
            <p className="nx-muted">
              Closing cancels {dialog.running === 1 ? "it" : "them"}. The file being copied is finished or cleaned up safely, but the remaining items are not copied or moved. Existing files are
              never left half-written.
            </p>
          }
          confirmLabel="Cancel Operations and Close"
          cancelLabel="Keep Nexdir Open"
          focusCancel
          danger
          onClose={() => setDialog(null)}
          onConfirm={() => void api.confirmClose().catch((e) => notify("error", "Nexdir could not close.", errorMessage(e)))}
        />
      )}
      {updates.notice && updates.info && !dialog && (
        <UpdateNotice
          offer={updates.notice}
          currentVersion={updates.info.currentVersion}
          onViewChanges={() => void updates.openRelease()}
          onDownload={() => {
            updates.dismissNotice();
            setDialog({ type: "update" });
          }}
          onRemindLater={() => updates.snooze(updates.notice!.version)}
          onDismiss={updates.dismissNotice}
        />
      )}
      {dialog?.type === "welcome" && (
        <WelcomeDialog
          onStart={() => startTour()}
          onSkip={() => {
            recordTour("skipped");
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === "help" && <HelpCenter discordAvailable={DISCORD_INVITE !== null} onAction={helpAction} onClose={() => setDialog(null)} />}
      {dialog?.type === "issue" && (
        <IssueDialog
          kind={dialog.kind}
          loadSystemInfo={api.systemInfo}
          userNames={userNames}
          openUrl={(url) => api.openUrl(url)}
          copyText={(text) => navigator.clipboard.writeText(text)}
          onOpened={(viaClipboard) => {
            setDialog(null);
            notify(
              "success",
              "GitHub opened in your browser.",
              viaClipboard ? "The report is on the clipboard — paste it into the form, review it and submit." : "Review the prefilled form there and submit it if you want to publish it.",
            );
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {touring && (
        <Tour
          steps={TOUR_STEPS}
          onFinish={(completed) => {
            recordTour(completed ? "completed" : "skipped");
            setTouring(false);
          }}
        />
      )}
    </div>
  );
}
