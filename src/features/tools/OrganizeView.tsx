import { useEffect, useMemo, useState } from "react";
import { Eye, Plus, Save, Trash2, TriangleAlert, WandSparkles } from "lucide-react";
import { api, errorMessage } from "../../services/api";
import { formatSize, pluralize } from "../../lib/format";
import { ConfirmDialog } from "../explorer/dialogs";
import type { Plan, Rule } from "../../types/tools";
import { FolderInput, type ToolContext } from "./shared";

export function newRule(ctx: Pick<ToolContext, "knownFolders">): Rule {
  const downloads = ctx.knownFolders.find((f) => f.id === "downloads")?.path ?? "";
  const documents = ctx.knownFolders.find((f) => f.id === "documents")?.path ?? "";
  return {
    id: `r${Date.now().toString(36)}`,
    name: "PDFs from Downloads",
    source: downloads,
    includeSubfolders: false,
    extensions: ["pdf"],
    nameContains: null,
    olderThanDays: null,
    minSize: null,
    action: "move",
    destination: documents ? `${documents}\\PDF` : "",
    policy: "keepBoth",
  };
}

/** One-line summary such as "Move .pdf files from Downloads to Documents\PDF". */
export function describeRule(r: Rule): string {
  const what = r.extensions.length ? r.extensions.map((e) => `.${e}`).join(", ") + " files" : "all files";
  const conditions = [
    r.nameContains ? `named "${r.nameContains}"` : null,
    r.olderThanDays ? `older than ${r.olderThanDays} days` : null,
    r.minSize ? `≥ ${formatSize(r.minSize)}` : null,
  ].filter(Boolean);
  return `${r.action === "move" ? "Move" : "Copy"} ${what}${conditions.length ? ` (${conditions.join(", ")})` : ""} from ${r.source || "…"}${r.includeSubfolders ? " and subfolders" : ""} to ${r.destination || "…"}`;
}

