import { act, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { ReleaseNotes, stripTags } from "./ReleaseNotes";
import { UpdateDialog, signatureStatus } from "./UpdateDialog";
import { UpdateNotice } from "./UpdateNotice";
import { speedOf, useUpdates, type Updates, type UpdatesApi } from "./useUpdates";
import { SettingsDialog } from "../settings/SettingsDialog";
import { defaultPreferences } from "../settings/preferences";
import type { CheckResult, UpdateInfo, UpdateOffer, UpdateSnapshot } from "./types";

const offer: UpdateOffer = {
  version: "0.5.0-alpha.2",
  prerelease: true,
  pubDate: "2026-11-20T12:00:00Z",
  notes: "## Fixes\n- Faster copies\n- **Safer** updates",
  releaseUrl: "https://github.com/Kervent/nexdir/releases/tag/v0.5.0-alpha.2",
  size: 2_600_000,
};

function snapshot(patch: Partial<UpdateSnapshot> = {}): UpdateSnapshot {
  return { phase: "idle", channel: "preview", offer: null, latest: null, noRelease: false, downloaded: 0, total: null, error: null, ...patch };
}

function info(patch: Partial<UpdateInfo> = {}): UpdateInfo {
  return {
    currentVersion: "0.5.0-alpha.1",
    installKind: "nsis",
    canInstall: true,
    defaultChannel: "preview",
    autoCheck: false,
    channel: "preview",
    notify: true,
    intervalHours: 24,
    lastSuccess: null,
    lastAttempt: null,
    releasesUrl: "https://github.com/Kervent/nexdir/releases",
    lastInstall: null,
    state: snapshot(),
    ...patch,
  };
}

function backend(overrides: Partial<UpdatesApi> = {}): UpdatesApi & { calls: string[] } {
  const calls: string[] = [];
  const track =
    <T,>(name: string, value: T) =>
    (...args: unknown[]) => {
      calls.push(`${name}(${args.map((a) => JSON.stringify(a)).join(",")})`);
      return Promise.resolve(value);
    };
  return {
    calls,
    info: track("info", info()) as UpdatesApi["info"],
    check: track("check", { result: "skipped" } satisfies CheckResult) as UpdatesApi["check"],
    saveSettings: track("saveSettings", info()) as UpdatesApi["saveSettings"],
    download: track("download", snapshot({ phase: "downloading", offer })) as UpdatesApi["download"],
    cancel: track("cancel", snapshot({ phase: "available", offer })) as UpdatesApi["cancel"],
    install: track("install", undefined) as UpdatesApi["install"],
    snooze: track("snooze", undefined) as UpdatesApi["snooze"],
    ackLastInstall: track("ackLastInstall", undefined) as UpdatesApi["ackLastInstall"],
    openRelease: track("openRelease", undefined) as UpdatesApi["openRelease"],
    onState: () => Promise.resolve(() => {}),
    onProgress: () => Promise.resolve(() => {}),
    ...overrides,
  };
}

/** A fixed hook state for rendering the dialog in a given phase. */
function updatesWith(state: UpdateSnapshot, i: UpdateInfo = info(), speed: number | null = null): Updates & { calls: string[] } {
  const calls: string[] = [];
  const rec = (name: string) => async () => {
    calls.push(name);
    return null;
  };
  return {
    calls,
    info: i,
    state,
    speed,
    notice: null,
    dismissNotice: () => {},
    refresh: async () => i,
    check: rec("check"),
    download: rec("download"),
    cancel: rec("cancel"),
    install: rec("install"),
    openRelease: rec("openRelease"),
    snooze: (v: string) => void calls.push(`snooze ${v}`),
    saveSettings: async (s) => {
      calls.push(`save ${JSON.stringify(s)}`);
      return i;
    },
  };
}

describe("automatic update checks", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("asks the backend only after the start delay and shows nothing when the check was skipped", async () => {
    const b = backend();
    const { result } = renderHook(() => useUpdates({ backend: b, autoCheckDelay: 5000 }));
    await act(async () => {});
    expect(b.calls.filter((c) => c.startsWith("check"))).toEqual([]);
    await act(async () => vi.advanceTimersByTime(5000));
    expect(b.calls).toContain("check(true)");
    expect(result.current.notice).toBeNull();
  });

  it("shows a notice when an automatic check found an update the user wants to hear about", async () => {
    const b = backend({ check: () => Promise.resolve({ result: "done", notify: true, state: snapshot({ phase: "available", offer }) }) });
    const { result } = renderHook(() => useUpdates({ backend: b, autoCheckDelay: 10 }));
    await act(async () => vi.advanceTimersByTime(10));
    expect(result.current.notice?.version).toBe("0.5.0-alpha.2");
  });

  it("never downloads or installs on its own", async () => {
    const b = backend({ check: () => Promise.resolve({ result: "done", notify: true, state: snapshot({ phase: "available", offer }) }) });
    renderHook(() => useUpdates({ backend: b, autoCheckDelay: 10 }));
    await act(async () => vi.advanceTimersByTime(60_000));
    expect(b.calls.some((c) => c.startsWith("download") || c.startsWith("install"))).toBe(false);
  });

  it("keeps quiet when an automatic check fails (offline)", async () => {
    const b = backend({ check: () => Promise.reject({ code: "offline", message: "No connection" }) });
    const { result } = renderHook(() => useUpdates({ backend: b, autoCheckDelay: 10 }));
    await act(async () => vi.advanceTimersByTime(10));
    expect(result.current.notice).toBeNull();
    expect(result.current.state?.phase).toBe("idle");
  });

  it("reports the result of the update started last time once", async () => {
    const seen: string[] = [];
    const b = backend({ info: () => Promise.resolve(info({ lastInstall: { version: "0.5.0-alpha.1", installed: true } })) });
    renderHook(() => useUpdates({ backend: b, autoCheckDelay: 1e9, onLastInstall: (v, ok) => seen.push(`${v} ${ok}`) }));
    await act(async () => {});
    expect(seen).toEqual(["0.5.0-alpha.1 true"]);
    expect(b.calls).toContain("ackLastInstall()");
  });
});

