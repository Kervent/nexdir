import type { ReactNode } from "react";
import { BadgeInfo, BookOpen, Bug, CodeXml, Keyboard, Lightbulb, MessagesSquare, Sparkles, type LucideIcon } from "lucide-react";
import { Dialog } from "../../components/Dialog";
import { SHORTCUTS } from "../settings/ShortcutsDialog";

export type HelpAction = "tour" | "documentation" | "bug" | "feature" | "repository" | "discord" | "about" | "shortcuts";

interface Topic {
  id: string;
  title: string;
  body: ReactNode;
}

/** Offline help: available without an internet connection, bundled with the app. */
export const HELP_TOPICS: Topic[] = [
  {
    id: "navigation",
    title: "Navigation",
    body: (
      <ul>
        <li>The sidebar opens Home, quick-access folders, pinned folders, tools and drives. Middle-click opens an item in a new tab.</li>
        <li>Click a part of the address bar to jump there; click the empty area (or press Ctrl+L) to type a path. %VARIABLES% and ~ work.</li>
        <li>Back / forward / up: Alt+← / Alt+→ / Alt+↑, Backspace, or the mouse side buttons.</li>
        <li>Type in the filter box (Ctrl+F) to narrow the current folder. Type letters in the file list to jump to a name.</li>
        <li>Tabs: Ctrl+T new, Ctrl+W close, Ctrl+Tab switch. Split view: Ctrl+\, switch panes with F6.</li>
      </ul>
    ),
  },
  {
    id: "files",
    title: "File operations",
    body: (
      <ul>
        <li>Copy, cut and paste with Ctrl+C / Ctrl+X / Ctrl+V — this works together with Windows Explorer.</li>
        <li>Drag and drop moves on the same drive and copies to another drive. Hold Ctrl to force a copy, Shift to force a move.</li>
        <li>When a name exists already, choose keep both, skip, or replace. Replacing writes a verified temporary copy first; your existing file is only swapped out once the new one is complete.</li>
        <li>Copies and moves run in the background. The activity button shows progress, speed and time left, and lets you cancel.</li>
        <li>Rename with F2 and undo the last rename with Ctrl+Z.</li>
        <li>Del moves items to the Recycle Bin. Nexdir never deletes permanently; on drives without a Recycle Bin (network, USB sticks) deleting is not offered.</li>
      </ul>
    ),
  },
  {
    id: "search",
    title: "Search",
    body: (
      <ul>
        <li>Open Search from the sidebar or with Ctrl+Shift+F. Type plain requests such as “PDF files from last month” or “videos larger than 2 GB in Downloads”; Nexdir shows how it understood them as chips.</li>
        <li>Wildcards (*.log) and filters for size, date, type and hidden files are available.</li>
        <li>The optional search index makes repeated searches in chosen folders fast. It is stored only on this PC and can be deleted with one click.</li>
      </ul>
    ),
  },
  {
    id: "duplicates",
    title: "Duplicate files",
    body: (
      <ul>
        <li>Choose a folder and a minimum size, then Find duplicates. Only files with byte-identical content are grouped; hard links (one file with several names) are not counted.</li>
        <li>Select the copies you don't need, or use “Select all but the oldest/newest”. At least one copy per group always stays — Nexdir checks this again right before removing anything, and re-checks that every file is still identical.</li>
        <li>Removed copies go to the Recycle Bin.</li>
      </ul>
    ),
  },
  {
    id: "storage",
    title: "Storage analysis",
    body: (
      <ul>
        <li>Analyze a drive or folder to see what uses the space: a treemap you can click into, the largest folders and files, and a breakdown by type.</li>
        <li>Links and junctions are not followed, so nothing is counted twice. Folders you cannot read are counted and shown.</li>
        <li>Right-click a folder → Analyze storage to start there.</li>
      </ul>
    ),
  },
  {
    id: "organize",
    title: "Organize rules",
    body: (
      <ul>
        <li>Rules move or copy files by extension, name, age or size into a destination folder.</li>
        <li>Every run shows a preview first; nothing changes until you confirm, and you can untick single files.</li>
      </ul>
    ),
  },
  {
    id: "shortcuts",
    title: "Keyboard shortcuts",
    body: (
      <div className="nx-shortcuts">
        {SHORTCUTS.map((g) => (
          <section key={g.group}>
            <h3>{g.group}</h3>
            <dl>
              {g.items.map(([keys, action]) => (
                <div key={keys}>
                  <dt>
                    <kbd>{keys}</kbd>
                  </dt>
                  <dd>{action}</dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
    ),
  },
  {
    id: "network",
    title: "Network shares and NAS",
    body: (
      <ul>
        <li>Open Network in the sidebar. It shows the network drives and connections Windows already has, and the shares you saved. Nothing is contacted until you open, check or search.</li>
        <li>Add network share connects to a path like \\NAS\Media or \\192.168.178.10\Backups. Type only \\NAS to see that server's shares.</li>
        <li>Nexdir uses your Windows account. If a share needs another user name and password, choose Sign in: Windows shows its own dialog and can remember them in the Credential Manager. Nexdir never sees or stores passwords.</li>
        <li>Search the network lists servers that Windows reports and the shares of servers you already used. Current Windows versions often report none; adding the path always works.</li>
        <li>Open shares in tabs or in split view next to a local folder and copy between them. Copies are verified before they count as finished.</li>
        <li>Deleting on a share and moving between a share and another drive are not available yet (shares have no Recycle Bin). Renaming and moving inside one share work.</li>
      </ul>
    ),
  },
  {
    id: "problems",
    title: "Common problems",
    body: (
      <dl className="nx-faq">
        <dt>“Access denied” when opening a folder</dt>
        <dd>Windows protects some folders. Nexdir never asks for administrator rights; use an account that has access to the folder.</dd>
        <dt>Windows SmartScreen warns when starting Nexdir</dt>
        <dd>Current builds are not code-signed yet. Check the SHA-256 checksum from the release page, then choose More info → Run anyway.</dd>
        <dt>Delete is not available on a network or USB drive</dt>
        <dd>These drives have no Recycle Bin, and Nexdir does not delete permanently. Delete such files in Explorer if you really want to.</dd>
        <dt>A network share does not open</dt>
        <dd>
          Check that the server is switched on and the name is right. “Sign-in required” or “Access denied”: choose Sign in. “Other account in use”: Windows allows one account per server; disconnect the other connection first. “Blocked by Windows”: the
          share needs a user account (Windows blocks guest access); Nexdir does not weaken this.
        </dd>
        <dt>A copy is slower than in Explorer</dt>
        <dd>Replacing files and moving between drives verify the data before the original is touched. This takes a little longer but protects your files.</dd>
        <dt>Nexdir does not start</dt>
        <dd>Nexdir needs the Microsoft Edge WebView2 Runtime. The installer adds it; for the portable version, install it from Microsoft.</dd>
        <dt>Where are my settings stored?</dt>
        <dd>In your user profile under AppData (no registry changes). Uninstalling Nexdir keeps them unless you choose to delete app data.</dd>
      </dl>
    ),
  },
];

interface HelpCenterProps {
  /** Whether a Discord invite is configured in this build. */
  discordAvailable: boolean;
  onAction: (action: HelpAction) => void;
  onClose: () => void;
  /** Topic to open first. */
  topic?: string;
}

export function HelpCenter({ discordAvailable, onAction, onClose, topic }: HelpCenterProps) {
  const card = (action: HelpAction, Icon: LucideIcon, label: string, detail: string, external = false, disabled = false) => (
    <button type="button" className="nx-help-card" disabled={disabled} onClick={() => onAction(action)}>
      <Icon size={20} aria-hidden="true" />
      <span className="nx-help-card-text">
        <strong>{label}</strong>
        <span>{detail}</span>
      </span>
      {external && !disabled && <span className="nx-help-external">Opens in browser</span>}
    </button>
  );

  return (
    <Dialog
      title="Help & Support"
      onClose={onClose}
      className="is-wide"
      footer={
        <button type="button" className="nx-button is-primary" onClick={onClose}>
          Close
        </button>
      }
    >
      <div className="nx-help-grid">
        {card("tour", Sparkles, "Take a Tour", "A one-minute guided introduction to the interface.")}
        {card("shortcuts", Keyboard, "Keyboard shortcuts", "All shortcuts at a glance (F1).")}
        {card("documentation", BookOpen, "Documentation", "Guides and technical documentation on GitHub.", true)}
        {card("bug", Bug, "Report a Bug", "Describe a problem; you review everything before it is shared.")}
        {card("feature", Lightbulb, "Request a Feature", "Suggest an idea or improvement.")}
        {card("repository", CodeXml, "GitHub Repository", "Source code, releases and issues.", true)}
        {card("discord", MessagesSquare, "Discord Community", discordAvailable ? "Questions, support and release news." : "The invite link is not available in this version yet.", true, !discordAvailable)}
        {card("about", BadgeInfo, "About Nexdir", "Version, license and privacy.")}
      </div>

      <h3 className="nx-help-heading">Quick help</h3>
      <p className="nx-muted">Available offline. Links to GitHub and Discord only open when you click them.</p>
      <div className="nx-help-topics">
        {HELP_TOPICS.map((t) => (
          <details key={t.id} className="nx-help-topic" open={t.id === topic}>
            <summary>{t.title}</summary>
            <div className="nx-help-topic-body">{t.body}</div>
          </details>
        ))}
      </div>
    </Dialog>
  );
}
