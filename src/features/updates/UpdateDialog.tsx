import { useState } from "react";
import { AlertCircle, CheckCircle2, Download, ExternalLink, RefreshCw, ShieldCheck } from "lucide-react";
import { Dialog } from "../../components/Dialog";
import { formatSize, percent } from "../../lib/format";
import { ReleaseNotes } from "./ReleaseNotes";
import type { Updates } from "./useUpdates";
import type { Channel, InstallKind, UpdateError, UpdateSnapshot } from "./types";

export const CHANNEL_LABEL: Record<Channel, string> = { stable: "Stable", preview: "Beta (Preview)" };
export const INSTALL_LABEL: Record<InstallKind, string> = { nsis: "Installer (per user)", msi: "MSI package", portable: "Portable" };

export function formatReleaseDate(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "—" : d.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

/** Status line of the update package's signature, in plain words. */
export function signatureStatus(state: UpdateSnapshot, canInstall: boolean, kind: InstallKind): string {
  if (kind === "portable") return "Not checked by Nexdir — compare the SHA-256 checksum on the release page.";
  if (!canInstall) return "This build has no update key, so it cannot verify or install updates.";
  if (state.phase === "readyToInstall" || state.phase === "installing") return "Verified: signed by the Nexdir release key.";
  if (state.error?.code === "signature") return "Invalid — the download was discarded.";
  return "Checked after the download, before anything is installed.";
}

interface Props {
  updates: Updates;
  onClose: () => void;
  onOpenSettings: () => void;
}

export function UpdateDialog({ updates, onClose, onOpenSettings }: Props) {
  const { info, state, speed } = updates;
  const [confirming, setConfirming] = useState(false);
  const [actionError, setActionError] = useState<UpdateError | null>(null);

  if (!info || !state) {
    return (
      <Dialog title="Check for Updates" onClose={onClose} footer={<button type="button" className="nx-button" onClick={onClose}>Close</button>}>
        <p className="nx-muted">Loading…</p>
      </Dialog>
    );
  }

  const offer = state.offer;
  const portable = info.installKind === "portable";
  const act = async (f: () => Promise<UpdateError | null>) => {
    setActionError(null);
    const error = await f();
    if (error) setActionError(error);
  };
  const error = actionError ?? state.error;
  const busy = state.phase === "checking" || state.phase === "downloading" || state.phase === "installing";

  const close = () => {
    // Closing the window never cancels a running download; it continues in the background.
    onClose();
  };

  const footer = (() => {
    const closeButton = (
      <button type="button" className="nx-button" onClick={close}>
        {state.phase === "downloading" ? "Hide" : "Close"}
      </button>
    );
    switch (state.phase) {
      case "available":
        return (
          <>
            {offer && (
              <button type="button" className="nx-button" onClick={() => void act(updates.openRelease)}>
                <ExternalLink size={15} /> View Changes
              </button>
            )}
            {offer && (
              <button
                type="button"
                className="nx-button"
                onClick={() => {
                  updates.snooze(offer.version);
                  onClose();
                }}
              >
                Remind Me Later
              </button>
            )}
            {info.canInstall ? (
              <button type="button" className="nx-button is-primary" onClick={() => void act(updates.download)}>
                <Download size={15} /> Download Update
              </button>
            ) : (
              <button type="button" className="nx-button is-primary" onClick={() => void act(updates.openRelease)}>
                <ExternalLink size={15} /> Open Download Page
              </button>
            )}
          </>
        );
      case "downloading":
        return (
          <>
            <button type="button" className="nx-button" onClick={() => void act(updates.cancel)}>
              Cancel Download
            </button>
            {closeButton}
          </>
        );
      case "readyToInstall":
        return confirming ? (
          <>
            <button type="button" className="nx-button" onClick={() => setConfirming(false)}>
              Not Now
            </button>
            <button
              type="button"
              className="nx-button is-primary"
              onClick={() =>
                void act(async () => {
                  const e = await updates.install();
                  setConfirming(false);
                  return e;
                })
              }
            >
              Close Nexdir and Install
            </button>
          </>
        ) : (
          <>
            <button type="button" className="nx-button" onClick={() => void act(updates.cancel)}>
              Discard Download
            </button>
            {closeButton}
            <button type="button" className="nx-button is-primary" onClick={() => setConfirming(true)}>
              Install Update…
            </button>
          </>
        );
      case "installing":
        return closeButton;
      default:
        return (
          <>
            {closeButton}
            <button type="button" className="nx-button is-primary" disabled={busy} onClick={() => void act(updates.check)}>
              <RefreshCw size={15} /> {state.phase === "idle" ? "Check for Updates" : "Check Again"}
            </button>
          </>
        );
    }
  })();

  const total = state.total ?? offer?.size ?? null;
  const pct = total ? percent(state.downloaded, total) : null;

  return (
    <Dialog title={offer ? `Nexdir ${offer.version} is available` : "Check for Updates"} onClose={close} className="is-wide" footer={footer} initialFocus=".nx-dialog-footer .is-primary, .nx-dialog-footer button">
      <dl className="nx-update-facts">
        <div>
          <dt>Installed version</dt>
          <dd>{info.currentVersion}</dd>
        </div>
        <div>
          <dt>Available version</dt>
          <dd>{offer ? offer.version : state.latest && state.phase === "upToDate" ? `${state.latest} (not newer)` : "—"}</dd>
        </div>
        <div>
          <dt>Released</dt>
          <dd>{formatReleaseDate(offer?.pubDate)}</dd>
        </div>
        <div>
          <dt>Release channel</dt>
          <dd>
            {CHANNEL_LABEL[state.channel]}{" "}
            <button type="button" className="nx-link-button" onClick={onOpenSettings}>
              Change
            </button>
          </dd>
        </div>
        <div>
          <dt>Download size</dt>
          <dd>{offer?.size ? formatSize(offer.size) : "—"}</dd>
        </div>
        <div>
          <dt>Signature</dt>
          <dd>{offer ? signatureStatus(state, info.canInstall, info.installKind) : "—"}</dd>
        </div>
      </dl>

      <div className="nx-update-status" role="status" aria-live="polite">
        {state.phase === "idle" && <p>Nexdir checks GitHub for a newer version in the {CHANNEL_LABEL[state.channel]} channel. Nothing is downloaded until you choose to.</p>}
        {state.phase === "checking" && (
          <>
            <p>Checking GitHub for updates…</p>
            <span className="nx-progress is-indeterminate" aria-hidden="true">
              <span style={{ width: "30%" }} />
            </span>
          </>
        )}
        {state.phase === "upToDate" && (
          <p className="nx-update-ok">
            <CheckCircle2 size={18} aria-hidden="true" />
            {state.noRelease
              ? `There is no ${CHANNEL_LABEL[state.channel]} release yet. Nexdir is currently in alpha; choose the Beta (Preview) channel to receive alpha and beta versions.`
              : `Nexdir ${info.currentVersion} is up to date.`}
          </p>
        )}
        {state.phase === "available" && offer && (
          <>
            {offer.prerelease && (
              <p className="nx-notice">
                <AlertCircle size={16} aria-hidden="true" />
                This is a preview (alpha/beta) version. It may contain bugs — keep backups of important files.
              </p>
            )}
            {portable && <p>The portable version is not updated automatically. Download the new ZIP from GitHub and replace the old Nexdir.exe; your settings are kept.</p>}
          </>
        )}
        {state.phase === "downloading" && (
          <>
            <p>
              Downloading… {formatSize(state.downloaded)}
              {total ? ` of ${formatSize(total)}` : ""}
              {speed ? ` · ${formatSize(speed)}/s` : ""}
            </p>
            <span
              className={`nx-progress${pct === null ? " is-indeterminate" : ""}`}
              role="progressbar"
              aria-label="Download progress"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={pct ?? undefined}
            >
              <span style={{ width: `${pct ?? 30}%` }} />
            </span>
          </>
        )}
        {state.phase === "readyToInstall" && (
          <>
            <p className="nx-update-ok">
              <ShieldCheck size={18} aria-hidden="true" />
              Download complete. The signature was verified — ready to install.
            </p>
            <p className="nx-muted">
              {confirming
                ? "Nexdir closes now, the installer updates it, and Nexdir starts again. Your settings, pinned folders and files are not changed."
                : "Installing closes Nexdir and starts it again afterwards. Install when no copy or move is running."}
              {info.installKind === "msi" ? " Windows may ask for administrator permission because the MSI package was installed for all users." : ""}
            </p>
          </>
        )}
        {state.phase === "installing" && <p>Starting the installer… Nexdir will close and restart when the update is installed.</p>}
        {error && (state.phase === "error" || actionError) && (
          <p className="nx-notice is-danger" role="alert">
            <AlertCircle size={16} aria-hidden="true" />
            {error.message}
          </p>
        )}
      </div>

      {offer && (state.phase === "available" || state.phase === "downloading" || state.phase === "readyToInstall") && (
        <section className="nx-update-notes" aria-label="Release notes">
          <h3>What's new</h3>
          <ReleaseNotes text={offer.notes} />
        </section>
      )}
    </Dialog>
  );
}
