import { useEffect, useId, useMemo, useState } from "react";
import { ArrowLeft, Clipboard, ExternalLink, ShieldCheck } from "lucide-react";
import { Dialog } from "../../components/Dialog";
import { chooseIssueLink, ISSUE_FORMS, prepareIssue, validateIssue, windowsLabel, type IssueKind, type Redaction, type SystemInfo } from "./report";

const REDACTION_LABELS: Record<Redaction["kind"], [string, string]> = {
  path: ["file path", "file paths"],
  user: ["user name", "user names"],
  email: ["e-mail address", "e-mail addresses"],
  secret: ["password, token or key", "passwords, tokens or keys"],
};

interface IssueDialogProps {
  kind: IssueKind;
  /** Reads version facts on this PC; nothing goes over the network. */
  loadSystemInfo: () => Promise<SystemInfo>;
  /** Words to hide in the report, such as the Windows user name. */
  userNames: string[];
  openUrl: (url: string) => Promise<void>;
  copyText: (text: string) => Promise<void>;
  /** Called after GitHub was opened, with whether the report had to go through the clipboard. */
  onOpened: (viaClipboard: boolean) => void;
  onClose: () => void;
}

/**
 * Bug report / feature request form. The user fills it in, reviews the cleaned report and
 * the technical details, and Nexdir opens the prefilled GitHub issue page — the issue is only
 * created if the user submits it there.
 */
