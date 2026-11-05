// Workspace state: tabs, each holding one or two explorer panes (split view).
// Pure reducer without side effects; directory loading is driven by usePaneLoader.
import { emptySelection, pruneSelection, type Selection } from "../../lib/selection";
import { baseName, samePath } from "../../lib/paths";
import type { ApiError, DirListing } from "../../types/fs";

export type ToolId = "search" | "storage" | "duplicates" | "organize";

/** Where a pane is: Home, a folder, one of the tools (optionally bound to a folder), or the
 *  Network overview (optionally showing the shares of one server). */
export type Location =
  | { type: "home" }
  | { type: "folder"; path: string }
  | { type: "tool"; tool: ToolId; path?: string }
  | { type: "network"; server?: string };

export const TOOL_IDS: ToolId[] = ["search", "storage", "duplicates", "organize"];

export const TOOL_LABELS: Record<ToolId, string> = {
  search: "Search",
  storage: "Storage analysis",
  duplicates: "Duplicate files",
  organize: "Organize",
};

export const HOME: Location = { type: "home" };
export const NETWORK: Location = { type: "network" };

export function sameLocation(a: Location, b: Location): boolean {
  if (a.type !== b.type) return false;
  if (a.type === "home") return true;
  if (a.type === "network") return (a.server ?? "").toLowerCase() === ((b as { server?: string }).server ?? "").toLowerCase();
  if (a.type === "folder") return samePath(a.path, (b as { path: string }).path);
  const t = b as { tool: ToolId; path?: string };
  return a.tool === t.tool && (a.path && t.path ? samePath(a.path, t.path) : a.path === t.path);
}

export function locationLabel(loc: Location): string {
  if (loc.type === "home") return "Home";
  if (loc.type === "network") return loc.server ?? "Network";
  if (loc.type === "tool") return loc.path ? `${TOOL_LABELS[loc.tool]}: ${baseName(loc.path)}` : TOOL_LABELS[loc.tool];
  return baseName(loc.path);
}

export interface PaneState {
  id: string;
  history: { stack: Location[]; index: number };
  listing: DirListing | null;
  error: ApiError | null;
  loading: boolean;
  selection: Selection;
  filter: string;
  /** Incremented to force a reload of the current location. */
  reloadToken: number;
  /** Name to select once the next listing arrives. */
  pendingSelect: string | null;
}

export interface Tab {
  id: string;
  /** One pane, or two in split view. */
  panes: string[];
  activePane: string;
}

export interface Workspace {
  tabs: Tab[];
  activeTab: string;
  panes: Record<string, PaneState>;
}

export type WorkspaceAction =
  | { type: "navigate"; pane: string; to: Location; select?: string }
  | { type: "back"; pane: string }
  | { type: "forward"; pane: string }
  | { type: "refresh"; pane: string; select?: string }
  | { type: "loadStart"; pane: string }
  | { type: "loaded"; pane: string; listing: DirListing }
  | { type: "loadFailed"; pane: string; error: ApiError }
  | { type: "select"; pane: string; selection: Selection }
  | { type: "filter"; pane: string; value: string }
  | { type: "newTab"; tabId: string; paneId: string; to: Location; select?: string; background?: boolean }
  | { type: "closeTab"; tab: string; replacement: { tabId: string; paneId: string } }
  | { type: "activateTab"; tab: string }
  | { type: "moveTab"; tab: string; toIndex: number }
  | { type: "toggleSplit"; paneId: string }
  | { type: "focusPane"; pane: string };

let idCounter = 0;
/** Unique ids for tabs and panes. Generated outside the reducer to keep it pure. */
export function newId(prefix: string): string {
  idCounter += 1;
  return `${prefix}${Date.now().toString(36)}${idCounter}`;
}

export function createPane(id: string, at: Location, select?: string): PaneState {
  return {
    id,
    history: { stack: [at], index: 0 },
    listing: null,
    error: null,
    loading: false,
    selection: emptySelection,
    filter: "",
    reloadToken: 0,
    pendingSelect: select ?? null,
  };
}

export function createWorkspace(tabs: { locations: Location[] }[] = [{ locations: [HOME] }], activeIndex = 0): Workspace {
  const panes: Record<string, PaneState> = {};
  const tabList: Tab[] = (tabs.length ? tabs : [{ locations: [HOME] }]).map((t) => {
    const ids = t.locations.slice(0, 2).map((loc) => {
      const pane = createPane(newId("p"), loc);
      panes[pane.id] = pane;
      return pane.id;
    });
    if (ids.length === 0) {
      const pane = createPane(newId("p"), HOME);
      panes[pane.id] = pane;
      ids.push(pane.id);
    }
    return { id: newId("t"), panes: ids, activePane: ids[0]! };
  });
  return { tabs: tabList, activeTab: tabList[Math.min(Math.max(activeIndex, 0), tabList.length - 1)]!.id, panes };
}

