import { Dialog } from "../../components/Dialog";

export const SHORTCUTS: { group: string; items: [keys: string, action: string][] }[] = [
  {
    group: "Navigation",
    items: [
      ["Alt+← / Backspace", "Back"],
      ["Alt+→", "Forward"],
      ["Alt+↑", "Parent folder"],
      ["Ctrl+L, Alt+D", "Edit address"],
      ["Ctrl+F, Ctrl+E", "Filter current folder"],
      ["F5", "Refresh"],
      ["Enter", "Open"],
    ],
  },
  {
    group: "Tabs and panes",
    items: [
      ["Ctrl+T", "New tab"],
      ["Ctrl+W", "Close tab"],
      ["Ctrl+Tab / Ctrl+Shift+Tab", "Next / previous tab"],
      ["Ctrl+1 … Ctrl+9", "Go to tab"],
      ["Middle click", "Open folder in new tab"],
      ["Ctrl+\\", "Toggle split view"],
      ["F6", "Switch pane"],
      ["Alt+P", "Toggle preview"],
    ],
  },
  {
    group: "Files",
    items: [
      ["Ctrl+C / Ctrl+X / Ctrl+V", "Copy / cut / paste (works with Explorer)"],
      ["Drag", "Move (same drive) or copy (other drive)"],
      ["Ctrl+Drag / Shift+Drag", "Force copy / force move"],
      ["Ctrl+Shift+C", "Copy path"],
      ["F2", "Rename"],
      ["Ctrl+Z", "Undo last rename"],
      ["Del", "Move to Recycle Bin"],
      ["Ctrl+Shift+N", "New folder"],
      ["Ctrl+A", "Select all"],
      ["Ctrl+D", "Pin current folder"],
      ["Alt+Enter", "Properties"],
      ["F1, Ctrl+/", "This list"],
    ],
  },
];

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog
      title="Keyboard shortcuts"
      onClose={onClose}
      footer={
        <button type="button" className="nx-button is-primary" onClick={onClose}>
          Close
        </button>
      }
    >
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
    </Dialog>
  );
}
