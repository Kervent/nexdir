import { useEffect, useMemo, useState } from "react";
import { Database, Search, Trash2, X } from "lucide-react";
import { api, errorMessage } from "../../services/api";
import { emptySelection, type Selection } from "../../lib/selection";
import { formatDate, formatSize, pluralize } from "../../lib/format";
import { sortEntries } from "../../lib/sort";
import { FileView } from "../explorer/FileView";
import { parseQuery } from "../search/queryParser";
import type { IndexSettings, IndexStatus, SearchQuery, SearchResult } from "../../types/tools";
import { FolderInput, resolveScope, TaskStatus, type ToolContext } from "./shared";
import { useTask } from "./useTask";

const EXAMPLES = ["videos larger than 2 GB", "PDF files in Downloads", "images from last month", "*.log older than 30 days", "folders named src"];

export function buildQuery(input: string, root: string, opts: { includeHidden: boolean; recursive: boolean }, ctx: Pick<ToolContext, "knownFolders">): { query: SearchQuery; chips: string[] } {
  const parsed = parseQuery(input);
  const scope = resolveScope(parsed.scope, ctx as ToolContext) ?? root;
  return {
    query: {
      root: scope,
      recursive: opts.recursive,
      includeHidden: opts.includeHidden,
      text: parsed.text,
      extensions: parsed.extensions,
      categories: parsed.categories,
      minSize: parsed.minSize,
      maxSize: parsed.maxSize,
      modifiedAfter: parsed.modifiedAfter,
      modifiedBefore: parsed.modifiedBefore,
      kind: parsed.kind,
      limit: 5000,
    },
    chips: parsed.chips,
  };
}

function IndexPanel({ ctx, onClose }: { ctx: ToolContext; onClose: () => void }) {
  const [settings, setSettings] = useState<IndexSettings | null>(null);
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [newFolder, setNewFolder] = useState("");
  const build = useTask<IndexStatus>();

  const refresh = () => {
    api.indexStatus().then(setStatus).catch(() => {});
  };
  useEffect(() => {
    api.indexGetSettings().then(setSettings).catch((e) => ctx.notify("error", "Could not load index settings.", errorMessage(e)));
    refresh();
  }, []); // run once / only when the input changes
  useEffect(() => {
    if (build.progress && build.progress.state !== "running") refresh();
  }, [build.progress]);

  if (!settings) return null;
  const save = async (next: IndexSettings) => {
    setSettings(next);
    try {
      await api.indexSaveSettings(next);
    } catch (e) {
      ctx.notify("error", "Could not save index settings.", errorMessage(e));
    }
  };

  return (
    <section className="nx-index-panel" aria-label="Search index settings">
      <header className="nx-dup-head">
        <Database size={16} />
        <strong>Search index</strong>
        <span className="nx-muted">Optional. Stored only on this PC.</span>
        <span className="nx-spacer" />
        <button type="button" className="nx-icon-button is-small" aria-label="Close index settings" onClick={onClose}>
          <X size={14} />
        </button>
      </header>
      <p className="nx-muted">
        {status?.exists
          ? `${status.files.toLocaleString()} items indexed · ${formatSize(status.sizeBytes)} · updated ${formatDate(status.lastBuilt)}`
          : "No index yet. Without an index, searches walk the folder live."}
      </p>
      <ul className="nx-index-folders">
        {settings.folders.map((f) => (
          <li key={f}>
            <span className="nx-ellipsis">{f}</span>
            <button type="button" className="nx-icon-button is-small" aria-label={`Remove ${f}`} onClick={() => void save({ ...settings, folders: settings.folders.filter((x) => x !== f) })}>
              <X size={13} />
            </button>
          </li>
        ))}
      </ul>
      <form
        className="nx-inline-form"
        onSubmit={(e) => {
          e.preventDefault();
          const f = newFolder.trim();
          if (f && !settings.folders.includes(f)) void save({ ...settings, folders: [...settings.folders, f] });
          setNewFolder("");
        }}
      >
        <FolderInput label="Add folder to index" value={newFolder} onChange={setNewFolder} ctx={ctx} />
        <button type="submit" className="nx-button">
          Add
        </button>
      </form>
      <label className="nx-check">
        <input type="checkbox" checked={settings.includeContent} onChange={(e) => void save({ ...settings, includeContent: e.target.checked })} /> Also index the text inside small text files (≤ 1 MB)
      </label>
      <label className="nx-check">
        <input type="checkbox" checked={settings.updateOnStartup} onChange={(e) => void save({ ...settings, updateOnStartup: e.target.checked })} /> Update the index when Nexdir starts
      </label>
      <TaskStatus progress={build.progress} onCancel={build.cancel} unit="items" />
      {build.error && <p className="nx-field-error">{build.error}</p>}
      <div className="nx-tool-actions">
        <button type="button" className="nx-button is-primary" disabled={build.running || settings.folders.length === 0} onClick={() => void build.start(() => api.indexBuild())}>
          {status?.exists ? "Update index now" : "Build index"}
        </button>
        <button
          type="button"
          className="nx-button"
          disabled={build.running || !status?.exists}
          onClick={() =>
            api
              .indexDelete()
              .then(() => {
                ctx.notify("success", "The search index was deleted.");
                refresh();
              })
              .catch((e) => ctx.notify("error", "Could not delete the index.", errorMessage(e)))
          }
        >
          <Trash2 size={14} /> Delete index
        </button>
      </div>
    </section>
  );
}

