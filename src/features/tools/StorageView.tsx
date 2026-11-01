import { useEffect, useMemo, useRef, useState } from "react";
import { ChartPie, FolderOpen, HardDrive, RefreshCw, TriangleAlert } from "lucide-react";
import { api } from "../../services/api";
import { formatDate, formatSize, percent, pluralize } from "../../lib/format";
import { useElementWidth } from "../explorer/FileView";
import type { StorageReport, SizeNode } from "../../types/tools";
import { squarify } from "./treemap";
import { asEntry, TaskStatus, type ToolContext } from "./shared";
import { useTask } from "./useTask";

const CATEGORY_LABELS: Record<string, string> = {
  image: "Images",
  video: "Videos",
  audio: "Music & audio",
  document: "Documents",
  archive: "Archives",
  program: "Programs",
  code: "Source code",
  other: "Other",
};

const TILE_COLORS = ["var(--nx-icon-doc)", "var(--nx-icon-image)", "var(--nx-icon-video)", "var(--nx-icon-audio)", "var(--nx-icon-code)", "var(--nx-icon-archive)", "var(--nx-secondary)"];

function Treemap({ nodes, total, onOpen }: { nodes: SizeNode[]; total: number; onOpen: (n: SizeNode) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const width = useElementWidth(ref);
  const height = 260;
  const shown = nodes.filter((n) => n.bytes > 0).slice(0, 60);
  const rects = useMemo(() => squarify(shown.map((n) => n.bytes), { x: 0, y: 0, w: Math.max(width, 1), h: height }), [shown, width]);
  return (
    <div ref={ref} className="nx-treemap" style={{ height }} role="list" aria-label="Space used by items in this folder">
      {shown.map((n, i) => {
        const r = rects[i]!;
        const big = r.w > 70 && r.h > 34;
        return (
          <button
            key={n.path}
            type="button"
            role="listitem"
            className={`nx-tile-map${n.isDir ? " is-dir" : ""}`}
            style={{ left: r.x, top: r.y, width: r.w, height: r.h, background: TILE_COLORS[i % TILE_COLORS.length] }}
            title={`${n.name} — ${formatSize(n.bytes)} (${percent(n.bytes, total)}%)${n.isDir ? " · click to analyze" : ""}`}
            onClick={() => onOpen(n)}
          >
            {big && (
              <span className="nx-tile-map-label">
                <strong className="nx-ellipsis">{n.name}</strong>
                <span>{formatSize(n.bytes)}</span>
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}

function Bars({ rows, total, onOpen }: { rows: { key: string; label: string; bytes: number; sub?: string; path?: string }[]; total: number; onOpen?: (path: string) => void }) {
  return (
    <ul className="nx-bars">
      {rows.map((r) => (
        <li key={r.key}>
          <button type="button" className="nx-bar-row" disabled={!r.path || !onOpen} onClick={() => r.path && onOpen?.(r.path)} title={r.path ?? r.label}>
            <span className="nx-bar-label nx-ellipsis">{r.label}</span>
            <span className="nx-bar-track">
              <span style={{ width: `${Math.max(1, percent(r.bytes, total))}%` }} />
            </span>
            <span className="nx-bar-value">{formatSize(r.bytes)}</span>
          </button>
          {r.sub && <span className="nx-bar-sub nx-ellipsis">{r.sub}</span>}
        </li>
      ))}
    </ul>
  );
}

export function StorageView({ path, ctx }: { path?: string; ctx: ToolContext }) {
  const task = useTask<StorageReport>();
  const [choice, setChoice] = useState("");

  useEffect(() => {
    if (path) void task.start(() => api.startStorageScan(path));
    // Restart only when the analysed folder changes.
  }, [path]); // run once / only when the input changes

  if (!path) {
    return (
      <div className="nx-tool">
        <header className="nx-tool-header">
          <ChartPie size={22} />
          <div>
            <h1>Storage analysis</h1>
            <p className="nx-muted">See which folders, files and file types use the most space. Nothing is changed.</p>
          </div>
        </header>
        <div className="nx-card-grid is-drives">
          {ctx.drives.filter((d) => d.available).map((d) => (
            <button key={d.path} type="button" className="nx-card" onClick={() => ctx.navigate({ type: "tool", tool: "storage", path: d.path })}>
              <HardDrive size={24} className="nx-card-icon" />
              <span className="nx-card-text">
                <span className="nx-card-title">
                  {d.label} ({d.letter}:)
                </span>
                <span className="nx-card-sub">
                  {formatSize(d.freeBytes)} free of {formatSize(d.totalBytes)}
                </span>
              </span>
            </button>
          ))}
          {ctx.knownFolders.map((f) => (
            <button key={f.id} type="button" className="nx-card" onClick={() => ctx.navigate({ type: "tool", tool: "storage", path: f.path })}>
              <FolderOpen size={22} className="nx-card-icon" />
              <span className="nx-card-text">
                <span className="nx-card-title">{f.label}</span>
                <span className="nx-card-sub nx-ellipsis">{f.path}</span>
              </span>
            </button>
          ))}
        </div>
        <form
          className="nx-inline-form"
          onSubmit={(e) => {
            e.preventDefault();
            if (choice.trim()) ctx.navigate({ type: "tool", tool: "storage", path: choice.trim() });
          }}
        >
          <input className="nx-input" placeholder="Or enter a folder path, e.g. D:\Projects" value={choice} onChange={(e) => setChoice(e.target.value)} aria-label="Folder to analyze" />
          <button type="submit" className="nx-button is-primary">
            Analyze
          </button>
        </form>
      </div>
    );
  }

  const r = task.result;
  const openFolder = (p: string) => ctx.navigate({ type: "tool", tool: "storage", path: p });
  return (
    <div className="nx-tool">
      <header className="nx-tool-header">
        <ChartPie size={22} />
        <div className="nx-tool-title">
          <h1>Storage analysis</h1>
          <p className="nx-muted nx-ellipsis" title={path}>
            {path}
          </p>
        </div>
        <span className="nx-spacer" />
        <button type="button" className="nx-button" onClick={() => ctx.navigate({ type: "folder", path })}>
          Open folder
        </button>
        <button type="button" className="nx-button" disabled={task.running} onClick={() => void task.start(() => api.startStorageScan(path))}>
          <RefreshCw size={14} /> Rescan
        </button>
      </header>

      <TaskStatus progress={task.progress} onCancel={task.cancel} />
      {task.error && <p className="nx-field-error">{task.error}</p>}

      {r && (
        <>
          {!r.complete && (
            <p className="nx-notice">
              <TriangleAlert size={14} /> The scan was stopped early. Numbers below are incomplete.
            </p>
          )}
          <div className="nx-stats">
            <div>
              <span className="nx-stat-value">{formatSize(r.totalBytes)}</span>
              <span className="nx-stat-label">in this folder</span>
            </div>
            <div>
              <span className="nx-stat-value">{r.fileCount.toLocaleString()}</span>
              <span className="nx-stat-label">files</span>
            </div>
            <div>
              <span className="nx-stat-value">{r.folderCount.toLocaleString()}</span>
              <span className="nx-stat-label">folders</span>
            </div>
            {r.volume && (
              <div>
                <span className="nx-stat-value">{formatSize(r.volume.freeBytes)}</span>
                <span className="nx-stat-label">free of {formatSize(r.volume.totalBytes)}</span>
              </div>
            )}
            {(r.unreadable > 0 || r.skippedLinks > 0) && (
              <div>
                <span className="nx-stat-value">{(r.unreadable + r.skippedLinks).toLocaleString()}</span>
                <span className="nx-stat-label">
                  skipped ({r.unreadable} unreadable, {r.skippedLinks} links)
                </span>
              </div>
            )}
          </div>
          {r.volume && (
            <div className="nx-meter is-large" role="meter" aria-valuenow={percent(r.volume.totalBytes - r.volume.freeBytes, r.volume.totalBytes)} aria-valuemin={0} aria-valuemax={100} aria-label="Drive usage">
              <span style={{ width: `${percent(r.volume.totalBytes - r.volume.freeBytes, r.volume.totalBytes)}%` }} />
            </div>
          )}

          <section>
            <h2 className="nx-home-heading">Contents</h2>
            {r.children.length === 0 ? (
              <p className="nx-muted">This folder is empty.</p>
            ) : (
              <Treemap nodes={r.children} total={r.totalBytes} onOpen={(n) => (n.isDir ? openFolder(n.path) : ctx.reveal(n.path))} />
            )}
            {r.otherChildren > 0 && <p className="nx-muted">…and {pluralize(r.otherChildren, "smaller item")} ({formatSize(r.otherChildrenBytes)}).</p>}
          </section>

          <div className="nx-tool-columns">
            <section>
              <h2 className="nx-home-heading">Largest folders</h2>
              <Bars
                rows={r.largestFolders.slice(0, 15).map((f) => ({ key: f.path, label: f.name, bytes: f.bytes, path: f.path, sub: f.path }))}
                total={r.totalBytes}
                onOpen={openFolder}
              />
            </section>
            <section>
              <h2 className="nx-home-heading">By type</h2>
              <Bars rows={r.categories.map((c) => ({ key: c.key, label: `${CATEGORY_LABELS[c.key] ?? c.key} · ${c.files.toLocaleString()}`, bytes: c.bytes }))} total={r.totalBytes} />
              <h2 className="nx-home-heading">Top extensions</h2>
              <Bars rows={r.extensions.slice(0, 8).map((c) => ({ key: c.key, label: `.${c.key} · ${c.files.toLocaleString()}`, bytes: c.bytes }))} total={r.totalBytes} />
            </section>
          </div>

          <section>
            <h2 className="nx-home-heading">Largest files</h2>
            <table className="nx-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Folder</th>
                  <th>Modified</th>
                  <th className="is-num">Size</th>
                </tr>
              </thead>
              <tbody>
                {r.largestFiles.slice(0, 50).map((f) => (
                  <tr key={f.path} onDoubleClick={() => ctx.reveal(f.path)} onContextMenu={(e) => ctx.itemsMenu(e, [asEntry(f.path, f.bytes, f.modified)])}>
                    <td>
                      <button type="button" className="nx-link-button" onClick={() => ctx.reveal(f.path)}>
                        {f.name}
                      </button>
                    </td>
                    <td className="nx-muted nx-ellipsis" title={f.path}>
                      {f.path.slice(0, f.path.lastIndexOf("\\"))}
                    </td>
                    <td className="nx-muted">{formatDate(f.modified)}</td>
                    <td className="is-num">{formatSize(f.bytes)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        </>
      )}
    </div>
  );
}
