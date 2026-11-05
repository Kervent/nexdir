import { useEffect, useState } from "react";
import { KeyRound, Server } from "lucide-react";
import { Dialog } from "../../components/Dialog";
import { api, errorMessage } from "../../services/api";
import type { ConnectOutcome, ParsedNetworkPath } from "../../types/network";
import { needsSignIn, STATUS_LABELS } from "./networkPaths";

interface Props {
  /** Prefilled path, e.g. a share chosen in the Network view. */
  initialPath?: string;
  onClose: () => void;
  /** The share is reachable: open it (and save it if the user asked for that). */
  onConnected: (outcome: ConnectOutcome, label: string, save: boolean) => void;
  /** The user typed only a server name: show its shares. */
  onShowServer: (server: string) => void;
}

const KIND_HINT = {
  server: "This is a server. Choose one of its shares, e.g. \\\\NAS\\Media.",
  share: "Share",
  folder: "Folder in a share",
} as const;

/** "Connect to a network share": path, optional name, optional saving. Sign-in goes through Windows. */
export function ConnectDialog({ initialPath = "", onClose, onConnected, onShowServer }: Props) {
  const [path, setPath] = useState(initialPath);
  const [label, setLabel] = useState("");
  const [save, setSave] = useState(true);
  const [parsed, setParsed] = useState<ParsedNetworkPath | null>(null);
  const [inputError, setInputError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"connect" | "signIn" | null>(null);
  const [result, setResult] = useState<ConnectOutcome | null>(null);

  // Live validation (no network access).
  useEffect(() => {
    setResult(null);
    if (!path.trim()) {
      setParsed(null);
      setInputError(null);
      return;
    }
    let current = true;
    const t = window.setTimeout(() => {
      api
        .networkParse(path)
        .then((p) => current && (setParsed(p), setInputError(null)))
        .catch((e) => current && (setParsed(null), setInputError(errorMessage(e))));
    }, 150);
    return () => {
      current = false;
      window.clearTimeout(t);
    };
  }, [path]);

  const connect = async (signIn: boolean) => {
    if (!parsed) return;
    if (parsed.kind === "server") return onShowServer(parsed.server);
    setBusy(signIn ? "signIn" : "connect");
    try {
      const outcome = await api.networkConnect(parsed.path, signIn);
      setResult(outcome);
      if (outcome.report.status === "connected") onConnected(outcome, label.trim(), save);
    } catch (e) {
      setInputError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const report = result?.report;
  // After Windows' dialog was cancelled, signing in again is the natural next step.
  const canSignIn = report && (needsSignIn(report.status) || report.status === "cancelled");
  return (
    <Dialog
      title="Connect to a network share"
      className="is-wide"
      onClose={onClose}
      initialFocus="#nx-connect-path"
      footer={
        <>
          <button type="button" className="nx-button" onClick={onClose}>
            Cancel
          </button>
          {canSignIn && (
            <button type="button" className="nx-button" disabled={busy !== null} onClick={() => void connect(true)}>
              <KeyRound size={15} /> {busy === "signIn" ? "Waiting for Windows…" : "Sign in…"}
            </button>
          )}
          <button type="button" className="nx-button is-primary" disabled={!parsed || busy !== null} onClick={() => void connect(false)}>
            {parsed?.kind === "server" ? (
              <>
                <Server size={15} /> Show shares
              </>
            ) : busy === "connect" ? (
              "Connecting…"
            ) : result ? (
              "Try again"
            ) : (
              "Connect"
            )}
          </button>
        </>
      }
    >
      <form
        className="nx-form"
        onSubmit={(e) => {
          e.preventDefault();
          void connect(false);
        }}
      >
        <label className="nx-field">
          <span>Server or network path</span>
          <input
            id="nx-connect-path"
            className="nx-input"
            value={path}
            spellCheck={false}
            autoComplete="off"
            placeholder="\\NAS\Media"
            aria-invalid={inputError ? true : undefined}
            aria-describedby="nx-connect-hint"
            onChange={(e) => setPath(e.target.value)}
          />
        </label>
        <p id="nx-connect-hint" className={inputError ? "nx-field-error" : "nx-muted"} role={inputError ? "alert" : undefined}>
          {inputError ?? (parsed ? `${KIND_HINT[parsed.kind]}${parsed.kind === "server" ? "" : `: ${parsed.path}`}` : "Examples: \\\\192.168.178.10\\Media · \\\\NAS-SERVER\\Backups · \\\\homeserver\\Documents")}
        </p>
        <label className="nx-field">
          <span>Name (optional)</span>
          <input className="nx-input" value={label} maxLength={120} placeholder={parsed?.share ?? "Home NAS"} onChange={(e) => setLabel(e.target.value)} />
        </label>
        <label className="nx-check">
          <input type="checkbox" checked={save} onChange={(e) => setSave(e.target.checked)} /> Save in Network (only the path and name are saved, never a password)
        </label>
        <p className="nx-muted">
          Nexdir uses your Windows account. If the share needs a different user name and password, Windows shows its own sign-in dialog; Windows can remember them in its Credential Manager. Nexdir never sees or stores them.
        </p>
        {report && report.status !== "connected" && (
          <div className="nx-network-result" role="alert">
            <strong>{STATUS_LABELS[report.status]}</strong>
            <span>{report.message}</span>
            {report.code !== null && <span className="nx-muted">Windows error {report.code}</span>}
          </div>
        )}
      </form>
    </Dialog>
  );
}