export function SearchView({ path, ctx }: { path?: string; ctx: ToolContext }) {
  const home = ctx.knownFolders.find((f) => f.id === "home")?.path ?? "C:\\";
  const [input, setInput] = useState("");
  const [root, setRoot] = useState(path ?? home);
  const [recursive, setRecursive] = useState(true);
  const [includeHidden, setIncludeHidden] = useState(false);
  const [useIndex, setUseIndex] = useState(false);
  const [indexReady, setIndexReady] = useState(false);
  const [showIndex, setShowIndex] = useState(false);
  const [indexResult, setIndexResult] = useState<SearchResult | null>(null);
  const [selection, setSelection] = useState<Selection>(emptySelection);
  const live = useTask<SearchResult>();

  useEffect(() => {
    api.indexStatus().then((s) => setIndexReady(s.exists)).catch(() => {});
  }, [showIndex]);

  const preview = useMemo(() => (input.trim() ? buildQuery(input, root, { includeHidden, recursive }, ctx).chips : []), [input, root, includeHidden, recursive, ctx]);
  const result = useIndex ? indexResult : live.result;
  const entries = useMemo(() => sortEntries(result?.entries ?? [], ctx.prefs.sort), [result, ctx.prefs.sort]);

  const run = async () => {
    const { query } = buildQuery(input, root, { includeHidden, recursive }, ctx);
    setSelection(emptySelection);
    if (useIndex) {
      try {
        // An empty location searches everything in the index.
        setIndexResult(await api.indexSearch({ ...query, root: root.trim() ? query.root : "" }));
      } catch (e) {
        ctx.notify("error", "Index search failed.", errorMessage(e));
      }
    } else {
      void live.start(() => api.startSearch(query));
    }
  };

  return (
    <div className="nx-tool is-search">
      <header className="nx-tool-header">
        <Search size={22} />
        <div className="nx-tool-title">
          <h1>Search</h1>
          <p className="nx-muted">Type what you are looking for, e.g. "{EXAMPLES[0]}". Everything is processed on this PC.</p>
        </div>
        <span className="nx-spacer" />
        <button type="button" className={`nx-button${showIndex ? " is-active" : ""}`} onClick={() => setShowIndex((s) => !s)}>
          <Database size={14} /> Index settings
        </button>
      </header>

      <form
        className="nx-search-form"
        onSubmit={(e) => {
          e.preventDefault();
          void run();
        }}
      >
        <input className="nx-input is-large" type="search" autoFocus aria-label="Search request" placeholder="What are you looking for?" value={input} onChange={(e) => setInput(e.target.value)} />
        <button type="submit" className="nx-button is-primary" disabled={live.running}>
          Search
        </button>
      </form>
      {preview.length > 0 && (
        <div className="nx-chips" aria-label="How your request is understood">
          {preview.map((c) => (
            <span key={c} className="nx-chip">
              {c}
            </span>
          ))}
        </div>
      )}
      {!input && (
        <div className="nx-chips">
          {EXAMPLES.map((ex) => (
            <button key={ex} type="button" className="nx-chip is-button" onClick={() => setInput(ex)}>
              {ex}
            </button>
          ))}
        </div>
      )}
      <div className="nx-tool-form">
        <FolderInput label="Look in" value={root} onChange={setRoot} ctx={ctx} />
        <label className="nx-check">
          <input type="checkbox" checked={recursive} onChange={(e) => setRecursive(e.target.checked)} /> Include subfolders
        </label>
        <label className="nx-check">
          <input type="checkbox" checked={includeHidden} onChange={(e) => setIncludeHidden(e.target.checked)} /> Hidden files
        </label>
        <label className="nx-check" title={indexReady ? "Search the index instead of walking folders" : "Build an index first (Index settings)"}>
          <input type="checkbox" checked={useIndex} disabled={!indexReady} onChange={(e) => setUseIndex(e.target.checked)} /> Use search index
        </label>
      </div>

      {showIndex && <IndexPanel ctx={ctx} onClose={() => setShowIndex(false)} />}

      <TaskStatus progress={live.progress} onCancel={live.cancel} unit="items checked" />
      {live.error && !useIndex && <p className="nx-field-error">{live.error}</p>}

      {result && (
        <p className="nx-muted">
          {pluralize(result.entries.length, "result")}
          {result.source === "index" ? " from the index" : ` · ${result.scanned.toLocaleString()} items checked`}
          {result.truncated ? " · showing the first results only, refine your search" : ""}
          {!result.complete && !result.truncated ? " · stopped early" : ""}
        </p>
      )}
      {result && (
        <div className="nx-search-results">
          <FileView
            entries={entries}
            selection={selection}
            onSelectionChange={setSelection}
            onOpen={ctx.openEntry}
            onContextMenu={(e, entry) => {
              e.preventDefault();
              if (!entry) return;
              const items = selection.selected.has(entry.path) ? entries.filter((x) => selection.selected.has(x.path)) : [entry];
              ctx.itemsMenu(e, items);
            }}
            viewMode="details"
            sort={ctx.prefs.sort}
            onSortChange={(sort) => ctx.updatePrefs({ sort })}
            folderPath={null}
            onDropPaths={() => {}}
            onMiddleClick={(entry) => ctx.openInNewTab({ type: "folder", path: entry.path })}
            showLocation
            empty={<div className="nx-state is-empty">No matches.</div>}
          />
        </div>
      )}
    </div>
  );
}
