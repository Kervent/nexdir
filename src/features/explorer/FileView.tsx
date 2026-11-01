import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp, Link2 } from "lucide-react";
import { FileIcon } from "../../components/FileIcon";
import { formatDate, formatSize } from "../../lib/format";
import { isNavigable, typeLabel } from "../../lib/fileTypes";
import { clickSelect, moveFocus, selectAll, type Selection } from "../../lib/selection";
import type { SortKey, SortSpec } from "../../lib/sort";
import type { FileEntry } from "../../types/fs";
import type { ViewMode } from "../settings/preferences";
import { canDrop, chooseEffect, draggedPaths, endDrag, startDrag, type DropEffect } from "../dnd/dnd";

interface FileViewProps {
  entries: FileEntry[];
  selection: Selection;
  onSelectionChange: (s: Selection) => void;
  onOpen: (entry: FileEntry) => void;
  onContextMenu: (e: React.MouseEvent, entry: FileEntry | null) => void;
  viewMode: ViewMode;
  sort: SortSpec;
  onSortChange: (s: SortSpec) => void;
  /** Rendered instead of the list when there are no entries. */
  empty: React.ReactNode;
  /** Folder shown by this view; dropping on empty space targets it. */
  folderPath: string | null;
  onDropPaths: (paths: string[], effect: DropEffect, target: string) => void;
  /** Middle click on a folder (opens it in a new tab). */
  onMiddleClick?: (entry: FileEntry) => void;
  /** Items currently cut to the clipboard, shown dimmed. */
  cutPaths?: ReadonlySet<string>;
  /** Adds a column with the containing folder (search results). */
  showLocation?: boolean;
}

const ROW_HEIGHT = 30;
const TILE_WIDTH = 112;
const TILE_HEIGHT = 108;

const COLUMNS: { key: SortKey; label: string; className: string; sortable?: boolean }[] = [
  { key: "name", label: "Name", className: "col-name" },
  { key: "modified", label: "Date modified", className: "col-date" },
  { key: "type", label: "Type", className: "col-type" },
  { key: "size", label: "Size", className: "col-size" },
];

const LOCATION_COLUMN = { key: "name" as SortKey, label: "Folder", className: "col-location", sortable: false };

export function useElementWidth(ref: React.RefObject<HTMLElement | null>) {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.clientWidth);
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setWidth(el.clientWidth));
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return width;
}

