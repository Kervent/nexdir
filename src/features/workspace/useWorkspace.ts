import { useCallback, useEffect, useMemo, useReducer, type Dispatch } from "react";
import { api, isApiError } from "../../services/api";
import { filterEntries, sortEntries, type SortSpec } from "../../lib/sort";
import {
  activePane,
  activeTab,
  createWorkspace,
  currentLocation,
  isValidSession,
  newId,
  parentOf,
  toSession,
  workspaceReducer,
  type Location,
  type PaneState,
  type Workspace,
  type WorkspaceAction,
} from "./workspace";

const SESSION_KEY = "nexdir.session.v1";

function loadSession(): Workspace {
  try {
    const raw = localStorage.getItem(SESSION_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : null;
    if (isValidSession(parsed)) return createWorkspace(parsed.tabs, parsed.activeIndex);
  } catch {
    // Unreadable session: start fresh.
  }
  return createWorkspace();
}

/** Tabs and panes of the window, restored from the previous session. */
export function useWorkspace() {
  const [ws, dispatch] = useReducer(workspaceReducer, undefined, loadSession);

  const session = useMemo(() => JSON.stringify(toSession(ws)), [ws]);
  useEffect(() => {
    try {
      localStorage.setItem(SESSION_KEY, session);
    } catch {
      // Storage unavailable: the session simply isn't restored next time.
    }
  }, [session]);

  const tab = activeTab(ws);
  const pane = activePane(ws);

  const actions = useMemo(
    () => ({
      navigate: (paneId: string, to: Location, select?: string) => dispatch({ type: "navigate", pane: paneId, to, select }),
      openTab: (to: Location, background = false, select?: string) =>
        dispatch({ type: "newTab", tabId: newId("t"), paneId: newId("p"), to, background, select }),
      closeTab: (tabId: string) => dispatch({ type: "closeTab", tab: tabId, replacement: { tabId: newId("t"), paneId: newId("p") } }),
      toggleSplit: () => dispatch({ type: "toggleSplit", paneId: newId("p") }),
    }),
    [],
  );

  const up = useCallback((p: PaneState) => {
    const parent = parentOf(p);
    if (parent) dispatch({ type: "navigate", pane: p.id, to: parent.to, select: parent.select });
  }, []);

  return { ws, dispatch, tab, pane, up, ...actions };
}

/** Loads the directory listing of a pane whenever its location or reload token changes. */
export function usePaneListing(pane: PaneState, showHidden: boolean, dispatch: Dispatch<WorkspaceAction>) {
  const location = currentLocation(pane);
  const path = location.type === "folder" ? location.path : null;
  useEffect(() => {
    if (!path) return;
    let stale = false;
    dispatch({ type: "loadStart", pane: pane.id });
    api
      .listDir(path, showHidden)
      .then((listing) => !stale && dispatch({ type: "loaded", pane: pane.id, listing }))
      .catch((err: unknown) => {
        if (stale) return;
        dispatch({
          type: "loadFailed",
          pane: pane.id,
          error: isApiError(err) ? err : { code: "io", message: String(err), path },
        });
      });
    return () => {
      stale = true;
    };
  }, [path, showHidden, pane.reloadToken, pane.id, dispatch]);
}

/** Sorted and filtered entries of a pane. */
export function useVisibleEntries(pane: PaneState, sort: SortSpec) {
  // Sort once per listing/sort order, then filter the sorted list (filtering keeps the order).
  // Typing in the filter box therefore never re-sorts the whole folder.
  const sorted = useMemo(() => (pane.listing ? sortEntries(pane.listing.entries, sort) : []), [pane.listing, sort]);
  return useMemo(() => filterEntries(sorted, pane.filter), [sorted, pane.filter]);
}
