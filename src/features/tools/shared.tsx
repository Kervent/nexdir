import { useId } from "react";
import type { DriveInfo, FileEntry, KnownFolder } from "../../types/fs";
import type { DuplicateRemoval, TaskProgress } from "../../types/tools";
import type { Location } from "../workspace/workspace";
import type { PinnedFolder, Preferences } from "../settings/preferences";
import type { ToastTone } from "../../components/Toasts";
import { formatSize, percent } from "../../lib/format";

/** Everything a tool view may do outside itself. Provided by the app. */
export interface ToolContext {
  navigate: (to: Location) => void;
  openInNewTab: (to: Location) => void;
  openEntry: (entry: FileEntry) => void;
  /** Opens the folder containing `path` and selects the item. */
  reveal: (path: string) => void;
  /** Asks the user to confirm, then moves the items to the Recycle Bin. */
  requestRecycle: (paths: string[], onDone: (recycled: string[]) => void) => void;
  /** Confirms, then removes duplicate copies through the backend guard that keeps one copy per group. */
  requestRecycleDuplicates: (groups: DuplicateRemoval[], onDone: (recycled: string[]) => void) => void;
  /** `recycle: false` leaves out "Move to Recycle Bin" (duplicates must go through their own guarded command). */
  itemsMenu: (e: React.MouseEvent, items: FileEntry[], options?: { recycle?: boolean }) => void;
  notify: (tone: ToastTone, message: string, detail?: string) => void;
  knownFolders: KnownFolder[];
  drives: DriveInfo[];
  pinned: PinnedFolder[];
  prefs: Preferences;
  updatePrefs: (patch: Partial<Preferences>) => void;
}

/** Text field for a folder path with suggestions from known folders, pins and drives. */
export function FolderInput({ value, onChange, label, ctx, placeholder }: {
  value: string;
  onChange: (v: string) => void;
  label: string;
  ctx: ToolContext;
  placeholder?: string;
}) {
  const listId = useId();
  const suggestions = [...ctx.knownFolders.map((f) => f.path), ...ctx.pinned.map((p) => p.path), ...ctx.drives.filter((d) => d.available).map((d) => d.path)];
  return (
    <label className="nx-field">
      <span>{label}</span>
      <input className="nx-input" list={listId} value={value} spellCheck={false} placeholder={placeholder ?? "C:\\Users\\…"} onChange={(e) => onChange(e.target.value)} />
      <datalist id={listId}>
        {[...new Set(suggestions)].map((s) => (
          <option key={s} value={s} />
        ))}
      </datalist>
    </label>
  );
}

export function TaskStatus({ progress, onCancel, unit = "files" }: { progress: TaskProgress | null; onCancel: () => void; unit?: string }) {
  if (!progress || progress.state !== "running") return null;
  const determinate = progress.total != null && progress.total > 0;
  const pct = determinate ? percent(progress.done, progress.total!) : null;
  return (
    <div className="nx-task" role="status">
      <div className="nx-task-row">
        <strong>{progress.phase}…</strong>
        <span className="nx-muted">
          {progress.items.toLocaleString()} {unit}
          {progress.bytes > 0 ? ` · ${formatSize(progress.bytes)}` : ""}
          {pct != null ? ` · ${pct}%` : ""}
        </span>
        <span className="nx-spacer" />
        <button type="button" className="nx-button is-small" onClick={onCancel}>
          Stop
        </button>
      </div>
      <div className={`nx-progress${determinate ? "" : " is-indeterminate"}`} role="progressbar" aria-valuenow={pct ?? undefined} aria-valuemin={0} aria-valuemax={100}>
        <span style={{ width: determinate ? `${pct}%` : "30%" }} />
      </div>
      {progress.current && <div className="nx-task-current nx-ellipsis">{progress.current}</div>}
    </div>
  );
}

/** Resolves a known folder id or path typed by the user to an absolute path. */
export function resolveScope(scope: string | null, ctx: ToolContext): string | null {
  if (!scope) return null;
  const known = ctx.knownFolders.find((f) => f.id === scope);
  return known ? known.path : scope;
}

export function asEntry(path: string, size: number | null = null, modified: number | null = null): FileEntry {
  const name = path.split("\\").pop() ?? path;
  const dot = name.lastIndexOf(".");
  return {
    name,
    path,
    kind: "file",
    size,
    modified,
    created: null,
    extension: dot > 0 ? name.slice(dot + 1).toLowerCase() : null,
    hidden: false,
    system: false,
    readonly: false,
  };
}
