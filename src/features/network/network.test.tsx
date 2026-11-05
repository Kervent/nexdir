import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { isNetworkPath, isUncPath, moveBlockedReason, needsSignIn, NETWORK_MOVE_REFUSED, serverOnly, statusTone } from "./networkPaths";
import { NetworkView, type NetworkActions } from "./NetworkView";
import { createPane, isValidSession, parentOf, sameLocation, type PaneState } from "../workspace/workspace";
import type { DriveInfo } from "../../types/fs";
import type { NetworkOverview } from "../../types/network";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => invoke(cmd, args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

describe("network paths", () => {
  it("recognises UNC paths and servers, not device paths", () => {
    expect(isUncPath("\\\\NAS\\Media")).toBe(true);
    expect(isUncPath("//nas/media/x")).toBe(true);
    expect(isUncPath("\\\\?\\UNC\\nas\\media")).toBe(true);
    expect(isUncPath("\\\\.\\pipe\\x")).toBe(false);
    expect(isUncPath("\\\\?\\C:\\x")).toBe(false);
    expect(isUncPath("\\\\NAS")).toBe(false);
    expect(isUncPath("C:\\Users")).toBe(false);
    expect(serverOnly("\\\\NAS-WOHNZIMMER")).toBe("NAS-WOHNZIMMER");
    expect(serverOnly(" \\\\nas\\ ")).toBe("nas");
    expect(serverOnly("\\\\nas\\media")).toBeNull();
    expect(serverOnly("\\\\.\\pipe")).toBeNull();
  });

  it("treats mapped network drives as network locations", () => {
    const drives = [
      { letter: "Z", kind: "network" },
      { letter: "C", kind: "fixed" },
    ] as DriveInfo[];
    expect(isNetworkPath("Z:\\Filme", drives)).toBe(true);
    expect(isNetworkPath("C:\\Users", drives)).toBe(false);
    expect(isNetworkPath("\\\\nas\\x", [])).toBe(true);
  });

  it("refuses moves across a share boundary before they start, allows renames inside a share", () => {
    const drives = [{ letter: "Z", kind: "network" }, { letter: "C", kind: "fixed" }, { letter: "D", kind: "fixed" }] as DriveInfo[];
    expect(moveBlockedReason(["\\\\NAS\\Media\\a.txt"], "C:\\Users\\me", drives)).toBe(NETWORK_MOVE_REFUSED);
    expect(moveBlockedReason(["C:\\x.txt"], "\\\\nas\\media", drives)).toBe(NETWORK_MOVE_REFUSED);
    expect(moveBlockedReason(["\\\\nas\\media\\a"], "\\\\nas\\backup", drives)).toBe(NETWORK_MOVE_REFUSED);
    expect(moveBlockedReason(["Z:\\a"], "C:\\b", drives)).toBe(NETWORK_MOVE_REFUSED);
    expect(moveBlockedReason(["\\\\NAS\\Media\\a"], "\\\\nas\\media\\sub", drives)).toBeNull();
    expect(moveBlockedReason(["Z:\\a"], "Z:\\b", drives)).toBeNull();
    expect(moveBlockedReason(["C:\\a"], "D:\\b", drives)).toBeNull();
  });

  it("maps statuses to tones and sign-in hints", () => {
    expect(statusTone("connected")).toBe("ok");
    expect(statusTone(null)).toBe("muted");
    expect(statusTone("signInRequired")).toBe("warn");
    expect(statusTone("unreachable")).toBe("error");
    expect(needsSignIn("accessDenied")).toBe(true);
    expect(needsSignIn("unreachable")).toBe(false);
  });
});

describe("network locations in the workspace", () => {
  const paneAt = (path: string): PaneState => createPane("p1", { type: "folder", path });

  it("goes up from a share root to the server's shares", () => {
    expect(parentOf(paneAt("\\\\NAS\\Media"))?.to).toEqual({ type: "network", server: "NAS" });
    expect(parentOf(paneAt("\\\\NAS\\Media\\Filme"))?.to).toEqual({ type: "folder", path: "\\\\NAS\\Media" });
    expect(parentOf(paneAt("C:\\"))?.to).toEqual({ type: "home" });
  });

  it("restores network locations from a saved session and compares them", () => {
    expect(isValidSession({ tabs: [{ locations: [{ type: "network" }, { type: "folder", path: "\\\\nas\\x" }] }], activeIndex: 0 })).toBe(true);
    expect(isValidSession({ tabs: [{ locations: [{ type: "network", server: 5 }] }], activeIndex: 0 })).toBe(false);
    expect(sameLocation({ type: "network", server: "NAS" }, { type: "network", server: "nas" })).toBe(true);
    expect(sameLocation({ type: "network" }, { type: "network", server: "nas" })).toBe(false);
  });
});

const overview: NetworkOverview = {
  warning: null,
  locations: [
    { path: "\\\\NAS-WOHNZIMMER\\Medien", server: "NAS-WOHNZIMMER", share: "Medien", label: "Medien", drive: "Z:", sources: ["drive"], status: "connected", managed: false },
    { path: "\\\\offline-nas\\Archiv", server: "offline-nas", share: "Archiv", label: "Archiv", drive: null, sources: ["favorite"], status: null, managed: false },
  ],
};

function actions(overrides: Partial<NetworkActions> = {}): NetworkActions {
  return {
    navigate: vi.fn(),
    openInNewTab: vi.fn(),
    openInOtherPane: vi.fn(),
    showProperties: vi.fn(),
    notify: vi.fn(),
    favorites: [{ path: "\\\\offline-nas\\Archiv", label: "" }],
    saveFavorites: vi.fn(async () => true),
    ...overrides,
  };
}

describe("Network view", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      switch (cmd) {
        case "network_overview":
          return overview;
        case "network_parse": {
          const input = String(args?.input ?? "");
          if (input === "\\\\NAS") return { path: "\\\\NAS", kind: "server", server: "NAS", share: null };
          if (input.startsWith("\\\\NAS\\Privat")) return { path: "\\\\NAS\\Privat", kind: "share", server: "NAS", share: "Privat" };
          throw { code: "invalidPath", message: "A network path starts with two backslashes, for example \\\\NAS\\Share.", path: null };
        }
        case "network_connect":
          return args?.signIn
            ? { path: "\\\\NAS\\Privat", shareRoot: "\\\\NAS\\Privat", report: { status: "connected", message: "ok", code: null }, managed: true, existing: false }
            : { path: "\\\\NAS\\Privat", shareRoot: "\\\\NAS\\Privat", report: { status: "signInRequired", message: "Sign-in required.", code: 1326 }, managed: false, existing: false };
        case "network_probe":
          return { status: "unreachable", message: "Not reachable.", code: 53 };
        case "network_discover":
          return 7;
        case "task_status":
          return null;
        default:
          throw new Error(`unexpected ${cmd}`);
      }
    });
  });

  it("shows network drives and saved locations without contacting servers", async () => {
    render(<NetworkView actions={actions()} />);
    expect(await screen.findByText("Medien (Z:)")).toBeInTheDocument();
    expect(screen.getByText("Archiv")).toBeInTheDocument();
    expect(screen.getByText("Connected")).toBeInTheDocument();
    expect(screen.getByText("Not checked")).toBeInTheDocument();
    expect(invoke.mock.calls.map((c) => c[0])).toEqual(["network_overview"]);
  });

  it("opens a mapped drive by its letter and offers the actions in a context menu", async () => {
    const a = actions();
    render(<NetworkView actions={a} />);
    fireEvent.click(await screen.findByText("Medien (Z:)"));
    expect(a.navigate).toHaveBeenCalledWith({ type: "folder", path: "Z:\\" });
    fireEvent.click(screen.getByRole("button", { name: "More actions for Archiv" }));
    const labels = screen.getAllByRole("menuitem").map((m) => m.textContent);
    expect(labels).toEqual(expect.arrayContaining(["Open", "Open in new tab", "Open in other pane", "Check connection", "Remove from saved", "Copy network path", "Properties"]));
    expect(labels).not.toContain("Disconnect");
    fireEvent.click(screen.getByRole("menuitem", { name: /Check connection/ }));
    expect(await screen.findByText("Not reachable", { selector: ".nx-status-chip" })).toBeInTheDocument();
  });

  it("starts a network search only when asked", async () => {
    render(<NetworkView actions={actions()} />);
    await screen.findByText("Medien (Z:)");
    expect(invoke).not.toHaveBeenCalledWith("network_discover", undefined);
    fireEvent.click(screen.getByRole("button", { name: /Search the network/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("network_discover", undefined));
  });

  it("connects manually and uses the Windows sign-in only after the user asks", async () => {
    const a = actions();
    render(<NetworkView actions={a} />);
    fireEvent.click(await screen.findByRole("button", { name: /Add network share/ }));
    const input = screen.getByLabelText("Server or network path");
    fireEvent.change(input, { target: { value: "C:\\Users" } });
    expect(await screen.findByText(/two backslashes/)).toBeInTheDocument();

    fireEvent.change(input, { target: { value: "\\\\NAS" } });
    expect(await screen.findByRole("button", { name: /Show shares/ })).toBeEnabled();

    fireEvent.change(input, { target: { value: "\\\\NAS\\Privat" } });
    await screen.findByText(/Share: \\\\NAS\\Privat/);
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByText("Sign-in required.")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("network_connect", { path: "\\\\NAS\\Privat", signIn: false });
    expect(invoke).not.toHaveBeenCalledWith("network_connect", { path: "\\\\NAS\\Privat", signIn: true });

    fireEvent.click(screen.getByRole("button", { name: /Sign in/ }));
    await waitFor(() => expect(a.navigate).toHaveBeenCalledWith({ type: "folder", path: "\\\\NAS\\Privat" }));
    expect(invoke).toHaveBeenCalledWith("network_connect", { path: "\\\\NAS\\Privat", signIn: true });
    // Saved (default on) with path and name only.
    expect(a.saveFavorites).toHaveBeenCalledWith([{ path: "\\\\offline-nas\\Archiv", label: "" }, { path: "\\\\NAS\\Privat", label: "" }]);
  });

  it("offers Sign in… again after Windows' dialog was cancelled", async () => {
    let signIns = 0;
    const base = invoke.getMockImplementation()!;
    invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "network_connect" && args?.signIn && ++signIns === 1)
        return { path: "\\\\NAS\\Privat", shareRoot: "\\\\NAS\\Privat", report: { status: "cancelled", message: "Sign-in was cancelled.", code: 1223 }, managed: false, existing: false };
      return base(cmd, args);
    });
    const a = actions();
    render(<NetworkView actions={a} />);
    fireEvent.click(await screen.findByRole("button", { name: /Add network share/ }));
    fireEvent.change(screen.getByLabelText("Server or network path"), { target: { value: "\\\\NAS\\Privat" } });
    await screen.findByText(/Share: \\\\NAS\\Privat/);
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    fireEvent.click(await screen.findByRole("button", { name: /Sign in/ }));
    expect(await screen.findByText("Sign-in was cancelled.")).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: /Sign in/ }));
    await waitFor(() => expect(a.navigate).toHaveBeenCalledWith({ type: "folder", path: "\\\\NAS\\Privat" }));
    expect(signIns).toBe(2);
  });

  it("renames a saved location (name only, path unchanged)", async () => {
    const a = actions();
    render(<NetworkView actions={a} />);
    fireEvent.click(await screen.findByRole("button", { name: "More actions for Archiv" }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Rename/ }));
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Altes NAS" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(a.saveFavorites).toHaveBeenCalledWith([{ path: "\\\\offline-nas\\Archiv", label: "Altes NAS" }]));
    // Only saved locations can be renamed.
    fireEvent.click(screen.getByRole("button", { name: "More actions for Medien (Z:)" }));
    expect(screen.queryByRole("menuitem", { name: /Rename/ })).toBeNull();
  });

  it("shows an empty state with the two main actions", async () => {
    invoke.mockImplementation(async (cmd: string) => (cmd === "network_overview" ? { locations: [], warning: null } : null));
    render(<NetworkView actions={actions({ favorites: [] })} />);
    expect(await screen.findByText("No network storage found")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /Add network share/ })).toHaveLength(2);
  });
});
