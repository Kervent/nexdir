import { CheckCircle2, CircleSlash, Copy, MoveRight, TriangleAlert, X } from "lucide-react";
import { formatSize, percent, pluralize } from "../../lib/format";
import { baseName } from "../../lib/paths";
import type { OperationProgress } from "../../types/fs";
import type { TransferRate } from "./useOperations";

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds} s`;
  const m = Math.floor(seconds / 60);
  if (m < 60) return `${m} min ${seconds % 60} s`;
  return `${Math.floor(m / 60)} h ${m % 60} min`;
}

interface OperationsPanelProps {
  operations: OperationProgress[];
  onCancel: (id: number) => void;
  onDismiss: (id: number) => void;
  onShowReport: (p: OperationProgress) => void;
  onClose: () => void;
  rateOf: (p: OperationProgress) => TransferRate | null;
}

function describe(p: OperationProgress): string {
  const verb = p.kind === "copy" ? "Copying" : "Moving";
  const done = p.kind === "copy" ? "Copied" : "Moved";
  const target = baseName(p.destination);
  switch (p.state) {
    case "queued":
      return `Waiting to ${p.kind} ${pluralize(p.sourceCount, "item")} to ${target}`;
    case "scanning":
      return `Preparing to ${p.kind} ${pluralize(p.sourceCount, "item")}…`;
    case "running":
      return `${verb} to ${target}`;
    case "completed":
      return `${done} ${pluralize(p.doneFiles, "file")} to ${target}`;
    case "cancelled":
      return `Cancelled after ${pluralize(p.doneFiles, "file")}`;
  }
}

export function OperationsPanel({ operations, onCancel, onDismiss, onShowReport, onClose, rateOf }: OperationsPanelProps) {
  return (
    <aside className="nx-ops" aria-label="File operations">
      <header className="nx-ops-header">
        <h2>File operations</h2>
        <button type="button" className="nx-icon-button is-small" aria-label="Close panel" onClick={onClose}>
          <X size={14} />
        </button>
      </header>
      {operations.length === 0 && <p className="nx-ops-empty">No operations yet.</p>}
      <ul className="nx-ops-list">
        {operations.map((p) => {
          const running = p.state === "queued" || p.state === "scanning" || p.state === "running";
          const pct = percent(p.doneBytes, p.totalBytes);
          const problems = p.failureCount + p.skippedCount;
          const Icon = p.kind === "copy" ? Copy : MoveRight;
          const rate = rateOf(p);
          return (
            <li key={p.id} className="nx-op">
              <div className="nx-op-title">
                <Icon size={15} aria-hidden="true" />
                <span className="nx-ellipsis">{describe(p)}</span>
                {p.state === "completed" && problems === 0 && <CheckCircle2 size={15} className="nx-ok" aria-label="Completed" />}
                {p.state === "cancelled" && <CircleSlash size={15} aria-label="Cancelled" />}
              </div>
              {running && (
                <>
                  <div className="nx-progress" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
                    <span style={{ width: `${p.state === "running" ? pct : 0}%` }} />
                  </div>
                  <div className="nx-op-meta">
                    <span className="nx-ellipsis">{p.current ? baseName(p.current) : " "}</span>
                    <span>
                      {formatSize(p.doneBytes)} / {formatSize(p.totalBytes)}
                    </span>
                  </div>
                  <div className="nx-op-meta">
                    <span>{rate ? `${formatSize(Math.round(rate.bytesPerSecond))}/s` : " "}</span>
                    <span>{rate?.etaSeconds != null ? `${formatDuration(rate.etaSeconds)} left` : " "}</span>
                  </div>
                </>
              )}
              <div className="nx-op-actions">
                {problems > 0 && (
                  <button type="button" className="nx-link-button is-warning" onClick={() => onShowReport(p)}>
                    <TriangleAlert size={13} /> {p.failureCount > 0 && `${pluralize(p.failureCount, "error")}`}
                    {p.failureCount > 0 && p.skippedCount > 0 && ", "}
                    {p.skippedCount > 0 && `${p.skippedCount} skipped`}
                  </button>
                )}
                {!running && problems === 0 && p.state === "cancelled" && (
                  <button type="button" className="nx-link-button" onClick={() => onShowReport(p)}>
                    Show details
                  </button>
                )}
                <span className="nx-spacer" />
                {running ? (
                  <button type="button" className="nx-button is-small" onClick={() => onCancel(p.id)}>
                    Cancel
                  </button>
                ) : (
                  <button type="button" className="nx-link-button" onClick={() => onDismiss(p.id)}>
                    Dismiss
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
