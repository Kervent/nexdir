import { useCallback, useEffect, useState } from "react";
import {
  Columns2,
  Copy,
  FolderOpen,
  HardDrive,
  Info,
  PanelsTopLeft,
  Plus,
  RefreshCw,
  Search,
  Server,
  ServerCog,
  Star,
  StarOff,
  Unplug,
  Activity,
  Network,
} from "lucide-react";
import { ContextMenu, type MenuEntry } from "../../components/ContextMenu";
import { api, errorMessage } from "../../services/api";
import type { Discovery, DiscoveredShare, NetworkFavorite, NetworkLocation, NetworkOverview, StatusReport } from "../../types/network";
import { TaskStatus } from "../tools/shared";
import { useTask } from "../tools/useTask";
import type { Location } from "../workspace/workspace";
import { ConnectDialog } from "./ConnectDialog";
import { needsSignIn, STATUS_LABELS, statusTone } from "./networkPaths";

/** What the Network view needs from the app. */
export interface NetworkActions {
  navigate: (to: Location) => void;
  openInNewTab: (to: Location) => void;
  /** Opens the folder in the other pane (switches split view on if needed). */
  openInOtherPane: (path: string) => void;
  showProperties: (path: string) => void;
  notify: (tone: "info" | "success" | "error", message: string, detail?: string) => void;
  favorites: NetworkFavorite[];
  saveFavorites: (list: NetworkFavorite[]) => Promise<boolean>;
}

/** One card target: a location Windows knows, a saved one or a share found by a search. */
interface Target {
  path: string;
  /** Path to open (drive letter for mapped drives). */
  openPath: string;
  label: string;
  sub: string;
  status: StatusReport["status"] | null;
  managed: boolean;
  drive: string | null;
}

function fromLocation(l: NetworkLocation): Target {
  return {
    path: l.path,
    openPath: l.drive ? `${l.drive}\\` : l.path,
    label: l.drive ? `${l.label} (${l.drive})` : l.label,
    sub: l.path,
    status: l.status,
    managed: l.managed,
    drive: l.drive,
  };
}

function fromShare(s: DiscoveredShare): Target {
  return { path: s.path, openPath: s.path, label: s.name, sub: s.comment ? `${s.path} · ${s.comment}` : s.path, status: null, managed: false, drive: null };
}

/** Stable element id for a location (for aria-describedby). */
function idFor(path: string): string {
  let h = 5381;
  for (const c of path.toLowerCase()) h = ((h << 5) + h + c.charCodeAt(0)) >>> 0;
  return `nx-net-${h.toString(36)}`;
}

function StatusChip({ report, status }: { report?: StatusReport; status: Target["status"] }) {
  const s = report?.status ?? status;
  const text = s ? STATUS_LABELS[s] : "Not checked";
  return (
    <span className={`nx-status-chip is-${statusTone(s)}`} title={report?.message}>
      {text}
    </span>
  );
}

/**
 * Network overview: network drives and connections Windows already has, saved locations,
 * an on-demand search and manual connection. Opening `server` shows that server's shares.
 */
