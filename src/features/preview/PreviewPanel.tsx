import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { api, errorMessage } from "../../services/api";
import { FileIcon } from "../../components/FileIcon";
import { formatDate, formatSize, pluralize } from "../../lib/format";
import { typeLabel } from "../../lib/fileTypes";
import type { Preview } from "../../types/fs";

interface PreviewPanelProps {
  /** Item to preview: the single selected item, or the current folder. */
  path: string | null;
  /** Changes when the item may have changed on disk (e.g. after a refresh). */
  version: number;
  onClose: () => void;
}

/**
 * Read-only preview. Images are shown through <img> (no script execution, even for
 * SVG); text is rendered as plain text by React, never as HTML.
 */
export function PreviewPanel({ path, version, onClose }: PreviewPanelProps) {
  const [preview, setPreview] = useState<Preview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setPreview(null);
    setError(null);
    if (!path) return;
    let stale = false;
    // Debounce so arrowing through a list doesn't read every file on the way.
    const timer = window.setTimeout(() => {
      setLoading(true);
      api
        .previewPath(path)
        .then((p) => !stale && setPreview(p))
        .catch((e) => !stale && setError(errorMessage(e)))
        .finally(() => !stale && setLoading(false));
    }, 150);
    return () => {
      stale = true;
      window.clearTimeout(timer);
    };
  }, [path, version]);

  const entry = preview?.entry;
  const content = preview?.content;

  return (
    <aside className="nx-preview" aria-label="Preview">
      <header className="nx-preview-header">
        <h2>Preview</h2>
        <button type="button" className="nx-icon-button is-small" aria-label="Close preview (Alt+P)" title="Close preview (Alt+P)" onClick={onClose}>
          <X size={14} />
        </button>
      </header>
      <div className="nx-preview-body" aria-busy={loading}>
        {!path && <p className="nx-muted">Select a file to preview it.</p>}
        {error && <p className="nx-field-error">{error}</p>}
        {content?.type === "image" && (
          <div className="nx-preview-media">
            <img src={`data:${content.mime};base64,${content.data}`} alt={entry?.name ?? "Preview"} draggable={false} />
          </div>
        )}
        {content?.type === "text" && (
          <>
            <pre className="nx-preview-text">{content.text}</pre>
            {content.truncated && <p className="nx-muted">Only the beginning of the file is shown.</p>}
          </>
        )}
        {entry && content?.type !== "image" && content?.type !== "text" && (
          <div className="nx-preview-icon">
            <FileIcon entry={entry} size={64} />
          </div>
        )}
        {entry && (
          <dl className="nx-preview-meta">
            <dt>Name</dt>
            <dd>{entry.name}</dd>
            <dt>Type</dt>
            <dd>{typeLabel(entry)}</dd>
            {entry.size != null && (
              <>
                <dt>Size</dt>
                <dd>
                  {formatSize(entry.size)} ({entry.size.toLocaleString()} bytes)
                </dd>
              </>
            )}
            {content?.type === "folder" && (
              <>
                <dt>Contains</dt>
                <dd>
                  {content.truncated ? "more than " : ""}
                  {pluralize(content.items, "item")}
                </dd>
              </>
            )}
            <dt>Modified</dt>
            <dd>{formatDate(entry.modified) || "—"}</dd>
            <dt>Created</dt>
            <dd>{formatDate(entry.created) || "—"}</dd>
            <dt>Attributes</dt>
            <dd>{[entry.readonly && "Read-only", entry.hidden && "Hidden", entry.system && "System"].filter(Boolean).join(", ") || "—"}</dd>
            <dt>Location</dt>
            <dd className="nx-preview-path">{entry.path}</dd>
          </dl>
        )}
        {content?.type === "none" && <p className="nx-muted">{content.reason}</p>}
      </div>
    </aside>
  );
}
