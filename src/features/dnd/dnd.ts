// Drag and drop of items inside Nexdir (between folders, panes, tabs' sidebar and
// breadcrumbs). Uses the HTML5 drag events of the WebView.
import { useState } from "react";
import { samePath } from "../../lib/paths";
import { parentPath } from "../workspace/workspace";

export type DropEffect = "copy" | "move";

const DRAG_MIME = "application/x-nexdir-paths";

// dataTransfer contents are not readable during dragover, so the active drag is kept here.
let activeDrag: string[] | null = null;

export function startDrag(e: React.DragEvent, paths: string[]) {
  activeDrag = paths;
  e.dataTransfer.setData(DRAG_MIME, JSON.stringify(paths));
  e.dataTransfer.setData("text/plain", paths.join("\r\n"));
  e.dataTransfer.effectAllowed = "copyMove";
}

export function endDrag() {
  activeDrag = null;
}

export function draggedPaths(): string[] | null {
  return activeDrag;
}

/** Volume identity: drive letter or UNC share. */
export function volumeOf(path: string): string {
  const drive = /^([A-Za-z]):/.exec(path);
  if (drive) return drive[1]!.toUpperCase();
  const unc = /^\\\\([^\\]+)\\([^\\]+)/.exec(path);
  return unc ? `\\\\${unc[1]}\\${unc[2]}`.toLowerCase() : path.toLowerCase();
}

/** Explorer semantics: Ctrl copies, Shift moves, otherwise move on the same volume and copy across volumes. */
export function chooseEffect(mods: { ctrlKey: boolean; shiftKey: boolean }, sources: string[], target: string): DropEffect {
  if (mods.ctrlKey) return "copy";
  if (mods.shiftKey) return "move";
  return sources.every((s) => volumeOf(s) === volumeOf(target)) ? "move" : "copy";
}

function isSameOrInside(inner: string, outer: string): boolean {
  if (samePath(inner, outer)) return true;
  const o = outer.replace(/[\\/]+$/, "").toLowerCase() + "\\";
  return inner.toLowerCase().startsWith(o);
}

/** A drop is pointless or dangerous if the target is a dragged item, lies inside one,
 *  or already contains every dragged item. */
export function canDrop(sources: string[], target: string): boolean {
  if (sources.length === 0) return false;
  if (sources.some((s) => isSameOrInside(target, s))) return false;
  return !sources.every((s) => {
    const parent = parentPath(s);
    return parent !== null && samePath(parent, target);
  });
}

/** Drop-target behaviour for a single element (sidebar item, breadcrumb, …). */
export function useDropTarget(target: string | null, onDrop: (paths: string[], effect: DropEffect, target: string) => void) {
  const [isOver, setOver] = useState(false);
  if (!target) return { dropProps: {}, isOver: false };
  const dropProps = {
    onDragOver: (e: React.DragEvent) => {
      const paths = draggedPaths();
      if (!paths || !canDrop(paths, target)) return;
      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = chooseEffect(e, paths, target);
      if (!isOver) setOver(true);
    },
    onDragLeave: (e: React.DragEvent) => {
      if (e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget)) return;
      setOver(false);
    },
    onDrop: (e: React.DragEvent) => {
      setOver(false);
      const paths = draggedPaths();
      if (!paths || !canDrop(paths, target)) return;
      e.preventDefault();
      e.stopPropagation();
      endDrag();
      onDrop(paths, chooseEffect(e, paths, target), target);
    },
  };
  return { dropProps, isOver };
}