export function IssueDialog({ kind, loadSystemInfo, userNames, openUrl, copyText, onOpened, onClose }: IssueDialogProps) {
  const form = ISSUE_FORMS[kind];
  const uid = useId();
  const [stage, setStage] = useState<"edit" | "review">("edit");
  const [title, setTitle] = useState("");
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(form.fields.map((f) => [f.id, ""])));
  const [touched, setTouched] = useState(false);
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [infoError, setInfoError] = useState(false);
  const [include, setInclude] = useState({ version: true, windows: kind === "bug", architecture: kind === "bug" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    loadSystemInfo()
      .then(setInfo)
      .catch(() => setInfoError(true));
  }, [loadSystemInfo]);

  const errors = validateIssue(kind, title, values);
  const valid = Object.keys(errors).length === 0;
  const prepared = useMemo(
    () =>
      prepareIssue(
        {
          kind,
          title,
          values,
          system: {
            version: include.version ? info?.appVersion : undefined,
            windows: include.windows && info ? windowsLabel(info) : undefined,
            architecture: include.architecture ? info?.architecture : undefined,
          },
        },
        userNames,
      ),
    [kind, title, values, include, info, userNames],
  );
  const link = useMemo(() => chooseIssueLink(kind, prepared), [kind, prepared]);
  const preview = `${form.titlePrefix}${prepared.title}\n\n${prepared.markdown}`;

  const copy = async () => {
    try {
      await copyText(preview);
      setCopied(true);
      return true;
    } catch {
      setError("Could not copy to the clipboard. Select the text in the preview and copy it manually.");
      return false;
    }
  };

  const open = async () => {
    setBusy(true);
    setError(null);
    try {
      if (!link.prefilled && !(await copy())) return;
      await openUrl(link.url);
      onOpened(!link.prefilled);
    } catch (e) {
      setError(`GitHub could not be opened: ${e instanceof Error ? e.message : String((e as { message?: string })?.message ?? e)}. Use “Copy report” and open the issue page on GitHub yourself.`);
    } finally {
      setBusy(false);
    }
  };

  const heading = kind === "bug" ? "Report a Bug" : "Request a Feature";
  const fieldError = (id: string) => (touched ? errors[id] : undefined);

  if (stage === "edit") {
    return (
      <Dialog
        title={heading}
        onClose={onClose}
        className="is-wide"
        footer={
          <>
            <button type="button" className="nx-button" onClick={onClose}>
              Cancel
            </button>
            <button
              type="button"
              className="nx-button is-primary"
              onClick={() => {
                setTouched(true);
                if (valid) setStage("review");
              }}
            >
              Review report
            </button>
          </>
        }
      >
        <p className="nx-muted">
          {kind === "bug"
            ? "Describe what went wrong. Next, you'll see exactly what will be shared before anything leaves this PC."
            : "Describe your idea. Next, you'll see the request before GitHub opens."}
        </p>
        <label className="nx-field">
          <span>{kind === "bug" ? "Title" : "Feature title"}</span>
          <input
            className="nx-input"
            value={title}
            maxLength={250}
            onChange={(e) => setTitle(e.target.value)}
            aria-invalid={!!fieldError("title")}
            aria-describedby={fieldError("title") ? `${uid}-title-err` : undefined}
            placeholder={kind === "bug" ? "Short summary of the problem" : "Short summary of the idea"}
          />
          {fieldError("title") && (
            <span id={`${uid}-title-err`} className="nx-field-error">
              {fieldError("title")}
            </span>
          )}
        </label>
        {form.fields.map((f) => {
          const err = fieldError(f.id);
          const common = {
            id: `${uid}-${f.id}`,
            value: values[f.id] ?? "",
            "aria-invalid": !!err,
            "aria-describedby": err ? `${uid}-${f.id}-err` : undefined,
            "aria-required": f.required,
          };
          return (
            <div key={f.id} className="nx-field">
              <label htmlFor={common.id}>{f.label}</label>
              {f.options ? (
                <select {...common} className="nx-input" onChange={(e) => setValues((v) => ({ ...v, [f.id]: e.target.value }))}>
                  <option value="">Choose…</option>
                  {f.options.map((o) => (
                    <option key={o} value={o}>
                      {o}
                    </option>
                  ))}
                </select>
              ) : (
                <textarea {...common} className="nx-input nx-textarea" rows={f.id === "steps" ? 4 : 3} placeholder={f.placeholder} onChange={(e) => setValues((v) => ({ ...v, [f.id]: e.target.value }))} />
              )}
              {err && (
                <span id={`${common.id}-err`} className="nx-field-error">
                  {err}
                </span>
              )}
            </div>
          );
        })}
      </Dialog>
    );
  }

  return (
    <Dialog
      title={`${heading}: review`}
      onClose={onClose}
      className="is-wide"
      initialFocus=".is-primary"
      footer={
        <>
          <button type="button" className="nx-button" onClick={() => setStage("edit")}>
            <ArrowLeft size={14} /> Back
          </button>
          <span className="nx-spacer" />
          <button type="button" className="nx-button" onClick={() => void copy()}>
            <Clipboard size={14} /> {copied ? "Copied" : "Copy report"}
          </button>
          <button type="button" className="nx-button is-primary" disabled={busy} onClick={() => void open()}>
            <ExternalLink size={14} /> Open GitHub
          </button>
        </>
      }
    >
      <p className="nx-notice is-info">
        <ShieldCheck size={14} />
        <span>
          Nothing has been sent. “Open GitHub” opens the issue page in your browser with this text filled in; the issue is only created if you submit it there (a GitHub account is required).
        </span>
      </p>

      <fieldset className="nx-fieldset">
        <legend>Technical details</legend>
        {infoError && <p className="nx-field-error">The version details could not be read. You can add them on GitHub.</p>}
        <label className="nx-check">
          <input type="checkbox" checked={include.version} disabled={!info} onChange={(e) => setInclude((s) => ({ ...s, version: e.target.checked }))} /> Nexdir version: {info?.appVersion ?? "…"}
        </label>
        <label className="nx-check">
          <input type="checkbox" checked={include.windows} disabled={!info} onChange={(e) => setInclude((s) => ({ ...s, windows: e.target.checked }))} /> Windows version: {info ? windowsLabel(info) : "…"}
        </label>
        <label className="nx-check">
          <input type="checkbox" checked={include.architecture} disabled={!info} onChange={(e) => setInclude((s) => ({ ...s, architecture: e.target.checked }))} /> Architecture: {info?.architecture ?? "…"}
        </label>
        <p className="nx-muted">No file names, folder paths, logs or other data from this PC are added. Nexdir keeps no log files.</p>
      </fieldset>

      {prepared.redactions.length > 0 && (
        <p className="nx-notice" role="status">
          <ShieldCheck size={14} />
          <span>
            For your privacy, Nexdir replaced{" "}
            {prepared.redactions.map((r, i) => `${i > 0 ? (i === prepared.redactions.length - 1 ? " and " : ", ") : ""}${r.count} ${REDACTION_LABELS[r.kind][r.count === 1 ? 0 : 1]}`).join("")} with placeholders.
          </span>
        </p>
      )}

      <div className="nx-field">
        <span>What GitHub will receive</span>
        <pre className="nx-report-preview" tabIndex={0} aria-label="Report preview">
          {preview}
        </pre>
      </div>
      {!link.prefilled && (
        <p className="nx-notice" role="status">
          This report is too long to pass to GitHub directly. “Open GitHub” copies it to the clipboard first; paste it into the form there.
        </p>
      )}
      {error && (
        <p className="nx-field-error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