export function currentLocation(pane: PaneState): Location {
  return pane.history.stack[pane.history.index]!;
}

export function activeTab(ws: Workspace): Tab {
  return ws.tabs.find((t) => t.id === ws.activeTab) ?? ws.tabs[0]!;
}

export function activePane(ws: Workspace): PaneState {
  return ws.panes[activeTab(ws).activePane]!;
}

/** Location to go to for "Up", plus the child name to select there. */
export function parentOf(pane: PaneState): { to: Location; select?: string } | null {
  const loc = currentLocation(pane);
  if (loc.type !== "folder") return null;
  const parent = pane.listing && samePath(pane.listing.path, loc.path) ? pane.listing.parent : parentPath(loc.path);
  const child = baseName(loc.path);
  if (parent) return { to: { type: "folder", path: parent }, select: child };
  // Up from a network share goes to the shares of its server in the Network view.
  const unc = /^\\\\([^\\]+)\\[^\\]+\\?$/.exec(loc.path);
  return unc ? { to: { type: "network", server: unc[1] } } : { to: HOME };
}

/** Lexical parent of a Windows path; null for drive and share roots. */
export function parentPath(path: string): string | null {
  const trimmed = path.replace(/[\\/]+$/, "");
  if (/^[A-Za-z]:$/.test(trimmed) || /^\\\\[^\\]+\\[^\\]+$/.test(trimmed)) return null;
  const idx = trimmed.lastIndexOf("\\");
  if (idx < 0) return null;
  const parent = trimmed.slice(0, idx);
  return /^[A-Za-z]:$/.test(parent) ? `${parent}\\` : parent;
}

function updatePane(ws: Workspace, id: string, fn: (p: PaneState) => PaneState): Workspace {
  const pane = ws.panes[id];
  if (!pane) return ws;
  return { ...ws, panes: { ...ws.panes, [id]: fn(pane) } };
}

function moveHistory(p: PaneState, delta: number): PaneState {
  const index = p.history.index + delta;
  if (index < 0 || index >= p.history.stack.length) return p;
  return { ...p, history: { ...p.history, index }, selection: emptySelection, filter: "", listing: null, error: null, pendingSelect: null };
}

