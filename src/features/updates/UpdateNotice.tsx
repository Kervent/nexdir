import { ArrowUpCircle, X } from "lucide-react";
import type { UpdateOffer } from "./types";

interface Props {
  offer: UpdateOffer;
  currentVersion: string;
  onViewChanges: () => void;
  onDownload: () => void;
  onRemindLater: () => void;
  onDismiss: () => void;
}

/** Small, non-modal notice after an automatic check found an update. Never takes focus. */
export function UpdateNotice({ offer, currentVersion, onViewChanges, onDownload, onRemindLater, onDismiss }: Props) {
  return (
    <aside className="nx-update-notice" role="status" aria-live="polite" aria-label="Update available">
      <ArrowUpCircle size={20} aria-hidden="true" className="nx-update-notice-icon" />
      <div className="nx-update-notice-text">
        <strong>Nexdir {offer.version} is available</strong>
        <span>
          Installed version: {currentVersion} · Available version: {offer.version}
          {offer.prerelease ? " (preview)" : ""}
        </span>
        <div className="nx-update-notice-actions">
          <button type="button" className="nx-button is-small" onClick={onViewChanges}>
            View Changes
          </button>
          <button type="button" className="nx-button is-small is-primary" onClick={onDownload}>
            Download Update
          </button>
          <button type="button" className="nx-button is-small" onClick={onRemindLater}>
            Remind Me Later
          </button>
        </div>
      </div>
      <button type="button" className="nx-icon-button is-small" aria-label="Dismiss" onClick={onDismiss}>
        <X size={14} />
      </button>
    </aside>
  );
}