/** Virtualised file list in details or icon layout, with full keyboard support. */
export function FileView(props: FileViewProps) {
  const { entries, selection, onSelectionChange, onOpen, onContextMenu, viewMode, sort, onSortChange, empty, folderPath, onDropPaths, onMiddleClick, cutPaths, showLocation } = props;
  const idPrefix = useId();
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const width = useElementWidth(scrollRef);
  const typeahead = useRef({ text: "", at: 0 });

  const order = useMemo(() => entries.map((e) => e.path), [entries]);
  const columns = viewMode === "icons" ? Math.max(1, Math.floor((width - 16) / TILE_WIDTH)) : 1;
  const rowCount = Math.ceil(entries.length / columns);

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => (viewMode === "icons" ? TILE_HEIGHT : ROW_HEIGHT),
    overscan: 12,
    initialRect: { width: 800, height: 600 },
  });

  useEffect(() => virtualizer.measure(), [viewMode, virtualizer]);

  // Keep the focused item visible.
  useEffect(() => {
    if (!selection.focus) return;
    const index = order.indexOf(selection.focus);
    if (index >= 0) virtualizer.scrollToIndex(Math.floor(index / columns), { align: "auto" });
  }, [selection.focus, order, columns, virtualizer]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const mods = { shift: e.shiftKey, ctrl: e.ctrlKey };
    const page = Math.max(1, Math.floor((scrollRef.current?.clientHeight ?? 300) / (viewMode === "icons" ? TILE_HEIGHT : ROW_HEIGHT)) - 1) * columns;
    let next: Selection | null = null;
    switch (e.key) {
      case "ArrowDown":
        next = moveFocus(selection, order, columns, mods);
        break;
      case "ArrowUp":
        next = moveFocus(selection, order, -columns, mods);
        break;
      case "ArrowRight":
        if (viewMode === "icons") next = moveFocus(selection, order, 1, mods);
        break;
      case "ArrowLeft":
        if (viewMode === "icons") next = moveFocus(selection, order, -1, mods);
        break;
      case "PageDown":
        next = moveFocus(selection, order, page, mods);
        break;
      case "PageUp":
        next = moveFocus(selection, order, -page, mods);
        break;
      case "Home":
        next = moveFocus(selection, order, 0, { ...mods, absolute: 0 });
        break;
      case "End":
        next = moveFocus(selection, order, 0, { ...mods, absolute: order.length - 1 });
        break;
      case " ":
        if (e.ctrlKey && selection.focus) next = clickSelect(selection, order, selection.focus, { ctrl: true, shift: false });
        break;
      case "Enter": {
        if (e.altKey) return; // Alt+Enter = properties, handled by the app
        const target = entries.find((x) => x.path === selection.focus) ?? entries.find((x) => selection.selected.has(x.path));
        if (target) {
          e.preventDefault();
          onOpen(target);
        }
        return;
      }
      case "a":
      case "A":
        if (e.ctrlKey) next = selectAll(order);
        break;
      default:
        if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
          // Type-ahead: jump to the first item whose name starts with the typed text.
          const now = Date.now();
          const t = typeahead.current;
          t.text = now - t.at > 800 ? e.key.toLowerCase() : t.text + e.key.toLowerCase();
          t.at = now;
          const match = entries.find((x) => x.name.toLowerCase().startsWith(t.text));
          if (match) next = { selected: new Set([match.path]), focus: match.path, anchor: match.path };
        }
    }
    if (next) {
      e.preventDefault();
      onSelectionChange(next);
    }
  };

  const onItemMouseDown = (e: React.MouseEvent, entry: FileEntry) => {
    if (e.button === 2) {
      // Right-click keeps an existing multi-selection that includes the item.
      if (!selection.selected.has(entry.path)) onSelectionChange(clickSelect(selection, order, entry.path, { ctrl: false, shift: false }));
      return;
    }
    if (e.button === 1) {
      e.preventDefault();
      if (isNavigable(entry)) onMiddleClick?.(entry);
      return;
    }
    if (e.button !== 0) return;
    // Mousedown on an already selected item keeps the selection so it can be dragged;
    // the click handler narrows it afterwards if no drag happened.
    if (selection.selected.has(entry.path) && !e.ctrlKey && !e.shiftKey) return;
    onSelectionChange(clickSelect(selection, order, entry.path, { ctrl: e.ctrlKey, shift: e.shiftKey }));
  };

  const toggleSort = (key: SortKey) =>
    onSortChange({ key, direction: sort.key === key && sort.direction === "asc" ? "desc" : "asc" });

  const renderItem = (entry: FileEntry) => {
    const selected = selection.selected.has(entry.path);
    const focused = selection.focus === entry.path;
    const isLink = entry.kind === "junction" || entry.kind === "dirLink" || entry.kind === "fileLink";
    const common = {
      role: "option" as const,
      "aria-selected": selected,
      id: `${idPrefix}item-${order.indexOf(entry.path)}`,
      className: `${viewMode === "icons" ? "nx-tile" : "nx-row"}${selected ? " is-selected" : ""}${focused ? " is-focused" : ""}${entry.hidden ? " is-hidden" : ""}${cutPaths?.has(entry.path) ? " is-cut" : ""}${dropTarget === entry.path ? " is-drop-target" : ""}`,
      title: entry.name,
      onMouseDown: (e: React.MouseEvent) => onItemMouseDown(e, entry),
      onDoubleClick: () => onOpen(entry),
      onClick: (e: React.MouseEvent) => {
        // Plain click on an item of a multi-selection selects just that item (after a possible drag).
        if (e.button === 0 && !e.ctrlKey && !e.shiftKey && selection.selected.size > 1 && selection.selected.has(entry.path)) {
          onSelectionChange(clickSelect(selection, order, entry.path, { ctrl: false, shift: false }));
        }
      },
      draggable: true,
      "data-drop-path": isNavigable(entry) ? entry.path : undefined,
      onDragStart: (e: React.DragEvent) => {
        const paths = selection.selected.has(entry.path) ? order.filter((p) => selection.selected.has(p)) : [entry.path];
        if (!selection.selected.has(entry.path)) onSelectionChange(clickSelect(selection, order, entry.path, { ctrl: false, shift: false }));
        startDrag(e, paths);
      },
      onDragEnd: () => {
        endDrag();
        setDropTarget(null);
      },
      onContextMenu: (e: React.MouseEvent) => {
        e.stopPropagation();
        onContextMenu(e, entry);
      },
    };
    if (viewMode === "icons") {
      return (
        <div key={entry.path} {...common}>
          <span className="nx-tile-icon">
            <FileIcon entry={entry} size={40} />
            {isLink && <Link2 className="nx-link-badge" size={14} aria-label="Link" />}
          </span>
          <span className="nx-tile-name">{entry.name}</span>
        </div>
      );
    }
    return (
      <div key={entry.path} {...common}>
        <span className="col-name">
          <span className="nx-row-icon">
            <FileIcon entry={entry} size={18} />
            {isLink && <Link2 className="nx-link-badge" size={10} aria-label="Link" />}
          </span>
          <span className="nx-ellipsis">{entry.name}</span>
        </span>
        {showLocation && (
          <span className="col-location nx-ellipsis" title={entry.path}>
            {entry.path.slice(0, Math.max(entry.path.lastIndexOf("\\"), 2))}
          </span>
        )}
        <span className="col-date">{formatDate(entry.modified)}</span>
        <span className="col-type nx-ellipsis">{typeLabel(entry)}</span>
        <span className="col-size">{formatSize(entry.size)}</span>
      </div>
    );
  };

  const focusIndex = selection.focus ? order.indexOf(selection.focus) : -1;

  return (
    <div className={`nx-fileview is-${viewMode}${showLocation ? " has-location" : ""}`}>
      {viewMode === "details" && (
        <div className="nx-columns" role="row">
          {(showLocation ? [COLUMNS[0]!, LOCATION_COLUMN, ...COLUMNS.slice(1)] : COLUMNS).map((c) => (
            <button
              key={c.className}
              type="button"
              role="columnheader"
              className={`nx-column ${c.className}`}
              aria-sort={c.sortable !== false && sort.key === c.key ? (sort.direction === "asc" ? "ascending" : "descending") : "none"}
              onClick={() => c.sortable !== false && toggleSort(c.key)}
            >
              {c.label}
              {c.sortable !== false && sort.key === c.key && (sort.direction === "asc" ? <ArrowUp size={12} /> : <ArrowDown size={12} />)}
            </button>
          ))}
        </div>
      )}
      <div
        ref={scrollRef}
        className="nx-scroll"
        role="listbox"
        aria-label="Files and folders"
        aria-multiselectable="true"
        aria-activedescendant={focusIndex >= 0 ? `${idPrefix}item-${focusIndex}` : undefined}
        tabIndex={0}
        data-fileview=""
        onKeyDown={onKeyDown}
        onMouseDown={(e) => {
          if (e.target === e.currentTarget || (e.target as HTMLElement).classList.contains("nx-virtual")) {
            if (!e.ctrlKey && !e.shiftKey) onSelectionChange({ selected: new Set(), focus: null, anchor: null });
          }
        }}
        onContextMenu={(e) => onContextMenu(e, null)}
        onDragOver={(e) => {
          const paths = draggedPaths();
          const target = dropTargetOf(e.target) ?? folderPath;
          if (!paths || !target || !canDrop(paths, target)) {
            setDropTarget(null);
            return;
          }
          e.preventDefault();
          e.dataTransfer.dropEffect = chooseEffect(e, paths, target);
          setDropTarget(target);
        }}
        onDragLeave={(e) => {
          if (!(e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget))) setDropTarget(null);
        }}
        onDrop={(e) => {
          const paths = draggedPaths();
          const target = dropTargetOf(e.target) ?? folderPath;
          setDropTarget(null);
          if (!paths || !target || !canDrop(paths, target)) return;
          e.preventDefault();
          endDrag();
          onDropPaths(paths, chooseEffect(e, paths, target), target);
        }}
        data-drop-active={dropTarget !== null && dropTarget === folderPath ? "" : undefined}
      >
        {entries.length === 0 ? (
          empty
        ) : (
          <div className="nx-virtual" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((row) => (
              <div
                key={row.key}
                className={viewMode === "icons" ? "nx-tile-row" : "nx-row-slot"}
                style={{ transform: `translateY(${row.start}px)`, height: row.size }}
              >
                {entries.slice(row.index * columns, row.index * columns + columns).map(renderItem)}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function dropTargetOf(target: EventTarget | null): string | null {
  const el = target instanceof Element ? target.closest<HTMLElement>("[data-drop-path]") : null;
  return el?.dataset.dropPath ?? null;
}