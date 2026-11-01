import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { App } from "./App";
import { ToastProvider } from "../components/Toasts";
import type { DirListing, FileEntry } from "../types/fs";

// The backend is replaced by an in-memory fake; the UI only talks to it through invoke().
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => invoke(cmd, args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const file = (dir: string, name: string, kind: FileEntry["kind"] = "file"): FileEntry => ({
  name,
  path: `${dir}\\${name}`,
  kind,
  size: kind === "file" ? 2048 : null,
  modified: Date.UTC(2026, 0, 2),
  created: null,
  extension: kind === "file" && name.includes(".") ? name.split(".").pop()! : null,
  hidden: false,
  system: false,
  readonly: false,
});

const folders: Record<string, DirListing> = {
  "C:\\Users\\test\\Documents": {
    path: "C:\\Users\\test\\Documents",
    parent: "C:\\Users\\test",
    entries: [file("C:\\Users\\test\\Documents", "Projects", "dir"), file("C:\\Users\\test\\Documents", "notes.txt"), file("C:\\Users\\test\\Documents", "setup.exe")],
    unreadable: 0,
    hiddenCount: 0,
  },
  "C:\\Users\\test\\Documents\\Projects": {
    path: "C:\\Users\\test\\Documents\\Projects",
    parent: "C:\\Users\\test\\Documents",
    entries: [file("C:\\Users\\test\\Documents\\Projects", "plan.md")],
    unreadable: 0,
    hiddenCount: 2,
  },
};

let lastTask = "";
const dupeHash = "a".repeat(64);
const duplicateReport = {
  groups: [
    {
      size: 2048,
      hash: dupeHash,
      files: [
        { path: "C:\\Users\\test\\Documents\\a.bin", name: "a.bin", modified: 1 },
        { path: "C:\\Users\\test\\Documents\\b.bin", name: "b.bin", modified: 2 },
        { path: "C:\\Users\\test\\Documents\\c.bin", name: "c.bin", modified: 3 },
      ],
    },
  ],
  wastedBytes: 4096,
  filesScanned: 3,
  bytesHashed: 6144,
  unreadable: 0,
  truncated: false,
  complete: true,
};
const storageReport = {
  root: "C:\\Users\\test\\Documents\\Projects",
  totalBytes: 4096,
  fileCount: 2,
  folderCount: 1,
  unreadable: 0,
  skippedLinks: 0,
  children: [{ name: "plan.md", path: "C:\\Users\\test\\Documents\\Projects\\plan.md", bytes: 4096, files: 1, isDir: false }],
  otherChildren: 0,
  otherChildrenBytes: 0,
  largestFiles: [{ name: "plan.md", path: "C:\\Users\\test\\Documents\\Projects\\plan.md", bytes: 4096, modified: 0 }],
  largestFolders: [],
  categories: [{ key: "document", bytes: 4096, files: 1 }],
  extensions: [{ key: "md", bytes: 4096, files: 1 }],
  volume: null,
  complete: true,
};

function backend(cmd: string, args: Record<string, unknown> = {}) {
  switch (cmd) {
    case "known_folders":
      return [{ id: "documents", label: "Documents", path: "C:\\Users\\test\\Documents" }];
    case "list_drives":
      return [{ path: "C:\\", letter: "C", label: "Local Disk", kind: "fixed", fileSystem: "NTFS", totalBytes: 100, freeBytes: 40, available: true }];
    case "list_dir": {
      const listing = folders[args.path as string];
      if (!listing) throw { code: "permissionDenied", message: "Access denied. You may not have permission for this item.", path: args.path };
      return listing;
    }
    case "list_operations":
      return [];
    case "startup_location":
    case "clipboard_get_files":
    case "clipboard_set_files":
      return null;
    case "open_path":
    case "create_folder":
      return null;
    case "rename_entry": {
      const path = args.path as string;
      return `${path.slice(0, path.lastIndexOf("\\"))}\\${args.newName as string}`;
    }
    case "find_conflicts":
      return { conflicts: [], sameFolder: false };
    case "start_operation":
      return 1;
    case "start_storage_scan":
    case "start_search":
    case "start_duplicate_scan":
      lastTask = cmd;
      return 7;
    case "task_status":
      return { id: 7, kind: "search", state: "completed", phase: "Done", items: 3, bytes: 0, total: null, done: 1, current: null, error: null, startedAt: 0, finishedAt: 1 };
    case "task_result":
      if (lastTask === "start_storage_scan") return storageReport;
      if (lastTask === "start_duplicate_scan") return duplicateReport;
      return { entries: [file("C:\\Users\\test\\Documents", "notes.txt")], scanned: 3, truncated: false, complete: true, source: "live" };
    case "index_status":
      return { exists: false, files: 0, lastBuilt: null, sizeBytes: 0, folders: [], includeContent: false };
    case "rules_get":
      return [];
    case "organize_preview":
      return { items: [{ source: "C:\\Users\\test\\Downloads\\a.pdf", target: "C:\\Users\\test\\Documents\\PDF\\a.pdf", size: 10, conflict: false }], scanned: 4, truncated: false, destinationExists: false };
    case "organize_apply":
      return 3;
    case "preview_path":
      return { entry: file("C:\\Users\\test\\Documents", "notes.txt"), content: { type: "text", text: "hello <b>world</b>", truncated: false } };
    case "recycle_entries":
      return { recycled: args.paths, failed: [] };
    case "recycle_duplicates":
      return { recycled: (args.groups as { remove: string[] }[]).flatMap((g) => g.remove), failed: [] };
    case "system_info":
      return { appVersion: "0.4.3", osName: "Windows 11", osVersion: "25H2", osBuild: "26300.1", architecture: "x86_64" };
    case "open_url":
      return null;
    default:
      throw new Error(`unexpected command ${cmd}`);
  }
}

function renderApp() {
  return render(
    <ToastProvider>
      <App />
    </ToastProvider>,
  );
}

async function openDocuments() {
  renderApp();
  const nav = await screen.findByRole("navigation", { name: "Navigation" });
  fireEvent.click(await within(nav).findByRole("button", { name: "Documents" }));
  return screen.findByRole("listbox", { name: "Files and folders" });
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string, args: Record<string, unknown>) => backend(cmd, args));
  localStorage.clear();
  // The first-start welcome is tested on its own below; other tests start as a returning user.
  localStorage.setItem("nexdir.onboarding.v1", JSON.stringify({ version: 1, status: "completed", offeredAt: "2026-01-01T00:00:00Z" }));
});