export function workspaceReducer(ws: Workspace, action: WorkspaceAction): Workspace {
  switch (action.type) {
    case "navigate":
      return updatePane(ws, action.pane, (p) => {
        if (sameLocation(currentLocation(p), action.to)) {
          return { ...p, reloadToken: p.reloadToken + 1, pendingSelect: action.select ?? null, filter: "" };
        }
        const stack = [...p.history.stack.slice(0, p.history.index + 1), action.to];
        return {
          ...p,
          history: { stack, index: stack.length - 1 },
          selection: emptySelection,
          filter: "",
          listing: null,
          error: null,
          pendingSelect: action.select ?? null,
        };
      });
    case "back":
      return updatePane(ws, action.pane, (p) => moveHistory(p, -1));
    case "forward":
      return updatePane(ws, action.pane, (p) => moveHistory(p, 1));
    case "refresh":
      return updatePane(ws, action.pane, (p) => ({ ...p, reloadToken: p.reloadToken + 1, pendingSelect: action.select ?? p.pendingSelect }));
    case "loadStart":
      return updatePane(ws, action.pane, (p) => ({ ...p, loading: true }));
    case "loaded":
      return updatePane(ws, action.pane, (p) => {
        const want = p.pendingSelect?.toLowerCase();
        const match = want ? action.listing.entries.find((e) => e.name.toLowerCase() === want) : undefined;
        const selection = match
          ? { selected: new Set([match.path]), focus: match.path, anchor: match.path }
          : pruneSelection(p.selection, action.listing.entries.map((e) => e.path));
        return { ...p, listing: action.listing, error: null, loading: false, selection, pendingSelect: null };
      });
    case "loadFailed":
      return updatePane(ws, action.pane, (p) => ({ ...p, listing: null, error: action.error, loading: false, pendingSelect: null }));
    case "select":
      return updatePane(ws, action.pane, (p) => ({ ...p, selection: action.selection }));
    case "filter":
      return updatePane(ws, action.pane, (p) => ({ ...p, filter: action.value }));
    case "newTab": {
      const pane = createPane(action.paneId, action.to, action.select);
      const current = ws.tabs.findIndex((t) => t.id === ws.activeTab);
      const tabs = [...ws.tabs];
      tabs.splice(current + 1, 0, { id: action.tabId, panes: [pane.id], activePane: pane.id });
      return { tabs, activeTab: action.background ? ws.activeTab : action.tabId, panes: { ...ws.panes, [pane.id]: pane } };
    }
    case "closeTab": {
      const index = ws.tabs.findIndex((t) => t.id === action.tab);
      if (index < 0) return ws;
      const closing = ws.tabs[index]!;
      const panes = { ...ws.panes };
      for (const id of closing.panes) delete panes[id];
      let tabs = ws.tabs.filter((t) => t.id !== action.tab);
      if (tabs.length === 0) {
        // The window always keeps one tab; closing the last one starts over at Home.
        const pane = createPane(action.replacement.paneId, HOME);
        panes[pane.id] = pane;
        tabs = [{ id: action.replacement.tabId, panes: [pane.id], activePane: pane.id }];
      }
      const activeTabId = ws.activeTab === action.tab ? tabs[Math.min(index, tabs.length - 1)]!.id : ws.activeTab;
      return { tabs, activeTab: activeTabId, panes };
    }
    case "activateTab":
      return ws.tabs.some((t) => t.id === action.tab) ? { ...ws, activeTab: action.tab } : ws;
    case "moveTab": {
      const from = ws.tabs.findIndex((t) => t.id === action.tab);
      if (from < 0) return ws;
      const tabs = [...ws.tabs];
      const [tab] = tabs.splice(from, 1);
      tabs.splice(Math.max(0, Math.min(action.toIndex, tabs.length)), 0, tab!);
      return { ...ws, tabs };
    }
    case "toggleSplit": {
      const tab = activeTab(ws);
      if (tab.panes.length === 2) {
        // Closing split view keeps the focused pane.
        const keep = tab.activePane;
        const drop = tab.panes.find((id) => id !== keep)!;
        const panes = { ...ws.panes };
        delete panes[drop];
        return { ...ws, panes, tabs: ws.tabs.map((t) => (t.id === tab.id ? { ...t, panes: [keep], activePane: keep } : t)) };
      }
      // The new pane starts where the current one is, like duplicating it.
      const source = ws.panes[tab.activePane]!;
      const pane = createPane(action.paneId, currentLocation(source));
      return {
        ...ws,
        panes: { ...ws.panes, [pane.id]: pane },
        tabs: ws.tabs.map((t) => (t.id === tab.id ? { ...t, panes: [...t.panes, pane.id], activePane: pane.id } : t)),
      };
    }
    case "focusPane": {
      const tab = ws.tabs.find((t) => t.panes.includes(action.pane));
      if (!tab || (tab.activePane === action.pane && ws.activeTab === tab.id)) return ws;
      return { ...ws, activeTab: tab.id, tabs: ws.tabs.map((t) => (t.id === tab.id ? { ...t, activePane: action.pane } : t)) };
    }
  }
}

/** Serializable snapshot of open tabs (locations only) for restoring the session. */
export interface SavedSession {
  tabs: { locations: Location[] }[];
  activeIndex: number;
}

export function toSession(ws: Workspace): SavedSession {
  return {
    tabs: ws.tabs.map((t) => ({ locations: t.panes.map((id) => currentLocation(ws.panes[id]!)) })),
    activeIndex: Math.max(0, ws.tabs.findIndex((t) => t.id === ws.activeTab)),
  };
}

export function isValidSession(value: unknown): value is SavedSession {
  if (typeof value !== "object" || value === null) return false;
  const s = value as SavedSession;
  return (
    Array.isArray(s.tabs) &&
    s.tabs.length > 0 &&
    s.tabs.length <= 50 &&
    s.tabs.every(
      (t) =>
        Array.isArray(t.locations) &&
        t.locations.length >= 1 &&
        t.locations.length <= 2 &&
        t.locations.every(
          (l) =>
            l?.type === "home" ||
            (l?.type === "network" && (l.server === undefined || typeof l.server === "string")) ||
            (l?.type === "folder" && typeof l.path === "string") ||
            (l?.type === "tool" && TOOL_IDS.includes(l.tool) && (l.path === undefined || typeof l.path === "string")),
        ),
    ) &&
    typeof s.activeIndex === "number"
  );
}

export type { Selection };
