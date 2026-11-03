import { useEffect, useState } from "react";
import { ExternalLink } from "lucide-react";
import { LINKS } from "../../config/links";
import { loadLicenseIndex, type LicenseIndex } from "./licenses";

interface Entry {
  name: string;
  /** Package name in the license index whose version is shown; none for toolchains. */
  pkg?: string;
  dev?: boolean;
  what: string;
}

const CORE: Entry[] = [
  { name: "Rust", what: "The language of Nexdir's backend: file operations, search, analysis and updates." },
  { name: "Tauri 2", pkg: "tauri", what: "Application framework that connects the Rust backend with the user interface." },
  { name: "Microsoft Edge WebView2", pkg: "Microsoft Edge WebView2 Loader", what: "Renders the user interface (the runtime is part of Windows)." },
  { name: "React", pkg: "react", what: "User interface library." },
  { name: "TypeScript", pkg: "typescript", dev: true, what: "Typed JavaScript for the user interface." },
  { name: "SQLite", pkg: "SQLite", what: "Database of the optional search index." },
];

const LIBRARIES: Entry[] = [
  { name: "Lucide", pkg: "lucide-react", what: "Icons." },
  { name: "TanStack Virtual", pkg: "@tanstack/react-virtual", what: "Fast lists for folders with many files." },
  { name: "BLAKE3", pkg: "blake3", what: "Content hashing for duplicate detection." },
  { name: "rusqlite", pkg: "rusqlite", what: "SQLite access from Rust." },
  { name: "windows-rs", pkg: "windows", what: "Windows API bindings (Recycle Bin, clipboard, drives)." },
  { name: "Serde", pkg: "serde", what: "Data exchange between backend and interface." },
  { name: "Tauri Updater", pkg: "tauri-plugin-updater", what: "Downloads updates and verifies their signatures." },
  { name: "Tauri Window State", pkg: "tauri-plugin-window-state", what: "Remembers the window size and position." },
  { name: "reqwest & rustls", pkg: "reqwest", what: "Secure HTTPS connection for update checks." },
];

const TOOLS: Entry[] = [
  { name: "Cargo and rustc", what: "Rust compiler and build system." },
  { name: "Tauri CLI", pkg: "@tauri-apps/cli", dev: true, what: "Builds the installers and signs updates." },
  { name: "Vite", pkg: "vite", dev: true, what: "Builds the user interface." },
  { name: "Vitest", pkg: "vitest", dev: true, what: "Runs the interface tests." },
  { name: "Testing Library", pkg: "@testing-library/react", dev: true, what: "Interface tests the way users interact." },
  { name: "jsdom", pkg: "jsdom", dev: true, what: "Browser environment for tests." },
  { name: "NSIS and WiX", what: "Windows installer technologies used by the Tauri bundler." },
];

function version(index: LicenseIndex | null, e: Entry): string | null {
  if (!index || !e.pkg) return null;
  const list = e.dev ? index.devTools : index.components;
  return list.find((c) => c.name === e.pkg)?.version ?? null;
}

interface Props {
  discordInvite: string | null;
  openUrl: (url: string) => void;
}

export function CreditsView({ discordInvite, openUrl }: Props) {
  const [index, setIndex] = useState<LicenseIndex | null>(null);
  useEffect(() => {
    loadLicenseIndex()
      .then(setIndex)
      .catch(() => {});
  }, []);

  const section = (title: string, entries: Entry[]) => (
    <section className="nx-credits-section" aria-label={title}>
      <h3>{title}</h3>
      <ul className="nx-credits-list">
        {entries.map((e) => {
          const v = version(index, e);
          return (
            <li key={e.name}>
              <strong>{e.name}</strong>
              {v && <span className="nx-muted"> {v}</span>}
              <span> — {e.what}</span>
            </li>
          );
        })}
      </ul>
    </section>
  );

  return (
    <div className="nx-credits">
      {section("Core Technologies", CORE)}
      {section("Libraries", LIBRARIES)}
      <p className="nx-muted">The complete list with licenses is under Open Source Licenses.</p>
      {section("Development Tools", TOOLS)}
      <section className="nx-credits-section" aria-label="Community">
        <h3>Community</h3>
        <p>
          Nexdir is developed by Kervent as an open-source project. Everyone who contributes code, reports or ideas is listed by GitHub — no names are added here
          by hand.
        </p>
        <div className="nx-inline-actions">
          <button type="button" className="nx-button is-small" onClick={() => openUrl(LINKS.repository)}>
            <ExternalLink size={14} /> GitHub Repository
          </button>
          <button type="button" className="nx-button is-small" onClick={() => openUrl(LINKS.contributors)}>
            <ExternalLink size={14} /> Contributors
          </button>
          <button type="button" className="nx-button is-small" onClick={() => openUrl(LINKS.contributing)}>
            <ExternalLink size={14} /> How to Contribute
          </button>
          {discordInvite && (
            <button type="button" className="nx-button is-small" onClick={() => openUrl(discordInvite)}>
              <ExternalLink size={14} /> Discord Community
            </button>
          )}
        </div>
        <p className="nx-muted">Bug reports, feature ideas, translations and code are welcome — see the contributing guide.</p>
      </section>
    </div>
  );
}
