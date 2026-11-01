import { forwardRef, useImperativeHandle, useRef, type Dispatch } from "react";
import { ArrowLeft, ArrowRight, ArrowUp, FolderX, RefreshCw, Search, TriangleAlert } from "lucide-react";
import { FileView } from "./FileView";
import { HomeView } from "./HomeView";
import { AddressBar, type AddressBarHandle } from "../navigation/AddressBar";
import { currentLocation, HOME, locationLabel, type Location, type PaneState, type WorkspaceAction } from "../workspace/workspace";
import { usePaneListing, useVisibleEntries } from "../workspace/useWorkspace";
import type { Preferences } from "../settings/preferences";
import type { DropEffect } from "../dnd/dnd";
import { baseName } from "../../lib/paths";
import { pluralize } from "../../lib/format";
import type { DriveInfo, FileEntry, KnownFolder } from "../../types/fs";
import type { ToolContext } from "../tools/shared";
import { StorageView } from "../tools/StorageView";
import { DuplicatesView } from "../tools/DuplicatesView";
import { SearchView } from "../tools/SearchView";
import { OrganizeView } from "../tools/OrganizeView";

export interface ExplorerPaneHandle {
  editAddress: () => void;
  focusFilter: () => void;
  focusList: () => void;
}

interface ExplorerPaneProps {
  pane: PaneState;
  active: boolean;
  split: boolean;
  prefs: Preferences;
  updatePrefs: (patch: Partial<Preferences>) => void;
  dispatch: Dispatch<WorkspaceAction>;
  drives: DriveInfo[];
  knownFolders: KnownFolder[];
  cutPaths: ReadonlySet<string>;
  onUp: (pane: PaneState) => void;
  onOpenEntry: (pane: PaneState, entry: FileEntry) => void;
  onContextMenu: (pane: PaneState, e: React.MouseEvent, entry: FileEntry | null) => void;
  onDropPaths: (paths: string[], effect: DropEffect, target: string) => void;
  onOpenInNewTab: (to: Location) => void;
  onSubmitAddress: (pane: PaneState, text: string) => Promise<string | null>;
  onRefreshHome: () => void;
  toolContext: (pane: PaneState) => ToolContext;
}

