import { Pin } from "lucide-react";
import { formatSize, percent } from "../../lib/format";
import type { DriveInfo, KnownFolder } from "../../types/fs";
import type { PinnedFolder } from "../settings/preferences";
import { DRIVE_ICONS, KNOWN_ICONS, TOOL_ICONS } from "../navigation/Sidebar";
import { TOOL_LABELS, type Location, type ToolId } from "../workspace/workspace";

const TOOL_DESCRIPTIONS: Record<ToolId, string> = {
  search: "Find files by name, type, size or date",
  storage: "See what uses your disk space",
  duplicates: "Find identical files",
  organize: "Sort files with rules",
};

interface HomeViewProps {
  knownFolders: KnownFolder[];
  drives: DriveInfo[];
  pinned: PinnedFolder[];
  onNavigate: (to: Location) => void;
}

/** Start page: quick access folders, pinned folders and drives with usage. */
export function HomeView({ knownFolders, drives, pinned, onNavigate }: HomeViewProps) {
  const go = (path: string) => onNavigate({ type: "folder", path });
  return (
    <div className="nx-home" tabIndex={-1}>
      <section aria-labelledby="home-quick" hidden={knownFolders.length + pinned.length === 0}>
        <h2 id="home-quick" className="nx-home-heading">Quick access</h2>
        <div className="nx-card-grid">
          {knownFolders.map((f) => {
            const Icon = KNOWN_ICONS[f.id];
            return (
              <button key={f.id} type="button" className="nx-card" onClick={() => go(f.path)} title={f.path}>
                <Icon size={22} className="nx-card-icon" aria-hidden="true" />
                <span className="nx-card-text">
                  <span className="nx-card-title">{f.label}</span>
                  <span className="nx-card-sub nx-ellipsis">{f.path}</span>
                </span>
              </button>
            );
          })}
          {pinned.map((p) => (
            <button key={p.path} type="button" className="nx-card" onClick={() => go(p.path)} title={p.path}>
              <Pin size={22} className="nx-card-icon" aria-hidden="true" />
              <span className="nx-card-text">
                <span className="nx-card-title">{p.label}</span>
                <span className="nx-card-sub nx-ellipsis">{p.path}</span>
              </span>
            </button>
          ))}
        </div>
      </section>

      <section aria-labelledby="home-tools">
        <h2 id="home-tools" className="nx-home-heading">Tools</h2>
        <div className="nx-card-grid">
          {(Object.keys(TOOL_DESCRIPTIONS) as ToolId[]).map((tool) => {
            const Icon = TOOL_ICONS[tool];
            return (
              <button key={tool} type="button" className="nx-card" onClick={() => onNavigate({ type: "tool", tool })}>
                <Icon size={22} className="nx-card-icon" aria-hidden="true" />
                <span className="nx-card-text">
                  <span className="nx-card-title">{TOOL_LABELS[tool]}</span>
                  <span className="nx-card-sub">{TOOL_DESCRIPTIONS[tool]}</span>
                </span>
              </button>
            );
          })}
        </div>
      </section>

      <section aria-labelledby="home-drives">
        <h2 id="home-drives" className="nx-home-heading">Drives</h2>
        <div className="nx-card-grid is-drives">
          {drives.map((d) => {
            const Icon = DRIVE_ICONS[d.kind];
            const used = d.totalBytes != null && d.freeBytes != null ? d.totalBytes - d.freeBytes : null;
            const usage = used != null && d.totalBytes ? percent(used, d.totalBytes) : 0;
            return (
              <button key={d.path} type="button" className={`nx-card is-drive${d.available ? "" : " is-unavailable"}`} onClick={() => go(d.path)}>
                <Icon size={26} className="nx-card-icon" aria-hidden="true" />
                <span className="nx-card-text">
                  <span className="nx-card-title">
                    {d.label} ({d.letter}:)
                  </span>
                  {d.available && d.totalBytes != null ? (
                    <>
                      <span className={`nx-meter${usage >= 90 ? " is-critical" : ""}`} role="meter" aria-valuenow={usage} aria-valuemin={0} aria-valuemax={100} aria-label={`${usage}% used`}>
                        <span style={{ width: `${usage}%` }} />
                      </span>
                      <span className="nx-card-sub">
                        {formatSize(d.freeBytes)} free of {formatSize(d.totalBytes)}
                        {d.fileSystem ? ` · ${d.fileSystem}` : ""}
                      </span>
                    </>
                  ) : (
                    <span className="nx-card-sub">Not available</span>
                  )}
                </span>
              </button>
            );
          })}
        </div>
      </section>
    </div>
  );
}