describe("manual checks", () => {
  it("shows the backend's error message", async () => {
    const b = backend({ check: () => Promise.reject({ code: "rateLimited", message: "GitHub is limiting requests right now (HTTP 429)." }) });
    const { result } = renderHook(() => useUpdates({ backend: b, autoCheckDelay: 1e9 }));
    await act(async () => {});
    let error: unknown;
    await act(async () => {
      error = await result.current.check();
    });
    expect(error).toEqual({ code: "rateLimited", message: "GitHub is limiting requests right now (HTTP 429)." });
    expect(result.current.state?.phase).toBe("error");
  });

  it("does not show a second check as an error", async () => {
    const b = backend({ check: () => Promise.reject({ code: "busy", message: "An update check is already running." }) });
    const { result } = renderHook(() => useUpdates({ backend: b, autoCheckDelay: 1e9 }));
    await act(async () => {});
    await act(async () => void (await result.current.check()));
    expect(result.current.state?.phase).not.toBe("error");
  });
});

describe("download speed", () => {
  it("averages over the samples and needs enough time", () => {
    expect(speedOf([])).toBeNull();
    expect(speedOf([{ t: 0, bytes: 0 }, { t: 100, bytes: 1000 }])).toBeNull();
    expect(speedOf([{ t: 0, bytes: 0 }, { t: 1000, bytes: 500_000 }, { t: 2000, bytes: 1_000_000 }])).toBe(500_000);
  });
});

