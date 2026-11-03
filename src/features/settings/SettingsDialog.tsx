import { useEffect, useId, useState } from "react";
import { ExternalLink, RefreshCw, ShieldCheck } from "lucide-react";
import { Dialog } from "../../components/Dialog";
import { formatDate } from "../../lib/format";
import { CHANNEL_LABEL, INSTALL_LABEL } from "../updates/UpdateDialog";
import type { Updates } from "../updates/useUpdates";
import type { Channel } from "../updates/types";
import type { Preferences, ThemePreference } from "./preferences";

export type SettingsSection = "general" | "updates";

interface Props {
  prefs: Preferences;
  updatePrefs: (patch: Partial<Preferences>) => void;
  updates: Updates;
  section?: SettingsSection;
  onCheckForUpdates: () => void;
  onOpenUrl: (url: string) => void;
  onClose: () => void;
}

const INTERVALS = [
  { hours: 24, label: "Once a day" },
  { hours: 24 * 7, label: "Once a week" },
];

export function SettingsDialog({ prefs, updatePrefs, updates, section = "general", onCheckForUpdates, onOpenUrl, onClose }: Props) {
  const [tab, setTab] = useState<SettingsSection>(section);
  const ids = { general: useId(), updates: useId() };

  const tabButton = (id: SettingsSection, label: string) => (
    <button
      type="button"
      role="tab"
      id={`${ids[id]}-tab`}
      aria-selected={tab === id}
      aria-controls={ids[id]}
      tabIndex={tab === id ? 0 : -1}
      className={`nx-settings-tab${tab === id ? " is-active" : ""}`}
      onClick={() => setTab(id)}
      onKeyDown={(e) => {
        if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "ArrowRight" || e.key === "ArrowLeft") {
          e.preventDefault();
          const next: SettingsSection = tab === "general" ? "updates" : "general";
          setTab(next);
          window.setTimeout(() => document.getElementById(`${ids[next]}-tab`)?.focus(), 0);
        }
      }}
    >
      {label}
    </button>
  );

  return (
    <Dialog
      title="Settings"
      onClose={onClose}
      className="is-wide"
      initialFocus=".nx-settings-tab.is-active"
      footer={
        <button type="button" className="nx-button is-primary" onClick={onClose}>
          Done
        </button>
      }
    >
      <div className="nx-settings">
        <div className="nx-settings-nav" role="tablist" aria-label="Settings sections" aria-orientation="vertical">
          {tabButton("general", "General")}
          {tabButton("updates", "Updates")}
        </div>
        <div className="nx-settings-panel" role="tabpanel" id={ids[tab]} aria-labelledby={`${ids[tab]}-tab`}>
          {tab === "general" ? <GeneralSettings prefs={prefs} updatePrefs={updatePrefs} /> : <UpdateSettings updates={updates} onCheck={onCheckForUpdates} onOpenUrl={onOpenUrl} />}
        </div>
      </div>
    </Dialog>
  );
}

function GeneralSettings({ prefs, updatePrefs }: { prefs: Preferences; updatePrefs: (patch: Partial<Preferences>) => void }) {
  const themeId = useId();
  return (
    <>
      <h3 className="nx-settings-heading">Appearance</h3>
      <div className="nx-field">
        <label htmlFor={themeId}>Theme</label>
        <select id={themeId} className="nx-input" value={prefs.theme} onChange={(e) => updatePrefs({ theme: e.target.value as ThemePreference })}>
          <option value="system">Same as Windows</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </div>
      <label className="nx-check">
        <input type="checkbox" checked={prefs.viewMode === "icons"} onChange={(e) => updatePrefs({ viewMode: e.target.checked ? "icons" : "details" })} />
        Show folders as icons (instead of details)
      </label>
      <label className="nx-check">
        <input type="checkbox" checked={prefs.showHidden} onChange={(e) => updatePrefs({ showHidden: e.target.checked })} />
        Show hidden files and folders
      </label>
      <label className="nx-check">
        <input type="checkbox" checked={prefs.showPreview} onChange={(e) => updatePrefs({ showPreview: e.target.checked })} />
        Show the preview pane
      </label>
    </>
  );
}

