import { useMemo, useState } from "react";
import { Files, ShieldCheck, Trash2, TriangleAlert } from "lucide-react";
import { api } from "../../services/api";
import { formatDate, formatSize, pluralize } from "../../lib/format";
import type { DuplicateGroup, DuplicateRemoval, DuplicateReport } from "../../types/tools";
import { asEntry, FolderInput, TaskStatus, type ToolContext } from "./shared";
import { useTask } from "./useTask";

const MIN_SIZES: { label: string; bytes: number }[] = [
  { label: "Any size", bytes: 1 },
  { label: "≥ 100 KB", bytes: 100 * 1024 },
  { label: "≥ 1 MB", bytes: 1024 ** 2 },
  { label: "≥ 10 MB", bytes: 10 * 1024 ** 2 },
  { label: "≥ 100 MB", bytes: 100 * 1024 ** 2 },
];

const PAGE = 100;

/** Groups in which every copy is selected: deleting them would lose the content. */
export function fullySelectedGroups(groups: DuplicateGroup[], selected: ReadonlySet<string>): DuplicateGroup[] {
  return groups.filter((g) => g.files.every((f) => selected.has(f.path)));
}

/**
 * Turns the selection into removal requests: per group with a selection, the unselected
 * files are kept. The backend re-checks every group and refuses one without a kept copy.
 */
export function buildRemovals(groups: DuplicateGroup[], selected: ReadonlySet<string>): DuplicateRemoval[] {
  return groups
    .filter((g) => g.files.some((f) => selected.has(f.path)))
    .map((g) => ({
      size: g.size,
      hash: g.hash,
      keep: g.files.filter((f) => !selected.has(f.path)).map((f) => f.path),
      remove: g.files.filter((f) => selected.has(f.path)).map((f) => f.path),
    }));
}

/** Selects every file except one per group (the oldest or newest by modification date). */
export function selectAllButOne(groups: DuplicateGroup[], keep: "oldest" | "newest"): Set<string> {
  const out = new Set<string>();
  for (const g of groups) {
    const sorted = [...g.files].sort((a, b) => (a.modified ?? 0) - (b.modified ?? 0));
    const keeper = keep === "oldest" ? sorted[0] : sorted[sorted.length - 1];
    for (const f of g.files) if (f !== keeper) out.add(f.path);
  }
  return out;
}