export function NetworkView({ server, actions }: { server?: string; actions: NetworkActions }) {
  const [overview, setOverview] = useState<NetworkOverview | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [checked, setChecked] = useState<Record<string, StatusReport>>({});
  const [checking, setChecking] = useState<Record<string, boolean>>({});
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuEntry[] } | null>(null);
  const [connect, setConnect] = useState<{ path: string } | null>(null);
  const [serverShares, setServerShares] = useState<{ loading: boolean; shares: DiscoveredShare[] | null; report: StatusReport | null }>({
    loading: false,
    shares: null,
    report: null,
  });
  const search = useTask<Discovery>();

  const load = useCallback(() => {
    setLoadError(null);
    api
      .networkOverview()
      .then(setOverview)
      .catch((e) => setLoadError(errorMessage(e)));
  }, []);
  useEffect(load, [load, actions.favorites]);

  // Shares of one server: the user opened \\server, so asking that server is expected.
  useEffect(() => {
    if (!server) return;
    let current = true;
    setServerShares({ loading: true, shares: null, report: null });
    api
      .networkListShares(server)
      .then((r) => current && setServerShares("Ok" in r ? { loading: false, shares: r.Ok, report: null } : { loading: false, shares: null, report: r.Err }))
      .catch((e) => current && setServerShares({ loading: false, shares: null, report: { status: "error", message: errorMessage(e), code: null } }));
    return () => {
      current = false;
    };
  }, [server]);

  const check = async (t: Target) => {
    setChecking((c) => ({ ...c, [t.path]: true }));
    try {
      const report = await api.networkProbe(t.path);
      setChecked((c) => ({ ...c, [t.path]: report }));
    } catch (e) {
      actions.notify("error", "Could not check the location.", errorMessage(e));
    } finally {
      setChecking((c) => ({ ...c, [t.path]: false }));
    }
  };

  const signIn = async (t: Target) => {
    try {
      const out = await api.networkConnect(t.path, true);
      setChecked((c) => ({ ...c, [t.path]: out.report }));
      if (out.report.status === "connected") {
        load();
        actions.navigate({ type: "folder", path: t.openPath });
      } else if (out.report.status !== "cancelled") actions.notify("error", STATUS_LABELS[out.report.status], out.report.message);
    } catch (e) {
      actions.notify("error", "Could not connect.", errorMessage(e));
    }
  };

  const isFavorite = (path: string) => actions.favorites.some((f) => f.path.toLowerCase() === path.toLowerCase());
  const toggleFavorite = async (t: Target) => {
    const list = isFavorite(t.path)
      ? actions.favorites.filter((f) => f.path.toLowerCase() !== t.path.toLowerCase())
      : [...actions.favorites, { path: t.path, label: t.drive ? t.label.replace(/ \([A-Z]:\)$/, "") : t.label }];
    await actions.saveFavorites(list);
  };

  const disconnect = async (t: Target) => {
    try {
      const r = await api.networkDisconnect(t.path);
      actions.notify(r.status === "notConnected" ? "success" : "error", r.status === "notConnected" ? `Disconnected from ${t.path}.` : r.message);
      load();
    } catch (e) {
      actions.notify("error", "Could not disconnect.", errorMessage(e));
    }
  };

  const openMenu = (x: number, y: number, t: Target) => {
    const report = checked[t.path];
    const items: MenuEntry[] = [
      { id: "open", label: "Open", icon: <FolderOpen size={15} />, onSelect: () => actions.navigate({ type: "folder", path: t.openPath }) },
      { id: "tab", label: "Open in new tab", icon: <PanelsTopLeft size={15} />, onSelect: () => actions.openInNewTab({ type: "folder", path: t.openPath }) },
      { id: "split", label: "Open in other pane", icon: <Columns2 size={15} />, onSelect: () => actions.openInOtherPane(t.openPath) },
      "separator",
      { id: "check", label: "Check connection", icon: <Activity size={15} />, onSelect: () => void check(t) },
      { id: "signin", label: "Sign in with Windows…", icon: <ServerCog size={15} />, disabled: !!t.drive, onSelect: () => void signIn(t) },
      isFavorite(t.path)
        ? { id: "unfav", label: "Remove from saved", icon: <StarOff size={15} />, onSelect: () => void toggleFavorite(t) }
        : { id: "fav", label: "Save in Network", icon: <Star size={15} />, onSelect: () => void toggleFavorite(t) },
      { id: "copy", label: "Copy network path", icon: <Copy size={15} />, onSelect: () => void navigator.clipboard.writeText(t.path).catch(() => {}) },
      { id: "props", label: "Properties", icon: <Info size={15} />, onSelect: () => actions.showProperties(t.openPath) },
    ];
    if (t.managed) {
      items.push("separator", { id: "disconnect", label: "Disconnect", icon: <Unplug size={15} />, onSelect: () => void disconnect(t) });
    }
    if (report && needsSignIn(report.status)) {
      // Most useful action first when sign-in is the problem.
      const i = items.findIndex((m) => m !== "separator" && m.id === "signin");
      items.unshift(items.splice(i, 1)[0]!);
    }
    setMenu({ x, y, items });
  };

  const card = (t: Target, icon: React.ReactNode) => {
    const report = checked[t.path];
    return (
      <div key={t.path} className="nx-network-card">
        <button
          type="button"
          className="nx-card"
          title={t.sub}
          onClick={() => actions.navigate({ type: "folder", path: t.openPath })}
          onMouseDown={(e) => {
            if (e.button === 1) {
              e.preventDefault();
              actions.openInNewTab({ type: "folder", path: t.openPath });
            }
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            openMenu(e.clientX, e.clientY, t);
          }}
          onKeyDown={(e) => {
            if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
              e.preventDefault();
              const r = e.currentTarget.getBoundingClientRect();
              openMenu(r.left + 24, r.bottom - 8, t);
            }
          }}
          aria-describedby={idFor(t.path)}
        >
          {icon}
          <span className="nx-card-text">
            <span className="nx-card-title nx-ellipsis">{t.label}</span>
            <span className="nx-card-sub nx-ellipsis">{t.sub}</span>
          </span>
        </button>
        <div className="nx-network-card-foot" id={idFor(t.path)}>
          {checking[t.path] ? <span className="nx-status-chip is-muted">Checking…</span> : <StatusChip report={report} status={t.status} />}
          {report && report.status !== "connected" && <span className="nx-muted nx-ellipsis" title={report.message}>{report.message}</span>}
          <span className="nx-spacer" />
          <button type="button" className="nx-icon-button is-small" aria-label={`More actions for ${t.label}`} title="More actions" onClick={(e) => openMenu(e.clientX, e.clientY, t)}>
            <span aria-hidden="true">⋯</span>
          </button>
        </div>
      </div>
    );
  };

  const locations = overview?.locations ?? [];
  const known = locations.filter((l) => l.sources.some((s) => s !== "favorite")).map(fromLocation);
  const saved = locations.filter((l) => l.sources.includes("favorite") && !l.sources.some((s) => s !== "favorite")).map(fromLocation);
  const found = search.result;
  const nothing = overview && known.length === 0 && saved.length === 0 && !found && !server;

  return (
    <div className="nx-tool nx-network">
      <header className="nx-tool-header">
        <Network size={22} />
        <div className="nx-tool-title">
          <h1>Network</h1>
          <p className="nx-muted">Network drives, shares and NAS devices (SMB). Windows handles connections and sign-in.</p>
        </div>
        <span className="nx-spacer" />
        <button type="button" className="nx-button" onClick={load} title="Reload the connections Windows knows (no network access)">
          <RefreshCw size={15} /> Refresh
        </button>
        <button type="button" className="nx-button" disabled={search.running} onClick={() => void search.start(() => api.networkDiscover())}>
          <Search size={15} /> {found ? "Search again" : "Search the network"}
        </button>
        <button type="button" className="nx-button is-primary" onClick={() => setConnect({ path: server ? `\\\\${server}\\` : "" })}>
          <Plus size={15} /> Add network share
        </button>
      </header>

      <TaskStatus progress={search.progress} onCancel={search.cancel} unit="entries" />
      {search.error && <p className="nx-field-error">{search.error}</p>}
      {loadError && <p className="nx-field-error">{loadError}</p>}
      {overview?.warning && <p className="nx-muted">{overview.warning}</p>}

      {server && (
        <section aria-labelledby="net-server">
          <h2 id="net-server" className="nx-home-heading">
            Shares on \\{server}
          </h2>
          {serverShares.loading && <p className="nx-muted" role="status">Asking \\{server} for its shares…</p>}
          {serverShares.report && (
            <div className="nx-network-result" role="alert">
              <strong>{STATUS_LABELS[serverShares.report.status]}</strong>
              <span>{serverShares.report.message}</span>
              {needsSignIn(serverShares.report.status) && <span>Open a share on this server with “Add network share” to sign in.</span>}
            </div>
          )}
          {serverShares.shares && serverShares.shares.length === 0 && <p className="nx-muted">The server reports no visible shares.</p>}
          {serverShares.shares && serverShares.shares.length > 0 && (
            <div className="nx-card-grid">{serverShares.shares.map((s) => card(fromShare(s), <FolderOpen size={22} className="nx-card-icon" aria-hidden="true" />))}</div>
          )}
        </section>
      )}

      {known.length > 0 && (
        <section aria-labelledby="net-known">
          <h2 id="net-known" className="nx-home-heading">
            Network drives and connections
          </h2>
          <div className="nx-card-grid">{known.map((t) => card(t, t.drive ? <HardDrive size={22} className="nx-card-icon" aria-hidden="true" /> : <Network size={22} className="nx-card-icon" aria-hidden="true" />))}</div>
        </section>
      )}

      {saved.length > 0 && (
        <section aria-labelledby="net-saved">
          <h2 id="net-saved" className="nx-home-heading">
            Saved
          </h2>
          <div className="nx-card-grid">{saved.map((t) => card(t, <Star size={22} className="nx-card-icon" aria-hidden="true" />))}</div>
        </section>
      )}

      {found && (
        <section aria-labelledby="net-found">
          <h2 id="net-found" className="nx-home-heading">
            Found on the network
          </h2>
          {found.cancelled && <p className="nx-muted">The search was stopped; the list may be incomplete.</p>}
          {!found.networkListAvailable && (
            <p className="nx-muted">
              Windows did not report any devices on the network. This is common on current Windows versions; servers you already used are still checked. Add other shares with their path, e.g. \\NAS\Media.
            </p>
          )}
          {found.servers.length === 0 && <p className="nx-muted">No servers found.</p>}
          {found.servers.map((s) => (
            <div key={s.path} className="nx-network-server">
              <h3 className="nx-network-server-name">
                <Server size={16} aria-hidden="true" /> {s.name}
                {!s.announced && <span className="nx-muted"> · known from your connections</span>}
              </h3>
              {s.shares && s.shares.length > 0 ? (
                <div className="nx-card-grid">{s.shares.map((sh) => card(fromShare(sh), <FolderOpen size={22} className="nx-card-icon" aria-hidden="true" />))}</div>
              ) : s.shares ? (
                <p className="nx-muted">No visible shares.</p>
              ) : (
                <p className="nx-muted">
                  Shares could not be listed: {s.status ? `${STATUS_LABELS[s.status.status]}. ${s.status.message}` : "unknown reason."}
                </p>
              )}
            </div>
          ))}
        </section>
      )}

      {nothing && (
        <div className="nx-state is-empty">
          <Network size={40} />
          <h2>No network storage found</h2>
          <p>Connect a network share manually or search for available shares.</p>
          <div className="nx-state-actions">
            <button type="button" className="nx-button is-primary" onClick={() => setConnect({ path: "" })}>
              <Plus size={15} /> Add network share
            </button>
            <button type="button" className="nx-button" disabled={search.running} onClick={() => void search.start(() => api.networkDiscover())}>
              <Search size={15} /> Search the network
            </button>
          </div>
        </div>
      )}

      <p className="nx-muted nx-network-note">
        Nexdir contacts a server only when you open it, search the network or check a connection. Deleting and moving items between a share and another drive are not
        available yet, because network shares have no Recycle Bin; copy instead.
      </p>

      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
      {connect && (
        <ConnectDialog
          initialPath={connect.path}
          onClose={() => setConnect(null)}
          onShowServer={(srv) => {
            setConnect(null);
            actions.navigate({ type: "network", server: srv });
          }}
          onConnected={(out, label, save) => {
            setConnect(null);
            const done = () => {
              actions.notify("success", out.existing ? `${out.shareRoot} was already connected.` : `Connected to ${out.shareRoot}.`);
              actions.navigate({ type: "folder", path: out.path });
            };
            if (save && !isFavorite(out.path)) void actions.saveFavorites([...actions.favorites, { path: out.path, label }]).then(done);
            else done();
          }}
        />
      )}
    </div>
  );
}
