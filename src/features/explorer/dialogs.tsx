import { useState } from "react";
import { Dialog } from "../../components/Dialog";
import { validateName } from "../../lib/paths";
import { pluralize } from "../../lib/format";
import type { ConflictPolicy, OperationReport, TransferKind } from "../../types/fs";

export function NameDialog({ title, label, initial, confirmLabel, onConfirm, onClose }: {
  title: string;
  label: string;
  initial: string;
  confirmLabel: string;
  /** Returns an error message to show, or null when done. */
  onConfirm: (name: string) => Promise<string | null>;
  onClose: () => void;
}) {
  const [name, setName] = useState(initial);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const localError = name === initial ? null : validateName(name);

  const submit = async () => {
    const problem = validateName(name);
    if (problem) return setError(problem);
    setBusy(true);
    const err = await onConfirm(name);
    setBusy(false);
    if (err) setError(err);
  };

  return (
    <Dialog
      title={title}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="nx-button" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="nx-button is-primary" disabled={busy || !!localError || !name.trim()} onClick={() => void submit()}>
            {confirmLabel}
          </button>
        </>
      }
    >
      <label className="nx-field">
        <span>{label}</span>
        <input
          className="nx-input"
          value={name}
          spellCheck={false}
          aria-invalid={!!(error ?? localError)}
          onChange={(e) => {
            setName(e.target.value);
            setError(null);
          }}
          onFocus={(e) => {
            // Select the name without its extension, like Explorer.
            const dot = e.target.value.lastIndexOf(".");
            if (dot > 0) e.target.setSelectionRange(0, dot);
          }}
          onKeyDown={(e) => e.key === "Enter" && void submit()}
        />
      </label>
      {(error ?? localError) && (
        <p className="nx-field-error" role="alert">
          {error ?? localError}
        </p>
      )}
    </Dialog>
  );
}

export function ConfirmDialog({ title, message, detail, confirmLabel, danger, onConfirm, onClose }: {
  title: string;
  message: string;
  detail?: React.ReactNode;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <Dialog
      title={title}
      onClose={onClose}
      initialFocus="button.is-primary, button.is-danger"
      footer={
        <>
          <button type="button" className="nx-button" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className={`nx-button ${danger ? "is-danger" : "is-primary"}`}
            onClick={() => {
              onClose();
              onConfirm();
            }}
          >
            {confirmLabel}
          </button>
        </>
      }
    >
      <p>{message}</p>
      {detail}
    </Dialog>
  );
}

export function ConflictDialog({ kind, conflicts, total, onChoose, onClose }: {
  kind: TransferKind;
  conflicts: string[];
  total: number;
  onChoose: (policy: ConflictPolicy) => void;
  onClose: () => void;
}) {
  const choose = (p: ConflictPolicy) => {
    onClose();
    onChoose(p);
  };
  const shown = conflicts.slice(0, 6);
  return (
    <Dialog
      title={conflicts.length === 1 ? "An item with this name already exists" : `${conflicts.length} items already exist`}
      onClose={onClose}
      initialFocus="button.is-primary"
      footer={
        <button type="button" className="nx-button" onClick={onClose}>
          Cancel
        </button>
      }
    >
      <p>
        The destination already contains {conflicts.length === 1 ? "an item" : "items"} with the same name. Choose how to {kind}{" "}
        {pluralize(total, "item")}:
      </p>
      <ul className="nx-conflict-names">
        {shown.map((n) => (
          <li key={n} className="nx-ellipsis">
            {n}
          </li>
        ))}
        {conflicts.length > shown.length && <li>…and {conflicts.length - shown.length} more</li>}
      </ul>
      <div className="nx-choice-list">
        <button type="button" className="nx-choice is-primary" onClick={() => choose("keepBoth")}>
          <strong>Keep both</strong>
          <span>Transferred items get a new name, e.g. "report (2).pdf".</span>
        </button>
        <button type="button" className="nx-choice" onClick={() => choose("skip")}>
          <strong>Skip these items</strong>
          <span>Existing items stay unchanged; only the others are transferred.</span>
        </button>
        <button type="button" className="nx-choice is-danger" onClick={() => choose("replace")}>
          <strong>Replace</strong>
          <span>Existing files are overwritten. Folders are merged; nothing else in them is removed.</span>
        </button>
      </div>
    </Dialog>
  );
}

export function ReportDialog({ title, report, onClose }: { title: string; report: OperationReport | null; onClose: () => void }) {
  return (
    <Dialog
      title={title}
      onClose={onClose}
      footer={
        <button type="button" className="nx-button is-primary" onClick={onClose}>
          Close
        </button>
      }
    >
      {!report ? (
        <p>No details are available for this operation.</p>
      ) : (
        <div className="nx-report">
          <p>
            {pluralize(report.completed.length, "item")} processed, {pluralize(report.failures.length, "error")},{" "}
            {report.skipped.length} skipped.
            {report.truncated && " The lists below were shortened."}
          </p>
          {report.failures.length > 0 && (
            <>
              <h3>Errors</h3>
              <ul>
                {report.failures.map((f, i) => (
                  <li key={i}>
                    <code>{f.path}</code>
                    <span>{f.message}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
          {report.skipped.length > 0 && (
            <>
              <h3>Skipped</h3>
              <ul>
                {report.skipped.map((f, i) => (
                  <li key={i}>
                    <code>{f.path}</code>
                    <span>{f.message}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
          {report.completed.length > 0 && (
            <details>
              <summary>Processed items ({report.completed.length})</summary>
              <ul>
                {report.completed.map((p, i) => (
                  <li key={i}>
                    <code>{p}</code>
                  </li>
                ))}
              </ul>
            </details>
          )}
        </div>
      )}
    </Dialog>
  );
}
