import { describe, expect, it } from "vitest";
import {
  activePane,
  activeTab,
  createWorkspace,
  currentLocation,
  HOME,
  isValidSession,
  parentOf,
  parentPath,
  toSession,
  workspaceReducer as reduce,
  type Location,
  type Workspace,
} from "./workspace";
import type { DirListing } from "../../types/fs";

const folder = (path: string): Location => ({ type: "folder", path });

function listing(path: string, names: string[]): DirListing {
  return {
    path,
    parent: parentPath(path),
    entries: names.map((name) => ({
      name,
      path: `${path}\\${name}`,
      kind: "file",
      size: 1,
      modified: 0,
      created: 0,
      extension: null,
      hidden: false,
      system: false,
      readonly: false,
    })),
    unreadable: 0,
    hiddenCount: 0,
  };
}

describe("workspace reducer", () => {
  it("navigates with back/forward history", () => {
    let ws = createWorkspace();
    const pane = activePane(ws).id;
    ws = reduce(ws, { type: "navigate", pane, to: folder("C:\\A") });
    ws = reduce(ws, { type: "navigate", pane, to: folder("C:\\A\\B") });
    expect(currentLocation(ws.panes[pane]!)).toEqual(folder("C:\\A\\B"));
    ws = reduce(ws, { type: "back", pane });
    expect(currentLocation(ws.panes[pane]!)).toEqual(folder("C:\\A"));
    ws = reduce(ws, { type: "forward", pane });
    expect(currentLocation(ws.panes[pane]!)).toEqual(folder("C:\\A\\B"));
    // Navigating after going back drops the forward history.
    ws = reduce(ws, { type: "back", pane });
    ws = reduce(ws, { type: "navigate", pane, to: folder("C:\\Z") });
    ws = reduce(ws, { type: "forward", pane });
    expect(currentLocation(ws.panes[pane]!)).toEqual(folder("C:\\Z"));
  });

  it("re-navigating to the same folder reloads instead of adding history", () => {
    let ws = createWorkspace([{ locations: [folder("C:\\A")] }]);
    const pane = activePane(ws).id;
    ws = reduce(ws, { type: "navigate", pane, to: folder("c:\\a\\"), select: "x" });
    expect(ws.panes[pane]!.history.stack).toHaveLength(1);
    expect(ws.panes[pane]!.reloadToken).toBe(1);
    expect(ws.panes[pane]!.pendingSelect).toBe("x");
  });

  it("selects a pending name after loading", () => {
    let ws = createWorkspace();
    const pane = activePane(ws).id;
    ws = reduce(ws, { type: "navigate", pane, to: folder("C:\\A"), select: "B.TXT" });
    ws = reduce(ws, { type: "loaded", pane, listing: listing("C:\\A", ["a.txt", "b.txt"]) });
    expect([...ws.panes[pane]!.selection.selected]).toEqual(["C:\\A\\b.txt"]);
    expect(ws.panes[pane]!.pendingSelect).toBeNull();
  });

  it("opens, activates, moves and closes tabs", () => {
    let ws = createWorkspace();
    const first = ws.activeTab;
    ws = reduce(ws, { type: "newTab", tabId: "t2", paneId: "p2", to: folder("C:\\X") });
    expect(ws.activeTab).toBe("t2");
    ws = reduce(ws, { type: "newTab", tabId: "t3", paneId: "p3", to: HOME, background: true });
    expect(ws.activeTab).toBe("t2");
    expect(ws.tabs.map((t) => t.id)).toEqual([first, "t2", "t3"]);
    ws = reduce(ws, { type: "moveTab", tab: "t3", toIndex: 0 });
    expect(ws.tabs.map((t) => t.id)).toEqual(["t3", first, "t2"]);
    ws = reduce(ws, { type: "closeTab", tab: "t2", replacement: { tabId: "r", paneId: "rp" } });
    expect(ws.activeTab).toBe(first);
    expect(ws.panes.p2).toBeUndefined();
  });

  it("keeps one Home tab when the last tab closes", () => {
    let ws = createWorkspace([{ locations: [folder("C:\\A")] }]);
    ws = reduce(ws, { type: "closeTab", tab: ws.activeTab, replacement: { tabId: "r", paneId: "rp" } });
    expect(ws.tabs).toHaveLength(1);
    expect(ws.activeTab).toBe("r");
    expect(currentLocation(ws.panes.rp!)).toEqual(HOME);
    expect(Object.keys(ws.panes)).toEqual(["rp"]);
  });

  it("splits and unsplits the active tab", () => {
    let ws: Workspace = createWorkspace([{ locations: [folder("C:\\A")] }]);
    const original = activePane(ws).id;
    ws = reduce(ws, { type: "toggleSplit", paneId: "split" });
    expect(activeTab(ws).panes).toEqual([original, "split"]);
    expect(activePane(ws).id).toBe("split");
    expect(currentLocation(ws.panes.split!)).toEqual(folder("C:\\A"));
    ws = reduce(ws, { type: "focusPane", pane: original });
    expect(activePane(ws).id).toBe(original);
    ws = reduce(ws, { type: "toggleSplit", paneId: "unused" });
    expect(activeTab(ws).panes).toEqual([original]);
    expect(ws.panes.split).toBeUndefined();
  });

  it("computes parent locations", () => {
    expect(parentPath("C:\\A\\B")).toBe("C:\\A");
    expect(parentPath("C:\\A")).toBe("C:\\");
    expect(parentPath("C:\\")).toBeNull();
    expect(parentPath("\\\\srv\\share\\x")).toBe("\\\\srv\\share");
    expect(parentPath("\\\\srv\\share")).toBeNull();
    const ws = createWorkspace([{ locations: [folder("C:\\A\\B")] }]);
    expect(parentOf(activePane(ws))).toEqual({ to: folder("C:\\A"), select: "B" });
    const root = createWorkspace([{ locations: [folder("C:\\")] }]);
    expect(parentOf(activePane(root))).toEqual({ to: HOME });
  });

  it("round-trips sessions and rejects malformed ones", () => {
    let ws = createWorkspace([{ locations: [folder("C:\\A"), HOME] }, { locations: [folder("D:\\")] }], 1);
    const session = toSession(ws);
    expect(session).toEqual({ tabs: [{ locations: [folder("C:\\A"), HOME] }, { locations: [folder("D:\\")] }], activeIndex: 1 });
    expect(isValidSession(session)).toBe(true);
    ws = createWorkspace(session.tabs, session.activeIndex);
    expect(ws.tabs).toHaveLength(2);
    expect(isValidSession({ tabs: [] })).toBe(false);
    expect(isValidSession({ tabs: [{ locations: [{ type: "folder", path: 1 }] }], activeIndex: 0 })).toBe(false);
    expect(isValidSession(null)).toBe(false);
  });
});