async function openHelpMenu() {
  fireEvent.click(await screen.findByRole("button", { name: "Help" }));
  return screen.findByRole("menu");
}

describe("App", () => {
  it("shows drives and quick access on the home page", async () => {
    renderApp();
    expect(await screen.findByText("Local Disk (C:)", { selector: ".nx-card-title" })).toBeInTheDocument();
    expect(screen.getByRole("meter", { name: "60% used" })).toBeInTheDocument();
  });

  it("navigates into folders, back and up", async () => {
    const list = await openDocuments();
    expect(within(list).getByText("notes.txt")).toBeInTheDocument();
    // Folders are listed first.
    const options = within(list).getAllByRole("option");
    expect(options[0]).toHaveTextContent("Projects");

    fireEvent.doubleClick(within(list).getByText("Projects"));
    expect(await screen.findByText("plan.md")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Up/ }));
    expect(await screen.findByText("notes.txt")).toBeInTheDocument();
    // Going up selects the folder we came from.
    await waitFor(() => expect(screen.getByRole("option", { name: /Projects/ })).toHaveAttribute("aria-selected", "true"));

    fireEvent.click(screen.getByRole("button", { name: /^Back/ }));
    expect(await screen.findByText("plan.md")).toBeInTheDocument();
  });

  it("supports multi-selection with ctrl and shift", async () => {
    const list = await openDocuments();
    const options = () => within(list).getAllByRole("option");
    fireEvent.mouseDown(options()[0]!, { button: 0 });
    fireEvent.mouseDown(options()[2]!, { button: 0, shiftKey: true });
    expect(options().filter((o) => o.getAttribute("aria-selected") === "true")).toHaveLength(3);
    fireEvent.mouseDown(options()[1]!, { button: 0, ctrlKey: true });
    expect(options().filter((o) => o.getAttribute("aria-selected") === "true")).toHaveLength(2);
    expect(screen.getByText(/2 items selected/)).toBeInTheDocument();
  });

  it("opens regular files directly but asks before running programs", async () => {
    const list = await openDocuments();
    fireEvent.doubleClick(within(list).getByText("notes.txt"));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_path", { path: "C:\\Users\\test\\Documents\\notes.txt", allowExecutable: false }));

    fireEvent.doubleClick(within(list).getByText("setup.exe"));
    const dialog = await screen.findByRole("dialog", { name: "Run this file?" });
    expect(invoke).not.toHaveBeenCalledWith("open_path", expect.objectContaining({ allowExecutable: true }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Run" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_path", { path: "C:\\Users\\test\\Documents\\setup.exe", allowExecutable: true }));
  });

  it("asks for confirmation before moving items to the Recycle Bin", async () => {
    const list = await openDocuments();
    fireEvent.mouseDown(within(list).getByText("notes.txt"), { button: 0 });
    fireEvent.keyDown(window, { key: "Delete" });
    const dialog = await screen.findByRole("dialog", { name: "Move to Recycle Bin" });
    expect(invoke).not.toHaveBeenCalledWith("recycle_entries", expect.anything());
    fireEvent.click(within(dialog).getByRole("button", { name: "Move to Recycle Bin" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("recycle_entries", { paths: ["C:\\Users\\test\\Documents\\notes.txt"] }));
  });

  it("validates names before renaming", async () => {
    const list = await openDocuments();
    fireEvent.mouseDown(within(list).getByText("notes.txt"), { button: 0 });
    fireEvent.keyDown(window, { key: "F2" });
    const dialog = await screen.findByRole("dialog", { name: /Rename/ });
    const input = within(dialog).getByRole("textbox");
    fireEvent.change(input, { target: { value: "bad:name.txt" } });
    expect(within(dialog).getByRole("alert")).toHaveTextContent("cannot contain");
    expect(within(dialog).getByRole("button", { name: "Rename" })).toBeDisabled();
    fireEvent.change(input, { target: { value: "renamed.txt" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Rename" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("rename_entry", { path: "C:\\Users\\test\\Documents\\notes.txt", newName: "renamed.txt" }));
  });

  it("shows a readable error for inaccessible folders", async () => {
    renderApp();
    await screen.findByText("Local Disk (C:)", { selector: ".nx-card-title" });
    fireEvent.click(within(screen.getByRole("navigation", { name: "Navigation" })).getByRole("button", { name: "Local Disk (C:)" }));
    expect(await screen.findByRole("heading", { name: "Access denied" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go to Home" })).toBeInTheDocument();
  });

  it("filters the current folder", async () => {
    const list = await openDocuments();
    act(() => {
      fireEvent.change(screen.getByRole("searchbox", { name: /Filter/ }), { target: { value: "notes" } });
    });
    await waitFor(() => expect(within(list).getAllByRole("option")).toHaveLength(1));
  });

  it("switches theme and persists preferences", async () => {
    renderApp();
    const button = await screen.findByRole("button", { name: "Theme: system" });
    fireEvent.click(button);
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    fireEvent.click(screen.getByRole("button", { name: "Theme: light" }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(JSON.parse(localStorage.getItem("nexdir.preferences.v1")!).theme).toBe("dark");
  });
});

describe("Phase 2: tabs, split view, clipboard, drag and drop, preview", () => {
  const tabs = () => screen.getAllByRole("tab");

  it("opens, switches and closes tabs with the keyboard", async () => {
    await openDocuments();
    expect(tabs()).toHaveLength(1);
    expect(tabs()[0]).toHaveTextContent("Documents");
    fireEvent.keyDown(window, { key: "t", ctrlKey: true });
    await waitFor(() => expect(tabs()).toHaveLength(2));
    // A new tab opens on the current folder and becomes active.
    expect(tabs()[1]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(window, { key: "Tab", ctrlKey: true });
    expect(tabs()[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(window, { key: "w", ctrlKey: true });
    await waitFor(() => expect(tabs()).toHaveLength(1));
  });

  it("opens folders in a background tab with the middle mouse button", async () => {
    const list = await openDocuments();
    fireEvent.mouseDown(within(list).getByText("Projects"), { button: 1 });
    await waitFor(() => expect(tabs()).toHaveLength(2));
    expect(tabs()[1]).toHaveTextContent("Projects");
    expect(tabs()[0]).toHaveAttribute("aria-selected", "true");
  });

  it("restores open tabs from the previous session", async () => {
    localStorage.setItem(
      "nexdir.session.v1",
      JSON.stringify({ tabs: [{ locations: [{ type: "home" }] }, { locations: [{ type: "folder", path: "C:\\Users\\test\\Documents\\Projects" }] }], activeIndex: 1 }),
    );
    renderApp();
    expect(await screen.findByText("plan.md")).toBeInTheDocument();
    expect(tabs()).toHaveLength(2);
  });

  it("splits the view and copies to the other pane", async () => {
    await openDocuments();
    fireEvent.click(screen.getByRole("button", { name: /^Split view/ }));
    await waitFor(() => expect(screen.getAllByRole("listbox", { name: "Files and folders" })).toHaveLength(2));
    const [left, right] = screen.getAllByRole("listbox", { name: "Files and folders" });
    // Navigate the right pane into Projects, then copy a file from the left pane there.
    fireEvent.doubleClick(within(right!).getByText("Projects"));
    await within(screen.getAllByRole("listbox", { name: "Files and folders" })[1]!).findByText("plan.md");
    const leftList = screen.getAllByRole("listbox", { name: "Files and folders" })[0]!;
    expect(leftList).toBe(left);
    fireEvent.mouseDown(within(leftList).getByText("notes.txt"), { button: 0 });
    fireEvent.contextMenu(within(leftList).getByText("notes.txt"));
    fireEvent.click(await screen.findByRole("menuitem", { name: /Copy to other pane/ }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("start_operation", {
        kind: "copy",
        sources: ["C:\\Users\\test\\Documents\\notes.txt"],
        destination: "C:\\Users\\test\\Documents\\Projects",
        policy: "skip",
      }),
    );
  });

  it("copies to and pastes from the Windows clipboard", async () => {
    const list = await openDocuments();
    fireEvent.mouseDown(within(list).getByText("notes.txt"), { button: 0 });
    fireEvent.keyDown(window, { key: "x", ctrlKey: true });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("clipboard_set_files", { paths: ["C:\\Users\\test\\Documents\\notes.txt"], cut: true }));
    // Cut items are dimmed until pasted.
    expect(within(list).getByRole("option", { name: /notes\.txt/ })).toHaveClass("is-cut");

    // Files copied in Explorer arrive through the system clipboard.
    invoke.mockImplementation(async (cmd: string, args: Record<string, unknown>) =>
      cmd === "clipboard_get_files" ? { paths: ["D:\\photos\\a.jpg"], cut: false } : backend(cmd, args),
    );
    fireEvent.keyDown(window, { key: "v", ctrlKey: true });
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("start_operation", { kind: "copy", sources: ["D:\\photos\\a.jpg"], destination: "C:\\Users\\test\\Documents", policy: "skip" }),
    );
  });

  it("moves items by dragging them onto a folder", async () => {
    const list = await openDocuments();
    const source = within(list).getByRole("option", { name: /notes\.txt/ });
    const target = within(list).getByRole("option", { name: /Projects/ });
    const dataTransfer = { setData: vi.fn(), effectAllowed: "", dropEffect: "" };
    fireEvent.dragStart(source, { dataTransfer });
    fireEvent.dragOver(target, { dataTransfer });
    expect(target).toHaveClass("is-drop-target");
    fireEvent.drop(target, { dataTransfer });
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("start_operation", {
        kind: "move",
        sources: ["C:\\Users\\test\\Documents\\notes.txt"],
        destination: "C:\\Users\\test\\Documents\\Projects",
        policy: "skip",
      }),
    );
  });

  it("does not allow dropping a folder into itself", async () => {
    const list = await openDocuments();
    const folder = within(list).getByRole("option", { name: /Projects/ });
    const dataTransfer = { setData: vi.fn(), effectAllowed: "", dropEffect: "" };
    fireEvent.dragStart(folder, { dataTransfer });
    fireEvent.dragOver(folder, { dataTransfer });
    expect(folder).not.toHaveClass("is-drop-target");
    fireEvent.drop(folder, { dataTransfer });
    expect(invoke).not.toHaveBeenCalledWith("start_operation", expect.anything());
  });

  it("previews text as plain text, never as HTML", async () => {
    const list = await openDocuments();
    fireEvent.click(screen.getByRole("button", { name: /^Preview pane/ }));
    fireEvent.mouseDown(within(list).getByText("notes.txt"), { button: 0 });
    const preview = await screen.findByRole("complementary", { name: "Preview" });
    expect(await within(preview).findByText("hello <b>world</b>")).toBeInTheDocument();
    expect(preview.querySelector("b")).toBeNull();
  });

  it("undoes the last rename with Ctrl+Z", async () => {
    const list = await openDocuments();
    fireEvent.mouseDown(within(list).getByText("notes.txt"), { button: 0 });
    fireEvent.keyDown(window, { key: "F2" });
    const dialog = await screen.findByRole("dialog", { name: /Rename/ });
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "todo.txt" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Rename" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("rename_entry", { path: "C:\\Users\\test\\Documents\\todo.txt", newName: "notes.txt" }));
  });

  it("lists keyboard shortcuts with F1", async () => {
    renderApp();
    await screen.findByRole("navigation", { name: "Navigation" });
    fireEvent.keyDown(window, { key: "F1" });
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toHaveTextContent("Toggle split view");
  });

  it("shows the app version in About", async () => {
    renderApp();
    await screen.findByRole("navigation", { name: "Navigation" });
    fireEvent.click(within(await openHelpMenu()).getByRole("menuitem", { name: /About Nexdir/ }));
    expect(await screen.findByRole("dialog", { name: "About Nexdir" })).toHaveTextContent(`Version ${__APP_VERSION__}`);
    expect(__APP_VERSION__).toMatch(/^\d+\.\d+\.\d+/);
  });
});

describe("Phase 3: tools", () => {
  it("opens Search from the sidebar, shows the interpretation and runs a live search", async () => {
    renderApp();
    const nav = await screen.findByRole("navigation", { name: "Navigation" });
    fireEvent.click(within(nav).getByRole("button", { name: "Search" }));
    const input = await screen.findByRole("searchbox", { name: "Search request" });
    fireEvent.change(input, { target: { value: "pdf files larger than 1 MB" } });
    expect(screen.getByText("Size > 1 MB")).toBeInTheDocument();
    expect(screen.getByText("Type: .pdf")).toBeInTheDocument();
    fireEvent.submit(input.closest("form")!);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("start_search", { query: expect.objectContaining({ extensions: ["pdf"], minSize: 1024 ** 2, recursive: true }) }),
    );
    expect(await screen.findByText("1 result · 3 items checked")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /notes\.txt/ })).toBeInTheDocument();
  });

  it("starts a storage analysis for a folder from the context menu", async () => {
    const list = await openDocuments();
    fireEvent.contextMenu(within(list).getByText("Projects"));
    fireEvent.click(await screen.findByRole("menuitem", { name: /Analyze storage/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("start_storage_scan", { path: "C:\\Users\\test\\Documents\\Projects" }));
    // Opens in a new tab with the report.
    expect(await screen.findByText("in this folder")).toBeInTheDocument();
    expect(screen.getAllByRole("tab", { hidden: true })).toHaveLength(2);
  });

  it("previews an organisation rule and applies only after confirmation", async () => {
    renderApp();
    const nav = await screen.findByRole("navigation", { name: "Navigation" });
    fireEvent.click(within(nav).getByRole("button", { name: "Organize" }));
    fireEvent.click(await screen.findByRole("button", { name: /New rule/ }));
    fireEvent.click(screen.getByRole("button", { name: /Preview \(dry run\)/ }));
    expect(await screen.findByText(/Nothing has been changed yet/)).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("organize_apply", expect.anything());
    fireEvent.click(screen.getByRole("button", { name: "Move 1 file" }));
    const dialog = await screen.findByRole("dialog", { name: "Apply rule?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Move files" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("organize_apply", expect.objectContaining({ sources: ["C:\\Users\\test\\Downloads\\a.pdf"] })));
  });
});

describe("0.4.3: duplicate removal goes through the backend guard", () => {
  async function scan() {
    renderApp();
    const nav = await screen.findByRole("navigation", { name: "Navigation" });
    fireEvent.click(within(nav).getByRole("button", { name: "Duplicate files" }));
    fireEvent.change(await screen.findByLabelText("Folder"), { target: { value: "C:\\Users\\test\\Documents" } });
    fireEvent.click(screen.getByRole("button", { name: "Find duplicates" }));
    return screen.findByText("C:\\Users\\test\\Documents\\a.bin");
  }

  it("sends kept and removed copies per group to recycle_duplicates", async () => {
    await scan();
    fireEvent.click(screen.getByRole("button", { name: "Select all but the oldest" }));
    fireEvent.click(screen.getByRole("button", { name: /Move 2 files to Recycle Bin/ }));
    const dialog = await screen.findByRole("dialog", { name: "Move to Recycle Bin" });
    expect(dialog).toHaveTextContent("At least one copy of every file is kept");
    fireEvent.click(within(dialog).getByRole("button", { name: "Move to Recycle Bin" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("recycle_duplicates", {
        groups: [{ size: 2048, hash: dupeHash, keep: ["C:\\Users\\test\\Documents\\a.bin"], remove: ["C:\\Users\\test\\Documents\\b.bin", "C:\\Users\\test\\Documents\\c.bin"] }],
      }),
    );
    expect(invoke).not.toHaveBeenCalledWith("recycle_entries", expect.anything());
  });

  it("cannot remove every copy and offers no unguarded delete in the context menu", async () => {
    await scan();
    for (const name of ["a.bin", "b.bin", "c.bin"]) fireEvent.click(screen.getByRole("checkbox", { name: new RegExp(name) }));
    expect(screen.getByRole("button", { name: /Move 3 files to Recycle Bin/ })).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("Every copy is selected");
    fireEvent.contextMenu(screen.getByText("C:\\Users\\test\\Documents\\a.bin"));
    const menu = await screen.findByRole("menu");
    expect(within(menu).queryByRole("menuitem", { name: /Recycle Bin/ })).toBeNull();
    expect(within(menu).getByRole("menuitem", { name: /Copy path/ })).toBeInTheDocument();
  });
});

describe("0.4.3: first start and tour", () => {
  beforeEach(() => localStorage.removeItem("nexdir.onboarding.v1"));

  it("offers the tour on first start; skipping keeps it away on the next start", async () => {
    const { unmount } = renderApp();
    const welcome = await screen.findByRole("dialog", { name: "Welcome to Nexdir" });
    expect(within(welcome).getByRole("button", { name: /Start Tutorial/ })).toHaveFocus();
    fireEvent.click(within(welcome).getByRole("button", { name: "Skip for Now" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(JSON.parse(localStorage.getItem("nexdir.onboarding.v1")!)).toMatchObject({ version: 1, status: "skipped" });
    unmount();
    renderApp();
    await screen.findByRole("navigation", { name: "Navigation" });
    expect(screen.queryByRole("dialog", { name: "Welcome to Nexdir" })).toBeNull();
  });

  it("Escape on the welcome counts as skip, and the app stays usable", async () => {
    renderApp();
    const welcome = await screen.findByRole("dialog", { name: "Welcome to Nexdir" });
    fireEvent.keyDown(welcome, { key: "Escape" });
    expect(JSON.parse(localStorage.getItem("nexdir.onboarding.v1")!).status).toBe("skipped");
    fireEvent.keyDown(window, { key: "F1" });
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
  });

  it("runs the tour over the real interface and records completion", async () => {
    renderApp();
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Welcome to Nexdir" })).getByRole("button", { name: /Start Tutorial/ }));
    expect(await screen.findByRole("dialog", { name: "Find your way around" })).toBeInTheDocument();
    // The highlighted elements exist in the app.
    for (const t of ["sidebar", "address", "tabs", "split", "file-ops", "operations", "tools", "appearance", "help"]) {
      expect(document.querySelector(`[data-tour~="${t}"]`), t).not.toBeNull();
    }
    // App shortcuts are paused while the tour runs.
    fireEvent.keyDown(window, { key: "t", ctrlKey: true });
    expect(screen.getAllByRole("tab")).toHaveLength(1);
    for (let i = 0; i < 4; i++) fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    fireEvent.click(screen.getByRole("button", { name: /Finish/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Get Started" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(JSON.parse(localStorage.getItem("nexdir.onboarding.v1")!).status).toBe("completed");
  });

  it("can be restarted from Help → Take a Tour", async () => {
    localStorage.setItem("nexdir.onboarding.v1", JSON.stringify({ version: 1, status: "completed", offeredAt: "x" }));
    renderApp();
    fireEvent.click(within(await openHelpMenu()).getByRole("menuitem", { name: /Take a Tour/ }));
    expect(await screen.findByRole("dialog", { name: "Find your way around" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Skip tour" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("0.4.3: help and support", () => {
  it("shows offline help and opens only project links after a click", async () => {
    renderApp();
    fireEvent.click(within(await openHelpMenu()).getByRole("menuitem", { name: /Help & Support/ }));
    const help = await screen.findByRole("dialog", { name: "Help & Support" });
    expect(invoke).not.toHaveBeenCalledWith("open_url", expect.anything());
    for (const topic of ["Navigation", "File operations", "Search", "Duplicate files", "Storage analysis", "Keyboard shortcuts", "Common problems"]) {
      expect(within(help).getByText(topic, { selector: "summary" })).toBeInTheDocument();
    }
    expect(within(help).getByText(/Del moves items to the Recycle Bin/)).toBeInTheDocument(); // bundled text, no network
    // No invite configured in this build: the Discord entry is disabled and nothing breaks.
    expect(within(help).getByRole("button", { name: /Discord Community/ })).toBeDisabled();
    fireEvent.click(within(help).getByRole("button", { name: /Documentation/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_url", { url: "https://github.com/Kervent/nexdir/tree/main/docs" }));
    fireEvent.click(within(help).getByRole("button", { name: /GitHub Repository/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_url", { url: "https://github.com/Kervent/nexdir" }));
  });

  it("reports a browser that cannot be opened (e.g. offline) without crashing", async () => {
    invoke.mockImplementation(async (cmd: string, args: Record<string, unknown>) => {
      if (cmd === "open_url") throw { code: "io", message: "Could not open the browser: no handler" };
      return backend(cmd, args);
    });
    renderApp();
    fireEvent.click(within(await openHelpMenu()).getByRole("menuitem", { name: /GitHub Repository/ }));
    expect(await screen.findByText("Could not open the browser.")).toBeInTheDocument();
  });

  it("help menu keeps the Discord entry disabled without an invite", async () => {
    renderApp();
    const menu = await openHelpMenu();
    expect(within(menu).getByRole("menuitem", { name: /Discord Community/ })).toBeDisabled();
  });
});

describe("0.4.3: bug reports and feature requests", () => {
  async function openBugReport() {
    renderApp();
    fireEvent.click(within(await openHelpMenu()).getByRole("menuitem", { name: /Report a Bug/ }));
    return screen.findByRole("dialog", { name: "Report a Bug" });
  }

  it("validates, reviews with redactions and opens the prefilled GitHub form", async () => {
    let dialog = await openBugReport();
    // Opening the form talks to nobody but the local backend.
    expect(invoke).toHaveBeenCalledWith("system_info", undefined);
    expect(invoke).not.toHaveBeenCalledWith("open_url", expect.anything());
    fireEvent.click(within(dialog).getByRole("button", { name: "Review report" }));
    expect(within(dialog).getAllByText("This field is required.").length).toBeGreaterThan(0);
    expect(within(dialog).getByText(/at least 6 characters/)).toBeInTheDocument();

    fireEvent.change(within(dialog).getByPlaceholderText("Short summary of the problem"), { target: { value: "Crash in C:\\Users\\test\\Secret Folder\\x.txt" } });
    fireEvent.change(within(dialog).getByLabelText("What happened?"), { target: { value: "It crashed. Mail me: test@example.com" } });
    fireEvent.change(within(dialog).getByLabelText("Steps to reproduce"), { target: { value: "1. Open it" } });
    fireEvent.change(within(dialog).getByLabelText("Expected behaviour"), { target: { value: "No crash" } });
    fireEvent.change(within(dialog).getByLabelText("Actual behaviour"), { target: { value: "Crash" } });
    fireEvent.change(within(dialog).getByLabelText(/data lost/), { target: { value: "No" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Review report" }));

    dialog = await screen.findByRole("dialog", { name: "Report a Bug: review" });
    expect(within(dialog).getByText(/Nothing has been sent/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Nexdir version: 0.4.3/)).toBeInTheDocument();
    expect(within(dialog).getByText(/replaced 1 file path and 1 e-mail address/)).toBeInTheDocument();
    const preview = within(dialog).getByLabelText("Report preview");
    expect(preview).not.toHaveTextContent("Secret Folder");
    expect(preview).not.toHaveTextContent("example.com");
    expect(preview).toHaveTextContent("Windows 11 25H2 (build 26300.1), x86_64");

    // The user removes the architecture before opening GitHub.
    fireEvent.click(within(dialog).getByRole("checkbox", { name: /Architecture/ }));
    expect(preview).not.toHaveTextContent("x86_64");
    fireEvent.click(within(dialog).getByRole("button", { name: /Open GitHub/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_url", expect.anything()));
    const url = new URL((invoke.mock.calls.find((c) => c[0] === "open_url")![1] as { url: string }).url);
    expect(url.origin + url.pathname).toBe("https://github.com/Kervent/nexdir/issues/new");
    expect(url.searchParams.get("template")).toBe("bug_report.yml");
    expect(url.searchParams.get("title")).toBe("[Bug] Crash in <path removed>");
    expect(url.searchParams.get("windows")).toBe("Windows 11 25H2 (build 26300.1)");
    expect(url.searchParams.get("description")).toBe("It crashed. Mail me: <email removed>");
    expect(await screen.findByText("GitHub opened in your browser.")).toBeInTheDocument();
  });

  it("cancelling sends nothing", async () => {
    const dialog = await openBugReport();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(invoke).not.toHaveBeenCalledWith("open_url", expect.anything());
  });

  it("copies long reports to the clipboard and opens the form with only the title", async () => {
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    const dialog = await openBugReport();
    fireEvent.change(within(dialog).getByPlaceholderText("Short summary of the problem"), { target: { value: "Very long report" } });
    fireEvent.change(within(dialog).getByLabelText("What happened?"), { target: { value: "lorem ipsum ".repeat(700) } });
    for (const label of ["Steps to reproduce", "Expected behaviour", "Actual behaviour"]) fireEvent.change(within(dialog).getByLabelText(label), { target: { value: "x" } });
    fireEvent.change(within(dialog).getByLabelText(/data lost/), { target: { value: "Not sure" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Review report" }));
    const review = await screen.findByRole("dialog", { name: "Report a Bug: review" });
    expect(within(review).getByText(/too long to pass to GitHub directly/)).toBeInTheDocument();
    fireEvent.click(within(review).getByRole("button", { name: /Open GitHub/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_url", expect.anything()));
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("### What happened?"));
    const url = new URL((invoke.mock.calls.find((c) => c[0] === "open_url")![1] as { url: string }).url);
    expect(url.searchParams.get("description")).toBeNull();
    expect(url.searchParams.get("title")).toBe("[Bug] Very long report");
    expect(await screen.findByText(/report is on the clipboard/)).toBeInTheDocument();
  });

  it("opens the feature request form with its own fields", async () => {
    renderApp();
    fireEvent.click(within(await openHelpMenu()).getByRole("menuitem", { name: /Request a Feature/ }));
    const dialog = await screen.findByRole("dialog", { name: "Request a Feature" });
    fireEvent.change(within(dialog).getByPlaceholderText("Short summary of the idea"), { target: { value: "Vertical tabs" } });
    fireEvent.change(within(dialog).getByLabelText("Problem description"), { target: { value: "Many tabs" } });
    fireEvent.change(within(dialog).getByLabelText("Proposed solution"), { target: { value: "Side tabs" } });
    fireEvent.change(within(dialog).getByLabelText("Expected benefit"), { target: { value: "Overview" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Review report" }));
    const review = await screen.findByRole("dialog", { name: "Request a Feature: review" });
    fireEvent.click(within(review).getByRole("button", { name: /Open GitHub/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_url", expect.anything()));
    const url = new URL((invoke.mock.calls.find((c) => c[0] === "open_url")![1] as { url: string }).url);
    expect(url.searchParams.get("template")).toBe("feature_request.yml");
    expect(url.searchParams.get("benefit")).toBe("Overview");
    expect(url.searchParams.get("context")).toBe("Nexdir version: 0.4.3");
  });
});
