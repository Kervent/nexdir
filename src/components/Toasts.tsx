import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from "react";
import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";

export type ToastTone = "info" | "success" | "error";

interface Toast {
  id: number;
  tone: ToastTone;
  message: string;
  detail?: string;
}

type Notify = (tone: ToastTone, message: string, detail?: string) => void;

const ToastContext = createContext<Notify>(() => {});

export function useNotify(): Notify {
  return useContext(ToastContext);
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);

  const notify = useCallback<Notify>(
    (tone, message, detail) => {
      const id = nextId.current++;
      setToasts((t) => [...t.slice(-3), { id, tone, message, detail }]);
      // Errors stay until dismissed so they can be read; others fade out.
      if (tone !== "error") window.setTimeout(() => dismiss(id), 4500);
    },
    [dismiss],
  );

  const value = useMemo(() => notify, [notify]);
  const icons = { info: Info, success: CheckCircle2, error: AlertCircle };

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="nx-toasts" role="status" aria-live="polite">
        {toasts.map((t) => {
          const Icon = icons[t.tone];
          return (
            <div key={t.id} className={`nx-toast is-${t.tone}`} role={t.tone === "error" ? "alert" : undefined}>
              <Icon size={18} aria-hidden="true" className="nx-toast-icon" />
              <div className="nx-toast-text">
                <div>{t.message}</div>
                {t.detail && <div className="nx-toast-detail">{t.detail}</div>}
              </div>
              <button type="button" className="nx-icon-button is-small" aria-label="Dismiss" onClick={() => dismiss(t.id)}>
                <X size={14} />
              </button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}