/** One explorer view: its own navigation row, address bar, filter and content. */
export const ExplorerPane = forwardRef<ExplorerPaneHandle, ExplorerPaneProps>(function ExplorerPane(props, ref) {
  const { pane, active, split, prefs, updatePrefs, dispatch, drives, knownFolders, cutPaths } = props;
  const location = currentLocation(pane);
  const folder = location.type === "folder" ? location.path : null;
  const addressRef = useRef<AddressBarHandle>(null);
  const filterRef = useRef<HTMLInputElement>(null);
  const rootRef = useRef<HTMLElement>(null);

  usePaneListing(pane, prefs.showHidden, dispatch);
  const entries = useVisibleEntries(pane, prefs.sort);

  useImperativeHandle(ref, () => ({
    editAddress: () => addressRef.current?.edit(),
    focusFilter: () => filterRef.current?.focus(),
    focusList: () => rootRef.current?.querySelector<HTMLElement>("[data-fileview]")?.focus(),
  }));

  const navigate = (to: Location, select?: string) => dispatch({ type: "navigate", pane: pane.id, to, select });
  const canBack = pane.history.index > 0;
  const canForward = pane.history.index < pane.history.stack.length - 1;

  const renderContent = () => {
    if (location.type === "home") {
      return <HomeView knownFolders={knownFolders} drives={drives} pinned={prefs.pinned} onNavigate={(to) => navigate(to)} />;
    }
    if (location.type === "tool") {
      const ctx = props.toolContext(pane);
      const key = `${location.tool}:${location.path ?? ""}`;
      switch (location.tool) {
        case "storage":
          return <StorageView key={key} path={location.path} ctx={ctx} />;
        case "duplicates":
          return <DuplicatesView key={key} path={location.path} ctx={ctx} />;
        case "search":
          return <SearchView key={key} path={location.path} ctx={ctx} />;
        case "organize":
          return <OrganizeView key={key} ctx={ctx} />;
      }
    }
    if (pane.error) {
      const denied = pane.error.code === "permissionDenied";
      return (
        <div className="nx-state" role="alert">
          {denied ? <TriangleAlert size={40} /> : <FolderX size={40} />}
          <h2>{denied ? "Access denied" : "This location can't be opened"}</h2>
          <p>{pane.error.message}</p>
          <p className="nx-state-path">{location.path}</p>
          <div className="nx-state-actions">
            <button type="button" className="nx-button" onClick={() => dispatch({ type: "refresh", pane: pane.id })}>
              Try again
            </button>
            <button type="button" className="nx-button is-primary" onClick={() => navigate(HOME)}>
              Go to Home
            </button>
          </div>
        </div>
      );
    }
    return (
      <FileView
        entries={entries}
        selection={pane.selection}
        onSelectionChange={(selection) => dispatch({ type: "select", pane: pane.id, selection })}
        onOpen={(entry) => props.onOpenEntry(pane, entry)}
        onContextMenu={(e, entry) => props.onContextMenu(pane, e, entry)}
        viewMode={prefs.viewMode}
        sort={prefs.sort}
        onSortChange={(sort) => updatePrefs({ sort })}
        folderPath={folder}
        onDropPaths={props.onDropPaths}
        onMiddleClick={(entry) => props.onOpenInNewTab({ type: "folder", path: entry.path })}
        cutPaths={cutPaths}
        empty={
          pane.loading || !pane.listing ? null : (
            <div className="nx-state is-empty">
              <p>{pane.filter ? `No items match "${pane.filter}".` : "This folder is empty."}</p>
              {!pane.filter && pane.listing.hiddenCount > 0 && (
                <button type="button" className="nx-link-button" onClick={() => updatePrefs({ showHidden: true })}>
                  Show {pluralize(pane.listing.hiddenCount, "hidden item")}
                </button>
              )}
            </div>
          )
        }
      />
    );
  };

  return (
    <section
      ref={rootRef}
      className={`nx-pane${active ? " is-active" : ""}${split ? " is-split" : ""}`}
      aria-label={split ? `${active ? "Active pane" : "Pane"}: ${locationLabel(location)}` : undefined}
      onMouseDownCapture={() => !active && dispatch({ type: "focusPane", pane: pane.id })}
      onFocusCapture={() => !active && dispatch({ type: "focusPane", pane: pane.id })}
    >
      <div className="nx-pane-nav" data-tour={active ? "address" : undefined}>
        <div className="nx-nav-buttons">
          <button type="button" className="nx-icon-button" aria-label="Back (Alt+Left)" title="Back (Alt+Left)" disabled={!canBack} onClick={() => dispatch({ type: "back", pane: pane.id })}>
            <ArrowLeft size={17} />
          </button>
          <button type="button" className="nx-icon-button" aria-label="Forward (Alt+Right)" title="Forward (Alt+Right)" disabled={!canForward} onClick={() => dispatch({ type: "forward", pane: pane.id })}>
            <ArrowRight size={17} />
          </button>
          <button type="button" className="nx-icon-button" aria-label="Up (Alt+Up)" title="Up (Alt+Up)" disabled={!folder} onClick={() => props.onUp(pane)}>
            <ArrowUp size={17} />
          </button>
          <button
            type="button"
            className="nx-icon-button"
            aria-label="Refresh (F5)"
            title="Refresh (F5)"
            onClick={() => (folder ? dispatch({ type: "refresh", pane: pane.id }) : props.onRefreshHome())}
          >
            <RefreshCw size={16} className={pane.loading ? "nx-spin" : undefined} />
          </button>
        </div>
        <AddressBar
          ref={addressRef}
          location={location}
          onNavigate={(to) => navigate(to)}
          onSubmit={(text) => props.onSubmitAddress(pane, text)}
          onDropPaths={props.onDropPaths}
        />
        <label className="nx-search">
          <Search size={15} aria-hidden="true" />
          <input
            ref={filterRef}
            type="search"
            placeholder={folder ? `Filter ${baseName(folder)}` : "Filter"}
            aria-label="Filter items in this folder"
            value={pane.filter}
            disabled={!folder}
            onChange={(e) => dispatch({ type: "filter", pane: pane.id, value: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === "Escape") dispatch({ type: "filter", pane: pane.id, value: "" });
              if (e.key === "ArrowDown" || e.key === "Enter") rootRef.current?.querySelector<HTMLElement>("[data-fileview]")?.focus();
            }}
          />
        </label>
      </div>
      <div className="nx-pane-content" aria-busy={pane.loading}>
        {renderContent()}
      </div>
    </section>
  );
});
