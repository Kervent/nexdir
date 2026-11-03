import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage, isApiError } from "../../services/api";
import type { UpdateError, UpdateInfo, UpdateOffer, UpdateSnapshot, UserUpdateSettings } from "./types";

/** Wait after start before an automatic check, so it never competes with loading the UI. */
export const AUTO_CHECK_DELAY_MS = 8000;
/** Download speed is averaged over this window. */
const SPEED_WINDOW_MS = 3000;

export interface UpdatesApi {
  check: typeof api.updateCheck;
  info: typeof api.updateInfo;
  saveSettings: typeof api.updateSaveSettings;
  download: typeof api.updateDownload;
  cancel: typeof api.updateCancel;
  install: typeof api.updateInstall;
  snooze: typeof api.updateSnooze;
  ackLastInstall: typeof api.updateAckLastInstall;
  openRelease: typeof api.updateOpenRelease;
  onState: typeof api.onUpdateState;
  onProgress: typeof api.onUpdateProgress;
}

export const defaultUpdatesApi: UpdatesApi = {
  check: api.updateCheck,
  info: api.updateInfo,
  saveSettings: api.updateSaveSettings,
  download: api.updateDownload,
  cancel: api.updateCancel,
  install: api.updateInstall,
  snooze: api.updateSnooze,
  ackLastInstall: api.updateAckLastInstall,
  openRelease: api.updateOpenRelease,
  onState: api.onUpdateState,
  onProgress: api.onUpdateProgress,
};

export function toUpdateError(err: unknown): UpdateError {
  if (isApiError(err)) return { code: String(err.code), message: err.message };
  return { code: "unknown", message: errorMessage(err) };
}

/** Bytes per second over the recent samples, or null while there is too little data. */
export function speedOf(samples: { t: number; bytes: number }[]): number | null {
  if (samples.length < 2) return null;
  const first = samples[0]!;
  const last = samples[samples.length - 1]!;
  const dt = last.t - first.t;
  return dt >= 300 ? ((last.bytes - first.bytes) * 1000) / dt : null;
}

interface Options {
  /** Called with the outcome of the update installed (or not) at the last start. */
  onLastInstall?: (version: string, installed: boolean) => void;
  backend?: UpdatesApi;
  autoCheckDelay?: number;
}

export function useUpdates({ onLastInstall, backend = defaultUpdatesApi, autoCheckDelay = AUTO_CHECK_DELAY_MS }: Options = {}) {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [state, setState] = useState<UpdateSnapshot | null>(null);
  const [speed, setSpeed] = useState<number | null>(null);
  /** An update found by an automatic check, shown as a small notice. */
  const [notice, setNotice] = useState<UpdateOffer | null>(null);
  const samples = useRef<{ t: number; bytes: number }[]>([]);
  const lastInstallRef = useRef(onLastInstall);
  lastInstallRef.current = onLastInstall;

  const refresh = useCallback(async () => {
    const next = await backend.info();
    setInfo(next);
    setState(next.state);
    return next;
  }, [backend]);

  useEffect(() => {
    let cancelled = false;
    const unlisten: Promise<() => void>[] = [
      backend.onState((s) => {
        setState(s);
        if (s.phase !== "downloading") {
          samples.current = [];
          setSpeed(null);
        }
      }),
      backend.onProgress((p) => {
        setState((s) => (s ? { ...s, phase: "downloading", downloaded: p.downloaded, total: p.total ?? s.total } : s));
        const now = performance.now();
        samples.current = [...samples.current.filter((x) => now - x.t <= SPEED_WINDOW_MS), { t: now, bytes: p.downloaded }];
        setSpeed(speedOf(samples.current));
      }),
    ];
    refresh()
      .then((i) => {
        if (cancelled || !i.lastInstall) return;
        lastInstallRef.current?.(i.lastInstall.version, i.lastInstall.installed);
        void backend.ackLastInstall();
      })
      .catch(() => {});
    // The backend decides whether an automatic check is enabled and due; when it is not,
    // nothing is requested from the network.
    const timer = window.setTimeout(() => {
      backend
        .check(true)
        .then((r) => {
          if (cancelled || r.result !== "done") return;
          setState(r.state);
          if (r.notify && r.state.offer) setNotice(r.state.offer);
          return refresh();
        })
        // Automatic checks fail quietly (offline PCs are normal); the time is still recorded.
        .catch(() => void refresh().catch(() => {}));
    }, autoCheckDelay);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      unlisten.forEach((u) => void u.then((f) => f()).catch(() => {}));
    };
  }, [backend, refresh, autoCheckDelay]);

  const check = useCallback(async (): Promise<UpdateError | null> => {
    setState((s) => (s ? { ...s, phase: "checking", error: null } : s));
    try {
      const r = await backend.check(false);
      if (r.result === "done") setState(r.state);
      setNotice(null);
      await refresh();
      return null;
    } catch (e) {
      const error = toUpdateError(e);
      // "busy": another check is running; its result arrives as an event.
      if (error.code !== "busy") setState((s) => (s ? { ...s, phase: "error", error } : s));
      // Only the settings part ("Last checked"): the error stays visible.
      await backend
        .info()
        .then(setInfo)
        .catch(() => {});
      return error;
    }
  }, [backend, refresh]);

  const run = useCallback(async (action: () => Promise<UpdateSnapshot | void>): Promise<UpdateError | null> => {
    try {
      const s = await action();
      if (s) setState(s);
      return null;
    } catch (e) {
      const error = toUpdateError(e);
      setState((s) => (s ? { ...s, error } : s));
      return error;
    }
  }, []);

  const download = useCallback(() => {
    samples.current = [];
    setSpeed(null);
    return run(backend.download);
  }, [backend, run]);
  const cancel = useCallback(() => run(backend.cancel), [backend, run]);
  const install = useCallback(() => run(backend.install), [backend, run]);
  const openRelease = useCallback(() => run(backend.openRelease), [backend, run]);

  const snooze = useCallback(
    (version: string) => {
      setNotice(null);
      void backend.snooze(version).catch(() => {});
    },
    [backend],
  );

  const saveSettings = useCallback(
    async (settings: UserUpdateSettings) => {
      const next = await backend.saveSettings(settings);
      setInfo(next);
      setState(next.state);
      return next;
    },
    [backend],
  );

  return { info, state, speed, notice, dismissNotice: () => setNotice(null), refresh, check, download, cancel, install, snooze, saveSettings, openRelease };
}

export type Updates = ReturnType<typeof useUpdates>;