describe("update window", () => {
  const show = (u: Updates) => render(<UpdateDialog updates={u} onClose={() => {}} onOpenSettings={() => {}} />);

  it("shows versions, date, channel, size and a preview warning", () => {
    show(updatesWith(snapshot({ phase: "available", offer, total: offer.size })));
    expect(screen.getByRole("dialog", { name: "Nexdir 0.5.0-alpha.2 is available" })).toBeInTheDocument();
    expect(screen.getByText("0.5.0-alpha.1")).toBeInTheDocument();
    expect(screen.getByText("Beta (Preview)")).toBeInTheDocument();
    expect(screen.getByText("2.48 MB")).toBeInTheDocument();
    expect(screen.getByText(/preview \(alpha\/beta\) version/)).toBeInTheDocument();
    expect(screen.getByText("Faster copies")).toBeInTheDocument();
    for (const name of ["View Changes", "Download Update", "Remind Me Later"]) expect(screen.getByRole("button", { name: new RegExp(name) })).toBeInTheDocument();
  });

  it("downloads only on click", () => {
    const u = updatesWith(snapshot({ phase: "available", offer }));
    show(u);
    expect(u.calls).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: /Download Update/ }));
    expect(u.calls).toEqual(["download"]);
  });

  it("shows real progress and speed while downloading, and can cancel", () => {
    const u = updatesWith(snapshot({ phase: "downloading", offer, downloaded: 1_300_000, total: 2_600_000 }), info(), 512 * 1024);
    show(u);
    expect(screen.getByRole("progressbar", { name: "Download progress" })).toHaveAttribute("aria-valuenow", "50");
    expect(screen.getByText(/1\.24 MB of 2\.48 MB · 512 KB\/s/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Download Update/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Cancel Download" }));
    expect(u.calls).toEqual(["cancel"]);
  });

  it("installs only after an explicit second confirmation", async () => {
    const u = updatesWith(snapshot({ phase: "readyToInstall", offer, downloaded: 2_600_000, total: 2_600_000 }));
    show(u);
    expect(screen.getByText(/signature was verified/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Install Update…" }));
    expect(u.calls).toEqual([]);
    expect(screen.getByText(/Nexdir closes now/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close Nexdir and Install" }));
    await waitFor(() => expect(u.calls).toEqual(["install"]));
  });

  it("explains errors without technical details", () => {
    show(updatesWith(snapshot({ phase: "error", offer, error: { code: "signature", message: "The update's signature is invalid. The download was discarded and nothing was installed." } })));
    expect(screen.getByRole("alert")).toHaveTextContent("signature is invalid");
  });

  it("portable copies get the download page instead of an installer", () => {
    show(updatesWith(snapshot({ phase: "available", offer }), info({ installKind: "portable", canInstall: false })));
    expect(screen.queryByRole("button", { name: /Download Update/ })).toBeNull();
    expect(screen.getByRole("button", { name: /Open Download Page/ })).toBeInTheDocument();
    expect(screen.getByText(/portable version is not updated automatically/)).toBeInTheDocument();
  });

  it("an empty stable channel invents no release", () => {
    show(updatesWith(snapshot({ phase: "upToDate", channel: "stable", noRelease: true }), info({ channel: "stable" })));
    expect(screen.getByText(/There is no Stable release yet/)).toBeInTheDocument();
    expect(screen.getByText("Available version").nextSibling).toHaveTextContent("—");
  });

  it("describes the signature status honestly", () => {
    const s = snapshot({ phase: "available", offer });
    expect(signatureStatus(s, true, "nsis")).toMatch(/after the download/);
    expect(signatureStatus({ ...s, phase: "readyToInstall" }, true, "msi")).toMatch(/Verified/);
    expect(signatureStatus(s, false, "nsis")).toMatch(/no update key/);
    expect(signatureStatus(s, false, "portable")).toMatch(/SHA-256/);
  });
});

describe("update notice", () => {
  it("is non-modal and offers the three actions", () => {
    const done: string[] = [];
    render(
      <UpdateNotice
        offer={offer}
        currentVersion="0.5.0-alpha.1"
        onViewChanges={() => done.push("view")}
        onDownload={() => done.push("download")}
        onRemindLater={() => done.push("later")}
        onDismiss={() => done.push("dismiss")}
      />,
    );
    const notice = screen.getByRole("status", { name: "Update available" });
    expect(notice).toHaveTextContent("Nexdir 0.5.0-alpha.2 is available");
    expect(notice).toHaveTextContent("Installed version: 0.5.0-alpha.1 · Available version: 0.5.0-alpha.2");
    expect(document.activeElement).toBe(document.body);
    for (const name of ["View Changes", "Download Update", "Remind Me Later"]) fireEvent.click(screen.getByRole("button", { name }));
    expect(done).toEqual(["view", "download", "later"]);
  });
});

describe("release notes", () => {
  it("never renders HTML from the network", () => {
    const { container } = render(<ReleaseNotes text={'<img src=x onerror="alert(1)"> <script>alert(2)</script> [link](https://evil.example) **bold**'} />);
    expect(container.querySelector("img, script, a")).toBeNull();
    expect(container).toHaveTextContent("link");
    expect(container.querySelector("strong")).toHaveTextContent("bold");
  });

  it("strips tags character by character", () => {
    expect(stripTags("a<b>c</b>d")).toBe("acd");
    expect(stripTags("x<scr<script>ipt>y")).toBe("xipt>y"); // a lone ">" is plain text; no "<" survives
    expect(stripTags("keep 1 > 0 and <unclosed")).toBe("keep 1 > 0 and ");
    expect(stripTags("no tags")).toBe("no tags");
  });

  it("leaves no tag remnants from nested or unclosed tags", () => {
    const { container } = render(<ReleaseNotes text={"a<scr<script>ipt>b <img src=x onerror=1 c"} />);
    expect(container.textContent).not.toContain("<");
    expect(container.querySelector("script, img")).toBeNull();
  });

  it("shows GitHub alerts as plain text and the real release notes without markup", () => {
    const { container } = render(<ReleaseNotes text={["> [!WARNING]", "> **Alpha.** Keep backups.", "", "| File | Use |", "|---|---|", "| a | b |"].join("\n")} />);
    expect(container.textContent).toBe("Alpha. Keep backups.");
  });

  it("renders headings and lists", () => {
    const { container } = render(<ReleaseNotes text={"## New\n- One\n- Two\n\nText"} />);
    expect(container.querySelector("h4")).toHaveTextContent("New");
    expect(container.querySelectorAll("li")).toHaveLength(2);
  });
});

describe("update settings", () => {
  const open = (u: Updates) =>
    render(<SettingsDialog prefs={defaultPreferences} updatePrefs={() => {}} updates={u} section="updates" onCheckForUpdates={() => {}} onOpenUrl={() => {}} onClose={() => {}} />);

  it("has automatic checks off by default and shows version and last check", () => {
    open(updatesWith(snapshot()));
    expect(screen.getByRole("checkbox", { name: "Automatically check for updates" })).not.toBeChecked();
    expect(screen.getByText("Never")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Updates" })).toHaveAttribute("aria-selected", "true");
  });

  it("saves the setting in the backend", async () => {
    const u = updatesWith(snapshot());
    open(u);
    fireEvent.click(screen.getByRole("checkbox", { name: "Automatically check for updates" }));
    await waitFor(() => expect(u.calls).toEqual([`save ${JSON.stringify({ autoCheck: true, channel: "preview", notify: true, intervalHours: 24 })}`]));
  });

  it("asks before switching to the preview channel", async () => {
    const u = updatesWith(snapshot({ channel: "stable" }), info({ channel: "stable" }));
    open(u);
    fireEvent.change(screen.getByLabelText("Release channel"), { target: { value: "preview" } });
    expect(u.calls).toEqual([]);
    expect(screen.getByRole("alert")).toHaveTextContent(/experimental/);
    fireEvent.click(screen.getByRole("button", { name: "Use Beta (Preview)" }));
    await waitFor(() => expect(u.calls[0]).toContain('"channel":"preview"'));
  });

  it("explains what a check sends", () => {
    open(updatesWith(snapshot()));
    expect(screen.getByText(/No file names, folders, paths, account data or device IDs are sent/)).toBeInTheDocument();
    expect(screen.getByText(/reveals your IP address/)).toBeInTheDocument();
  });
});

describe("dialogs and focus", () => {
  it("Escape closes the dialog even when focus fell back to the page", () => {
    const closed: string[] = [];
    render(<UpdateDialog updates={updatesWith(snapshot({ phase: "checking" }))} onClose={() => closed.push("x")} onOpenSettings={() => {}} />);
    (document.activeElement as HTMLElement | null)?.blur();
    expect(document.activeElement).toBe(document.body);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(closed).toEqual(["x"]);
  });
});