export function DuplicatesView({ path, ctx }: { path?: string; ctx: ToolContext }) {
  const task = useTask<DuplicateReport>();
  const [root, setRoot] = useState(path ?? ctx.knownFolders.find((f) => f.id === "home")?.path ?? "");
  const [minSize, setMinSize] = useState(1024 ** 2);
  const [includeHidden, setIncludeHidden] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [shown, setShown] = useState(PAGE);

  const report = task.result;
  const groups = report?.groups ?? [];
  const selectedBytes = useMemo(() => groups.reduce((sum, g) => sum + g.size * g.files.filter((f) => selected.has(f.path)).length, 0), [groups, selected]);
  const unsafe = useMemo(() => fullySelectedGroups(groups, selected), [groups, selected]);

  const start = () => {
    if (!root.trim()) return ctx.notify("info", "Choose a folder to search for duplicates.");
    setSelected(new Set());
    setShown(PAGE);
    void task.start(() => api.startDuplicateScan([root.trim()], minSize, includeHidden));
  };

  const toggle = (p: string) =>
    setSelected((s) => {
      const next = new Set(s);
      if (next.has(p)) next.delete(p);
      else next.add(p);
      return next;
    });

  const recycleSelected = () => {
    if (unsafe.length > 0) return;
    ctx.requestRecycleDuplicates(buildRemovals(groups, selected), (recycled) => {
      const gone = new Set(recycled.map((p) => p.toLowerCase()));
      // Remove recycled files; groups with fewer than two files are no longer duplicates.
      const remaining = groups
        .map((g) => ({ ...g, files: g.files.filter((f) => !gone.has(f.path.toLowerCase())) }))
        .filter((g) => g.files.length > 1);
      task.setResult({ ...report!, groups: remaining, wastedBytes: remaining.reduce((s, g) => s + g.size * (g.files.length - 1), 0) });
      setSelected(new Set());
    });
  };

  return (
    <div className="nx-tool">
      <header className="nx-tool-header">
        <Files size={22} />
        <div className="nx-tool-title">
          <h1>Duplicate files</h1>
          <p className="nx-muted">Finds files with identical content (BLAKE3). Nothing is deleted unless you choose files and confirm.</p>
        </div>
      </header>

      <form
        className="nx-tool-form"
        onSubmit={(e) => {
          e.preventDefault();
          start();
        }}
      >
        <FolderInput label="Folder" value={root} onChange={setRoot} ctx={ctx} />
        <label className="nx-field">
          <span>Minimum size</span>
          <select className="nx-input" value={minSize} onChange={(e) => setMinSize(Number(e.target.value))}>
            {MIN_SIZES.map((m) => (
              <option key={m.bytes} value={m.bytes}>
                {m.label}
              </option>
            ))}
          </select>
        </label>
        <label className="nx-check">
          <input type="checkbox" checked={includeHidden} onChange={(e) => setIncludeHidden(e.target.checked)} /> Include hidden and system files
        </label>
        <button type="submit" className="nx-button is-primary" disabled={task.running}>
          Find duplicates
        </button>
      </form>

      <TaskStatus progress={task.progress} onCancel={task.cancel} />
      {task.error && <p className="nx-field-error">{task.error}</p>}

      {report && (
        <>
          {!report.complete && (
            <p className="nx-notice">
              <TriangleAlert size={14} /> The search was stopped early. The groups below are confirmed duplicates, but some may be missing.
            </p>
          )}
          <div className="nx-stats">
            <div>
              <span className="nx-stat-value">{groups.length.toLocaleString()}</span>
              <span className="nx-stat-label">groups of identical files</span>
            </div>
            <div>
              <span className="nx-stat-value">{formatSize(report.wastedBytes)}</span>
              <span className="nx-stat-label">could be freed</span>
            </div>
            <div>
              <span className="nx-stat-value">{report.filesScanned.toLocaleString()}</span>
              <span className="nx-stat-label">files checked</span>
            </div>
          </div>

          {groups.length > 0 && (
            <div className="nx-tool-actions">
              <button type="button" className="nx-button" onClick={() => setSelected(selectAllButOne(groups, "oldest"))}>
                Select all but the oldest
              </button>
              <button type="button" className="nx-button" onClick={() => setSelected(selectAllButOne(groups, "newest"))}>
                Select all but the newest
              </button>
              <button type="button" className="nx-button" disabled={selected.size === 0} onClick={() => setSelected(new Set())}>
                Clear selection
              </button>
              <span className="nx-spacer" />
              <button type="button" className="nx-button is-danger" disabled={selected.size === 0 || unsafe.length > 0} onClick={recycleSelected}>
                <Trash2 size={14} /> Move {pluralize(selected.size, "file")} to Recycle Bin ({formatSize(selectedBytes)})
              </button>
            </div>
          )}
          {unsafe.length > 0 && (
            <p className="nx-notice is-danger" role="alert">
              <ShieldCheck size={14} /> Every copy is selected in {pluralize(unsafe.length, "group")}. Keep at least one file per group.
            </p>
          )}
          {groups.length === 0 && report.complete && <p className="nx-muted">No duplicates found.</p>}

          <ul className="nx-dup-groups">
            {groups.slice(0, shown).map((g) => (
              <li key={g.hash} className="nx-dup-group">
                <div className="nx-dup-head">
                  <strong>{pluralize(g.files.length, "copy", "copies")}</strong>
                  <span className="nx-muted">
                    {formatSize(g.size)} each · {formatSize(g.size * (g.files.length - 1))} reclaimable
                  </span>
                </div>
                {g.files.map((f) => (
                  <label key={f.path} className="nx-dup-file" onContextMenu={(e) => ctx.itemsMenu(e, [asEntry(f.path, g.size, f.modified)], { recycle: false })}>
                    <input type="checkbox" checked={selected.has(f.path)} onChange={() => toggle(f.path)} aria-label={`Select ${f.path}`} />
                    <span className="nx-ellipsis" title={f.path}>
                      {f.path}
                    </span>
                    <span className="nx-muted">{formatDate(f.modified)}</span>
                    <button type="button" className="nx-link-button" onClick={() => ctx.reveal(f.path)}>
                      Show
                    </button>
                  </label>
                ))}
              </li>
            ))}
          </ul>
          {groups.length > shown && (
            <button type="button" className="nx-button" onClick={() => setShown((n) => n + PAGE)}>
              Show more ({groups.length - shown} remaining)
            </button>
          )}
        </>
      )}
    </div>
  );
}