export function OrganizeView({ ctx }: { ctx: ToolContext }) {
  const [rules, setRules] = useState<Rule[] | null>(null);
  const [current, setCurrent] = useState<Rule | null>(null);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [approved, setApproved] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);

  useEffect(() => {
    api
      .rulesGet()
      .then((r) => {
        setRules(r);
        setCurrent(r[0] ?? null);
      })
      .catch((e) => ctx.notify("error", "Could not load rules.", errorMessage(e)));
  }, []); // run once / only when the input changes

  const dirty = useMemo(() => !!current && JSON.stringify(rules?.find((r) => r.id === current.id)) !== JSON.stringify(current), [rules, current]);

  if (!rules) return <div className="nx-tool" />;

  const persist = async (next: Rule[]) => {
    try {
      await api.rulesSave(next);
      setRules(next);
    } catch (e) {
      ctx.notify("error", "Could not save the rules.", errorMessage(e));
    }
  };
  const edit = (patch: Partial<Rule>) => {
    setCurrent((c) => (c ? { ...c, ...patch } : c));
    setPlan(null);
  };
  const saveCurrent = async () => {
    if (!current) return;
    const exists = rules.some((r) => r.id === current.id);
    await persist(exists ? rules.map((r) => (r.id === current.id ? current : r)) : [...rules, current]);
    ctx.notify("success", `Saved rule "${current.name}".`);
  };
  const preview = async () => {
    if (!current) return;
    setBusy(true);
    try {
      const p = await api.organizePreview(current);
      setPlan(p);
      setApproved(new Set(p.items.map((i) => i.source)));
    } catch (e) {
      setPlan(null);
      ctx.notify("error", "The rule cannot be previewed.", errorMessage(e));
    }
    setBusy(false);
  };
  const apply = async () => {
    if (!current || !plan) return;
    try {
      await api.organizeApply(current, [...approved]);
      ctx.notify("success", `${current.action === "move" ? "Moving" : "Copying"} ${pluralize(approved.size, "file")}…`, "Progress and a detailed report are in File operations.");
      setPlan(null);
    } catch (e) {
      ctx.notify("error", "The rule could not be applied.", errorMessage(e));
    }
  };

  return (
    <div className="nx-tool">
      <header className="nx-tool-header">
        <WandSparkles size={22} />
        <div className="nx-tool-title">
          <h1>Organize</h1>
          <p className="nx-muted">Rules sort files into folders. You always see a preview first, and nothing runs without your confirmation.</p>
        </div>
      </header>

      <div className="nx-organize">
        <nav className="nx-rule-list" aria-label="Rules">
          {rules.map((r) => (
            <button key={r.id} type="button" className={`nx-side-item${current?.id === r.id ? " is-active" : ""}`} onClick={() => (setCurrent(r), setPlan(null))}>
              <span className="nx-ellipsis">{r.name}</span>
            </button>
          ))}
          {current && !rules.some((r) => r.id === current.id) && (
            <button type="button" className="nx-side-item is-active">
              <span className="nx-ellipsis">{current.name} (unsaved)</span>
            </button>
          )}
          <button type="button" className="nx-button" onClick={() => (setCurrent(newRule(ctx)), setPlan(null))}>
            <Plus size={14} /> New rule
          </button>
        </nav>

        {!current ? (
          <p className="nx-muted">No rules yet. Create one to get started.</p>
        ) : (
          <div className="nx-rule-editor">
            <p className="nx-rule-summary">{describeRule(current)}</p>
            <div className="nx-form-grid">
              <label className="nx-field">
                <span>Rule name</span>
                <input className="nx-input" value={current.name} onChange={(e) => edit({ name: e.target.value })} />
              </label>
              <FolderInput label="Source folder" value={current.source} onChange={(source) => edit({ source })} ctx={ctx} />
              <label className="nx-field">
                <span>File extensions (comma separated, empty = all)</span>
                <input
                  className="nx-input"
                  value={current.extensions.join(", ")}
                  onChange={(e) => edit({ extensions: e.target.value.split(",").map((x) => x.trim().replace(/^\./, "").toLowerCase()).filter(Boolean) })}
                />
              </label>
              <label className="nx-field">
                <span>Name contains (optional)</span>
                <input className="nx-input" value={current.nameContains ?? ""} onChange={(e) => edit({ nameContains: e.target.value || null })} />
              </label>
              <label className="nx-field">
                <span>Older than (days, optional)</span>
                <input className="nx-input" type="number" min={1} value={current.olderThanDays ?? ""} onChange={(e) => edit({ olderThanDays: e.target.value ? Number(e.target.value) : null })} />
              </label>
              <label className="nx-field">
                <span>Minimum size in MB (optional)</span>
                <input
                  className="nx-input"
                  type="number"
                  min={0}
                  step="any"
                  value={current.minSize != null ? current.minSize / 1024 ** 2 : ""}
                  onChange={(e) => edit({ minSize: e.target.value ? Math.round(Number(e.target.value) * 1024 ** 2) : null })}
                />
              </label>
              <label className="nx-field">
                <span>Action</span>
                <select className="nx-input" value={current.action} onChange={(e) => edit({ action: e.target.value as Rule["action"] })}>
                  <option value="move">Move</option>
                  <option value="copy">Copy</option>
                </select>
              </label>
              <FolderInput label="Destination folder" value={current.destination} onChange={(destination) => edit({ destination })} ctx={ctx} />
              <label className="nx-field">
                <span>If a file with the same name exists</span>
                <select className="nx-input" value={current.policy} onChange={(e) => edit({ policy: e.target.value as Rule["policy"] })}>
                  <option value="keepBoth">Keep both (rename)</option>
                  <option value="skip">Skip the file</option>
                  <option value="replace">Replace the existing file</option>
                </select>
              </label>
              <label className="nx-check">
                <input type="checkbox" checked={current.includeSubfolders} onChange={(e) => edit({ includeSubfolders: e.target.checked })} /> Include subfolders of the source
              </label>
            </div>
            <div className="nx-tool-actions">
              <button type="button" className="nx-button is-primary" disabled={busy} onClick={() => void preview()}>
                <Eye size={14} /> Preview (dry run)
              </button>
              <button type="button" className="nx-button" disabled={!dirty && rules.some((r) => r.id === current.id)} onClick={() => void saveCurrent()}>
                <Save size={14} /> Save rule
              </button>
              <span className="nx-spacer" />
              <button
                type="button"
                className="nx-button"
                onClick={() => {
                  void persist(rules.filter((r) => r.id !== current.id));
                  setCurrent(rules.find((r) => r.id !== current.id) ?? null);
                  setPlan(null);
                }}
              >
                <Trash2 size={14} /> Delete rule
              </button>
            </div>

            {plan && (
              <section className="nx-plan">
                <h2 className="nx-home-heading">
                  Preview: {pluralize(plan.items.length, "file")} would be {current.action === "move" ? "moved" : "copied"}
                </h2>
                <p className="nx-muted">
                  {plan.scanned.toLocaleString()} files checked. Nothing has been changed yet.
                  {!plan.destinationExists && " The destination folder will be created."}
                  {plan.truncated && " Only the first 10,000 matches are listed."}
                </p>
                {plan.items.some((i) => i.conflict) && (
                  <p className="nx-notice">
                    <TriangleAlert size={14} /> {plan.items.filter((i) => i.conflict).length} names already exist at the destination — the rule will{" "}
                    {current.policy === "keepBoth" ? "keep both" : current.policy === "skip" ? "skip them" : "replace the existing files"}.
                  </p>
                )}
                {plan.items.length > 0 && (
                  <>
                    <table className="nx-table">
                      <thead>
                        <tr>
                          <th>
                            <input
                              type="checkbox"
                              aria-label="Select all"
                              checked={approved.size === plan.items.length}
                              onChange={(e) => setApproved(e.target.checked ? new Set(plan.items.map((i) => i.source)) : new Set())}
                            />
                          </th>
                          <th>File</th>
                          <th>New location</th>
                          <th className="is-num">Size</th>
                        </tr>
                      </thead>
                      <tbody>
                        {plan.items.slice(0, 500).map((i) => (
                          <tr key={i.source} className={i.conflict ? "is-warning" : undefined}>
                            <td>
                              <input
                                type="checkbox"
                                aria-label={`Include ${i.source}`}
                                checked={approved.has(i.source)}
                                onChange={() =>
                                  setApproved((s) => {
                                    const n = new Set(s);
                                    if (n.has(i.source)) n.delete(i.source);
                                    else n.add(i.source);
                                    return n;
                                  })
                                }
                              />
                            </td>
                            <td className="nx-ellipsis" title={i.source}>
                              {i.source}
                            </td>
                            <td className="nx-ellipsis nx-muted" title={i.target}>
                              {i.target}
                              {i.conflict ? " (exists)" : ""}
                            </td>
                            <td className="is-num">{formatSize(i.size)}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                    {plan.items.length > 500 && <p className="nx-muted">…and {plan.items.length - 500} more (all selected items are included).</p>}
                    <div className="nx-tool-actions">
                      <button type="button" className="nx-button is-primary" disabled={approved.size === 0} onClick={() => setConfirm(true)}>
                        {current.action === "move" ? "Move" : "Copy"} {pluralize(approved.size, "file")}
                      </button>
                    </div>
                  </>
                )}
              </section>
            )}
          </div>
        )}
      </div>
      {confirm && current && (
        <ConfirmDialog
          title="Apply rule?"
          message={`${current.action === "move" ? "Move" : "Copy"} ${pluralize(approved.size, "file")} to ${current.destination}?`}
          detail={<p className="nx-muted">Files are processed through the normal file operation queue. You can cancel it there, and a report lists every file.</p>}
          confirmLabel={current.action === "move" ? "Move files" : "Copy files"}
          onClose={() => setConfirm(false)}
          onConfirm={() => void apply()}
        />
      )}
    </div>
  );
}
