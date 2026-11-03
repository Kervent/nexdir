import { useEffect, useId, useRef, type ReactNode } from "react";

interface DialogProps {
  title: string;
  children: ReactNode;
  footer: ReactNode;
  onClose: () => void;
  /** CSS selector of the element to focus first; defaults to the first input or button. */
  initialFocus?: string;
  /** Extra class for the dialog box, e.g. "is-wide". */
  className?: string;
}

/** Accessible modal dialog: traps Tab focus, closes on Escape and restores focus afterwards. */
export function Dialog({ title, children, footer, onClose, initialFocus, className }: DialogProps) {
  const ref = useRef<HTMLDivElement>(null);
  const titleId = useId();

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const root = ref.current;
    const target = root?.querySelector<HTMLElement>(initialFocus ?? "input, button");
    target?.focus();
    if (target instanceof HTMLInputElement) target.select();
    return () => previous?.focus?.();
  }, [initialFocus]);

  // Escape closes the topmost dialog wherever focus is — also after the focused button was
  // disabled (e.g. "Check Again" while checking) and focus fell back to the page.
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      const dialogs = document.querySelectorAll(".nx-dialog");
      if (dialogs[dialogs.length - 1] !== ref.current) return;
      e.preventDefault();
      e.stopPropagation();
      closeRef.current();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Tab" && ref.current) {
      const focusable = ref.current.querySelectorAll<HTMLElement>("input:not(:disabled), button:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, a[href], [tabindex='0']");
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
    }
  };

  return (
    <div className="nx-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className={`nx-dialog${className ? ` ${className}` : ""}`} role="dialog" aria-modal="true" aria-labelledby={titleId} onKeyDown={onKeyDown}>
        <h2 id={titleId} className="nx-dialog-title">
          {title}
        </h2>
        <div className="nx-dialog-body">{children}</div>
        <div className="nx-dialog-footer">{footer}</div>
      </div>
    </div>
  );
}