function UpdateSettings({ updates, onCheck, onOpenUrl }: { updates: Updates; onCheck: () => void; onOpenUrl: (url: string) => void }) {
  const { info } = updates;
  const channelId = useId();
  const intervalId = useId();
  const [error, setError] = useState<string | null>(null);
  const [pendingChannel, setPendingChannel] = useState<Channel | null>(null);

  useEffect(() => {
    if (!info) void updates.refresh().catch(() => {});
  }, [info, updates]);

  if (!info) return <p className="nx-muted">Loading…</p>;

  const save = (patch: Partial<{ autoCheck: boolean; channel: Channel; notify: boolean; intervalHours: number }>) => {
    setError(null);
    updates
      .saveSettings({ autoCheck: info.autoCheck, channel: info.channel, notify: info.notify, intervalHours: info.intervalHours, ...patch })
      .catch((e: unknown) => setError(e && typeof e === "object" && "message" in e ? String((e as { message: unknown }).message) : String(e)));
  };

  const lastChecked = info.lastSuccess ? formatDate(info.lastSuccess) : "Never";

  return (
    <>
      <h3 className="nx-settings-heading">Update Settings</h3>
      <label className="nx-check">
        <input type="checkbox" checked={info.autoCheck} onChange={(e) => save({ autoCheck: e.target.checked })} />
        Automatically check for updates
      </label>
      <div className="nx-field">
        <label htmlFor={intervalId}>How often</label>
        <select id={intervalId} className="nx-input" disabled={!info.autoCheck} value={info.intervalHours} onChange={(e) => save({ intervalHours: Number(e.target.value) })}>
          {INTERVALS.some((i) => i.hours === info.intervalHours) ? null : <option value={info.intervalHours}>Every {info.intervalHours} hours</option>}
          {INTERVALS.map((i) => (
            <option key={i.hours} value={i.hours}>
              {i.label}
            </option>
          ))}
        </select>
      </div>
      <label className="nx-check">
        <input type="checkbox" checked={info.notify} disabled={!info.autoCheck} onChange={(e) => save({ notify: e.target.checked })} />
        Notify me when an update is available
      </label>
      <div className="nx-field">
        <label htmlFor={channelId}>Release channel</label>
        <select
          id={channelId}
          className="nx-input"
          value={pendingChannel ?? info.channel}
          onChange={(e) => {
            const channel = e.target.value as Channel;
            if (channel === "preview" && info.channel !== "preview") setPendingChannel(channel);
            else {
              setPendingChannel(null);
              save({ channel });
            }
          }}
        >
          <option value="stable">{CHANNEL_LABEL.stable}</option>
          <option value="preview">{CHANNEL_LABEL.preview}</option>
        </select>
        <span className="nx-muted">
          {info.channel === "stable"
            ? "Only finished releases. Nexdir has no stable release yet, so this channel offers nothing until version 1.0."
            : "Alpha and beta versions, and stable versions when they are newer. Experimental: expect bugs."}{" "}
          Switching channels never installs an older version.
        </span>
      </div>
      {pendingChannel === "preview" && (
        <div className="nx-notice" role="alert">
          <div>
            <p>Preview versions are experimental and may contain bugs that affect your work. Keep backups of important files.</p>
            <div className="nx-inline-actions">
              <button type="button" className="nx-button is-small" onClick={() => setPendingChannel(null)}>
                Keep {CHANNEL_LABEL[info.channel]}
              </button>
              <button
                type="button"
                className="nx-button is-small is-primary"
                onClick={() => {
                  setPendingChannel(null);
                  save({ channel: "preview" });
                }}
              >
                Use Beta (Preview)
              </button>
            </div>
          </div>
        </div>
      )}

      <dl className="nx-update-facts is-compact">
        <div>
          <dt>Installed version</dt>
          <dd>{info.currentVersion}</dd>
        </div>
        <div>
          <dt>Installation type</dt>
          <dd>{INSTALL_LABEL[info.installKind]}</dd>
        </div>
        <div>
          <dt>Last checked</dt>
          <dd>{lastChecked}</dd>
        </div>
      </dl>
      {error && (
        <p className="nx-notice is-danger" role="alert">
          {error}
        </p>
      )}
      <div className="nx-inline-actions">
        <button type="button" className="nx-button is-primary" onClick={onCheck}>
          <RefreshCw size={15} /> Check for Updates
        </button>
        <button type="button" className="nx-button" onClick={() => void updates.openRelease()}>
          <ExternalLink size={15} /> Open Release Notes
        </button>
        <button type="button" className="nx-button" onClick={() => onOpenUrl(info.releasesUrl)}>
          <ExternalLink size={15} /> View All Releases on GitHub
        </button>
      </div>

      <details className="nx-help-topic">
        <summary>
          <ShieldCheck size={15} aria-hidden="true" /> Update security and privacy
        </summary>
        <div className="nx-help-topic-body">
          <ul>
            <li>Nexdir only contacts GitHub when you click Check for Updates or turn on automatic checks. With automatic checks off, Nexdir makes no update requests.</li>
            <li>A check downloads one small file from github.com (raw.githubusercontent.com). Like every web request it reveals your IP address and that a Nexdir version checks for updates. No file names, folders, paths, account data or device IDs are sent, and there is no telemetry.</li>
            <li>Updates are downloaded only after you click Download Update and installed only after you confirm. Every update is signed; Nexdir checks the signature and the signed version number before installing and discards anything that does not match.</li>
            <li>Nexdir never installs an older version, also not when you switch channels.</li>
            <li>The portable version is never changed automatically: it only tells you about new versions.</li>
            <li>The update signature is not the same as Windows code signing: Windows may still show a SmartScreen warning for the installer.</li>
          </ul>
        </div>
      </details>
    </>
  );
}
