import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { ChevronRight, House } from "lucide-react";
import { breadcrumbs, type Crumb } from "../../lib/paths";
import { TOOL_LABELS, type Location } from "../workspace/workspace";
import { useDropTarget, type DropEffect } from "../dnd/dnd";

export interface AddressBarHandle {
  edit: () => void;
}

interface AddressBarProps {
  location: Location;
  onNavigate: (to: Location) => void;
  /** Resolves typed text; returns an error message or null on success. */
  onSubmit: (text: string) => Promise<string | null>;
  onDropPaths: (paths: string[], effect: DropEffect, target: string) => void;
}

function CrumbButton({ crumb, current, onNavigate, onDropPaths }: {
  crumb: Crumb;
  current: boolean;
  onNavigate: (to: Location) => void;
  onDropPaths: AddressBarProps["onDropPaths"];
}) {
  const { dropProps, isOver } = useDropTarget(crumb.path, onDropPaths);
  return (
    <span className="nx-crumb-wrap">
      <ChevronRight size={14} className="nx-crumb-sep" aria-hidden="true" />
      <button
        type="button"
        className={`nx-crumb${current ? " is-current" : ""}${isOver ? " is-drop-target" : ""}`}
        title={crumb.path}
        onClick={() => onNavigate({ type: "folder", path: crumb.path })}
        {...dropProps}
      >
        {crumb.label}
      </button>
    </span>
  );
}

/** Breadcrumb address bar that switches to a text field on click or Ctrl+L. */
export const AddressBar = forwardRef<AddressBarHandle, AddressBarProps>(function AddressBar({ location, onNavigate, onSubmit, onDropPaths }, ref) {
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const path = location.type === "folder" ? location.path : "";

  const startEditing = () => {
    setText(path);
    setError(null);
    setEditing(true);
  };
  useImperativeHandle(ref, () => ({ edit: startEditing }));

  useEffect(() => {
    if (editing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [editing]);

  useEffect(() => setEditing(false), [location]);

  const submit = async () => {
    if (!text.trim()) return setEditing(false);
    const err = await onSubmit(text);
    if (err) setError(err);
    else setEditing(false);
  };

  if (editing) {
    return (
      <div className="nx-address is-editing">
        <input
          ref={inputRef}
          className="nx-address-input"
          aria-label="Address"
          aria-invalid={error ? true : undefined}
          value={text}
          spellCheck={false}
          onChange={(e) => {
            setText(e.target.value);
            setError(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") void submit();
            if (e.key === "Escape") setEditing(false);
          }}
          onBlur={() => !error && setEditing(false)}
        />
        {error && (
          <div className="nx-address-error" role="alert">
            {error}
          </div>
        )}
      </div>
    );
  }

  const crumbs = location.type === "folder" ? breadcrumbs(location.path) : [];
  return (
    <div className="nx-address" onClick={(e) => e.target === e.currentTarget && startEditing()}>
      <button type="button" className="nx-crumb" onClick={() => onNavigate({ type: "home" })} aria-label="Home">
        <House size={14} />
      </button>
      {crumbs.map((c, i) => (
        <CrumbButton key={c.path} crumb={c} current={i === crumbs.length - 1} onNavigate={onNavigate} onDropPaths={onDropPaths} />
      ))}
      {location.type === "tool" && (
        <span className="nx-crumb-wrap">
          <ChevronRight size={14} className="nx-crumb-sep" aria-hidden="true" />
          <span className="nx-crumb is-current" title={location.path}>
            {TOOL_LABELS[location.tool]}
            {location.path ? ` · ${location.path}` : ""}
          </span>
        </span>
      )}
      {location.type === "home" && (
        <span className="nx-crumb-wrap">
          <ChevronRight size={14} className="nx-crumb-sep" aria-hidden="true" />
          <span className="nx-crumb is-current">Home</span>
        </span>
      )}
      <button type="button" className="nx-address-fill" aria-label="Edit address" onClick={startEditing} />
    </div>
  );
});
