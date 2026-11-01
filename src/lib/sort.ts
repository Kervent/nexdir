import type { FileEntry } from "../types/fs";
import { isNavigable, typeLabel } from "./fileTypes";

export type SortKey = "name" | "modified" | "type" | "size";
export type SortDirection = "asc" | "desc";

export interface SortSpec {
  key: SortKey;
  direction: SortDirection;
}

// Natural ordering like Windows Explorer: "file2" sorts before "file10".
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

interface Keyed {
  entry: FileEntry;
  folder: boolean;
  num: number;
  text: string;
}

/** Sorts entries with folders always first; ties fall back to the name. Sort keys are
 *  computed once per entry, not once per comparison (matters for 100k-entry folders). */
export function sortEntries(entries: readonly FileEntry[], spec: SortSpec): FileEntry[] {
  const factor = spec.direction === "asc" ? 1 : -1;
  const keyed: Keyed[] = entries.map((entry) => ({
    entry,
    folder: isNavigable(entry),
    num: spec.key === "size" ? (entry.size ?? -1) : spec.key === "modified" ? (entry.modified ?? 0) : 0,
    text: spec.key === "type" ? typeLabel(entry) : "",
  }));
  const textual = spec.key === "name" || spec.key === "type";
  keyed.sort((a, b) => {
    if (a.folder !== b.folder) return a.folder ? -1 : 1;
    let primary: number;
    if (spec.key === "name") primary = collator.compare(a.entry.name, b.entry.name);
    else if (textual) primary = collator.compare(a.text, b.text);
    else primary = a.num - b.num;
    primary *= factor;
    if (primary !== 0 || spec.key === "name") return primary;
    return collator.compare(a.entry.name, b.entry.name);
  });
  return keyed.map((k) => k.entry);
}

export function filterEntries(entries: readonly FileEntry[], query: string): FileEntry[] {
  const q = query.trim().toLocaleLowerCase();
  if (!q) return [...entries];
  return entries.filter((e) => e.name.toLocaleLowerCase().includes(q));
}
