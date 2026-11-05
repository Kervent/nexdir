import {
  Disc3,
  Download,
  FileText,
  Film,
  HardDrive,
  House,
  Image,
  Monitor,
  Music,
  Network,
  Server,
  ChartPie,
  Files,
  Pin,
  PinOff,
  Search,
  WandSparkles,
  Usb,
  User,
  type LucideIcon,
} from "lucide-react";
import type { DriveInfo, DriveKind, KnownFolder, KnownFolderId } from "../../types/fs";
import type { PinnedFolder } from "../settings/preferences";
import { sameLocation, TOOL_LABELS, type Location, type ToolId } from "../workspace/workspace";
import { useDropTarget, type DropEffect } from "../dnd/dnd";
import type { NetworkFavorite } from "../../types/network";

export const KNOWN_ICONS: Record<KnownFolderId, LucideIcon> = {
  home: User,
  desktop: Monitor,
  downloads: Download,
  documents: FileText,
  pictures: Image,
  music: Music,
  videos: Film,
};

export const TOOL_ICONS: Record<ToolId, LucideIcon> = {
  search: Search,
  storage: ChartPie,
  duplicates: Files,
  organize: WandSparkles,
};

export const DRIVE_ICONS: Record<DriveKind, LucideIcon> = {
  fixed: HardDrive,
  removable: Usb,
  network: Network,
  optical: Disc3,
  ram: HardDrive,
  unknown: HardDrive,
};

interface SidebarProps {
  location: Location;
  knownFolders: KnownFolder[];
  drives: DriveInfo[];
  pinned: PinnedFolder[];
  /** Saved network locations; shown without contacting the servers. */
  networkFavorites: NetworkFavorite[];
  onNavigate: (to: Location) => void;
  onUnpin: (path: string) => void;
  onOpenInNewTab: (to: Location) => void;
  onDropPaths: (paths: string[], effect: DropEffect, target: string) => void;
}

function Item({ icon: Icon, label, active, onClick, title, action, path, onMiddleClick, onDropPaths }: {
  icon: LucideIcon;
  label: string;
  active: boolean;
  onClick: () => void;
  title?: string;
  action?: React.ReactNode;
  /** Folder represented by the item; makes it a drop target. */
  path?: string;
  onMiddleClick?: () => void;
  onDropPaths?: (paths: string[], effect: DropEffect, target: string) => void;
}) {
  const { dropProps, isOver } = useDropTarget(path && onDropPaths ? path : null, (p, e, t) => onDropPaths?.(p, e, t));
  return (
    <li className="nx-side-row">
      <button
        type="button"
        className={`nx-side-item${active ? " is-active" : ""}${isOver ? " is-drop-target" : ""}`}
        aria-current={active ? "page" : undefined}
        onClick={onClick}
        onMouseDown={(e) => {
          if (e.button === 1 && onMiddleClick) {
            e.preventDefault();
            onMiddleClick();
          }
        }}
        title={title ?? label}
        {...dropProps}
      >
        <Icon size={16} aria-hidden="true" />
        <span className="nx-ellipsis">{label}</span>
      </button>
      {action}
    </li>
  );
}

export function Sidebar({ location, knownFolders, drives, pinned, networkFavorites, onNavigate, onUnpin, onOpenInNewTab, onDropPaths }: SidebarProps) {
  const folderProps = (path: string) => ({
    path,
    onDropPaths,
    onMiddleClick: () => onOpenInNewTab({ type: "folder", path }),
  });
  const isAt = (path: string) => sameLocation(location, { type: "folder", path });
  const quick = knownFolders.filter((f) => f.id !== "home");

  return (
    <nav className="nx-sidebar" aria-label="Navigation" data-tour="sidebar">
      <ul className="nx-side-list">
        <Item icon={House} label="Home" active={location.type === "home"} onClick={() => onNavigate({ type: "home" })} />
      </ul>

      <h3 className="nx-side-heading">Quick access</h3>
      <ul className="nx-side-list">
        {quick.map((f) => (
          <Item key={f.id} icon={KNOWN_ICONS[f.id]} label={f.label} title={f.path} active={isAt(f.path)} onClick={() => onNavigate({ type: "folder", path: f.path })} {...folderProps(f.path)} />
        ))}
      </ul>

      {pinned.length > 0 && (
        <>
          <h3 className="nx-side-heading">Pinned</h3>
          <ul className="nx-side-list">
            {pinned.map((p) => (
              <Item
                key={p.path}
                icon={Pin}
                label={p.label}
                title={p.path}
                active={isAt(p.path)}
                onClick={() => onNavigate({ type: "folder", path: p.path })}
                {...folderProps(p.path)}
                action={
                  <button type="button" className="nx-icon-button is-small nx-side-action" aria-label={`Unpin ${p.label}`} title="Unpin" onClick={() => onUnpin(p.path)}>
                    <PinOff size={13} />
                  </button>
                }
              />
            ))}
          </ul>
        </>
      )}

      <h3 className="nx-side-heading" data-tour="tools">Tools</h3>
      <ul className="nx-side-list" data-tour="tools">
        {(Object.keys(TOOL_ICONS) as ToolId[]).map((tool) => (
          <Item
            key={tool}
            icon={TOOL_ICONS[tool]}
            label={TOOL_LABELS[tool]}
            active={location.type === "tool" && location.tool === tool}
            onClick={() => onNavigate({ type: "tool", tool })}
            onMiddleClick={() => onOpenInNewTab({ type: "tool", tool })}
          />
        ))}
      </ul>

      <h3 className="nx-side-heading">This PC</h3>
      <ul className="nx-side-list">
        {drives.map((d) => (
          <Item
            key={d.path}
            icon={DRIVE_ICONS[d.kind]}
            label={`${d.label} (${d.letter}:)`}
            title={d.available ? d.path : `${d.path} — not available`}
            active={isAt(d.path)}
            onClick={() => onNavigate({ type: "folder", path: d.path })}
            {...(d.available ? folderProps(d.path) : {})}
          />
        ))}
      </ul>

      <h3 className="nx-side-heading">Network</h3>
      <ul className="nx-side-list">
        <Item
          icon={Network}
          label="Network"
          title="Network drives, shares and NAS devices"
          active={location.type === "network"}
          onClick={() => onNavigate({ type: "network" })}
          onMiddleClick={() => onOpenInNewTab({ type: "network" })}
        />
        {networkFavorites.map((f) => {
          const name = f.label || f.path.split("\\").filter(Boolean).pop() || f.path;
          return <Item key={f.path} icon={Server} label={name} title={f.path} active={isAt(f.path)} onClick={() => onNavigate({ type: "folder", path: f.path })} {...folderProps(f.path)} />;
        })}
      </ul>
    </nav>
  );
}
