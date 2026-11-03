import { useId, useState } from "react";
import { RefreshCw } from "lucide-react";
import { Dialog } from "../../components/Dialog";
import logo from "../../../assets/branding/png/nexdir-logo-128.png";
import { LicensesView } from "../about/LicensesView";
import { CreditsView } from "../about/CreditsView";
import { CHANNEL_LABEL, INSTALL_LABEL } from "../updates/UpdateDialog";
import type { UpdateInfo } from "../updates/types";

export type AboutSection = "about" | "licenses" | "credits";

const SECTIONS: { id: AboutSection; label: string }[] = [
  { id: "about", label: "About" },
  { id: "licenses", label: "Open Source Licenses" },
  { id: "credits", label: "Technologies & Credits" },
];

interface Props {
  onClose: () => void;
  updateInfo: UpdateInfo | null;
  onCheckForUpdates: () => void;
  openUrl: (url: string) => void;
  openProjectUrl: (url: string) => Promise<unknown>;
  copyText: (text: string) => Promise<void>;
  notify: (tone: "success" | "error", message: string) => void;
  discordInvite: string | null;
  section?: AboutSection;
}

/** Version and project facts — the version is what bug reports ask for. */
export function AboutDialog({ onClose, updateInfo, onCheckForUpdates, openUrl, openProjectUrl, copyText, notify, discordInvite, section = "about" }: Props) {
  const [tab, setTab] = useState<AboutSection>(section);
  const baseId = useId();

  return (
    <Dialog
      title="About Nexdir"
      onClose={onClose}
      className={tab === "about" ? "is-wide" : "is-wide is-tall"}
      initialFocus=".nx-about-tabs [aria-selected='true']"
      footer={
        <button type="button" className="nx-button is-primary" onClick={onClose}>
          Close
        </button>
      }
    >
      <div className="nx-about-tabs" role="tablist" aria-label="About Nexdir">
        {SECTIONS.map((s, i) => (
          <button
            key={s.id}
            type="button"
            role="tab"
            id={`${baseId}-${s.id}`}
            aria-selected={tab === s.id}
            aria-controls={`${baseId}-panel`}
            tabIndex={tab === s.id ? 0 : -1}
            className={`nx-about-tab${tab === s.id ? " is-active" : ""}`}
            onClick={() => setTab(s.id)}
            onKeyDown={(e) => {
              const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
              if (!step) return;
              e.preventDefault();
              const next = SECTIONS[(i + step + SECTIONS.length) % SECTIONS.length]!;
              setTab(next.id);
              window.setTimeout(() => document.getElementById(`${baseId}-${next.id}`)?.focus(), 0);
            }}
          >
            {s.label}
          </button>
        ))}
      </div>
      <div role="tabpanel" id={`${baseId}-panel`} aria-labelledby={`${baseId}-${tab}`} className="nx-about-panel">
        {tab === "about" && (
          <>
            <div className="nx-about">
              <img src={logo} width={64} height={64} alt="" />
              <div>
                <p className="nx-about-name">Nexdir</p>
                <p className="nx-about-version">Version {__APP_VERSION__}</p>
                <p>A smarter way to manage your files.</p>
              </div>
            </div>
            <dl className="nx-update-facts is-compact">
              <div>
                <dt>Release channel</dt>
                <dd>{updateInfo ? CHANNEL_LABEL[updateInfo.channel] : "—"}</dd>
              </div>
              <div>
                <dt>Installation</dt>
                <dd>{updateInfo ? INSTALL_LABEL[updateInfo.installKind] : "—"}</dd>
              </div>
              <div>
                <dt>Build</dt>
                <dd>{__APP_COMMIT__ ? `${__APP_COMMIT__} (${__APP_COMMIT_DATE__})` : "Development build"} · Windows x64</dd>
              </div>
            </dl>
            <div className="nx-inline-actions">
              <button type="button" className="nx-button" onClick={onCheckForUpdates}>
                <RefreshCw size={15} /> Check for Updates
              </button>
              <button type="button" className="nx-button" onClick={() => setTab("licenses")}>
                Open Source Licenses
              </button>
              <button type="button" className="nx-button" onClick={() => setTab("credits")}>
                Technologies & Credits
              </button>
            </div>
            <ul className="nx-about-facts">
              <li>Open source under the MIT License. Developed by Kervent.</li>
              <li>No telemetry and no accounts. Nexdir only connects to GitHub when you check for updates (or turn on automatic checks) and when you open a link. File names and paths are never sent.</li>
              <li>Nexdir never deletes files permanently — removed items go to the Recycle Bin.</li>
              <li>Report bugs at github.com/Kervent/nexdir/issues and include the version above.</li>
            </ul>
          </>
        )}
        {tab === "licenses" && <LicensesView openProjectUrl={openProjectUrl} copyText={copyText} notify={notify} />}
        {tab === "credits" && <CreditsView discordInvite={discordInvite} openUrl={openUrl} />}
      </div>
    </Dialog>
  );
}
