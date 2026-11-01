import { useRef, useState } from "react";
import { Columns2, House, Folder, Plus, X } from "lucide-react";
import { currentLocation, locationLabel, type Workspace } from "./workspace";
import { TOOL_ICONS } from "../navigation/Sidebar";

interface TabBarProps {
  ws: Workspace;
  onActivate: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onNew: () => void;
  onMove: (tabId: string, toIndex: number) => void;
}

const TAB_MIME = "application/x-nexdir-tab";

export function TabBar({ ws, onActivate, onClose, onNew, onMove }: TabBarProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const tabs = Array.from(listRef.current?.querySelectorAll<HTMLElement>("[role=tab]") ?? []);
    const i = tabs.indexOf(document.activeElement as HTMLElement);
    if (i < 0) return;
    const next = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : null;
    if (next === null) return;
    e.preventDefault();
    const target = tabs[(next + tabs.length) % tabs.length];
    target?.focus();
    target?.click();
  };

  return (
    <div className="nx-tabbar" data-tour="tabs">
      <div ref={listRef} className="nx-tabs" role="tablist" aria-label="Tabs" onKeyDown={onKeyDown}>
        {ws.tabs.map((tab, index) => {
          const pane = ws.panes[tab.activePane]!;
          const loc = currentLocation(pane);
          const label = locationLabel(loc);
          const active = tab.id === ws.activeTab;
          const Icon = loc.type === "home" ? House : loc.type === "tool" ? TOOL_ICONS[loc.tool] : Folder;
          return (
            <div
              key={tab.id}
              role="tab"
              aria-selected={active}
              tabIndex={active ? 0 : -1}
              title={loc.type === "home" ? "Home" : (loc.path ?? label)}
              className={`nx-tab${active ? " is-active" : ""}${dropIndex === index ? " is-drop-before" : ""}`}
              draggable
              onClick={() => onActivate(tab.id)}
              onMouseDown={(e) => {
                if (e.button === 1) {
                  e.preventDefault();
                  onClose(tab.id);
                }
              }}
              onDragStart={(e) => {
                e.dataTransfer.setData(TAB_MIME, tab.id);
                e.dataTransfer.effectAllowed = "move";
              }}
              onDragOver={(e) => {
                if (!e.dataTransfer.types.includes(TAB_MIME)) return;
                e.preventDefault();
                setDropIndex(index);
              }}
              onDragLeave={() => setDropIndex(null)}
              onDrop={(e) => {
                const id = e.dataTransfer.getData(TAB_MIME);
                setDropIndex(null);
                if (id) onMove(id, index);
              }}
            >
              <Icon size={14} aria-hidden="true" className="nx-tab-icon" />
              <span className="nx-tab-label">{label}</span>
              {tab.panes.length > 1 && <Columns2 size={12} aria-label="Split view" className="nx-tab-split" />}
              <button
                type="button"
                className="nx-tab-close"
                aria-label={`Close tab ${label}`}
                tabIndex={-1}
                onClick={(e) => {
                  e.stopPropagation();
                  onClose(tab.id);
                }}
              >
                <X size={12} />
              </button>
            </div>
          );
        })}
      </div>
      <button type="button" className="nx-icon-button is-small nx-tab-new" aria-label="New tab (Ctrl+T)" title="New tab (Ctrl+T)" onClick={onNew}>
        <Plus size={15} />
      </button>
    </div>
  );
}
