import { describe, expect, it } from "vitest";
import { formatSize, percent, pluralize } from "./format";
import { categoryOf, isExecutable, typeLabel } from "./fileTypes";
import { baseName, breadcrumbs, samePath, uniqueName, validateName } from "./paths";
import { filterEntries, sortEntries } from "./sort";
import { clickSelect, emptySelection, moveFocus, pruneSelection, selectAll } from "./selection";
import type { FileEntry } from "../types/fs";

const entry = (name: string, patch: Partial<FileEntry> = {}): FileEntry => ({
  name,
  path: `C:\\t\\${name}`,
  kind: "file",
  size: 0,
  modified: 0,
  created: 0,
  extension: name.includes(".") ? name.split(".").pop()!.toLowerCase() : null,
  hidden: false,
  system: false,
  readonly: false,
  ...patch,
});

describe("format", () => {
  it("formats sizes like Windows", () => {
    expect(formatSize(0)).toBe("0 B");
    expect(formatSize(1023)).toBe("1023 B");
    expect(formatSize(1024)).toBe("1.00 KB");
    expect(formatSize(1536 * 1024)).toBe("1.50 MB");
    expect(formatSize(5 * 1024 ** 3)).toBe("5.00 GB");
    expect(formatSize(null)).toBe("");
  });
  it("pluralizes and clamps percentages", () => {
    expect(pluralize(1, "item")).toBe("1 item");
    expect(pluralize(2, "item")).toBe("2 items");
    expect(percent(5, 10)).toBe(50);
    expect(percent(5, 0)).toBe(0);
    expect(percent(20, 10)).toBe(100);
  });
});

describe("fileTypes", () => {
  it("categorises entries", () => {
    expect(categoryOf(entry("a.JPG", { extension: "jpg" }))).toBe("image");
    expect(categoryOf(entry("dir", { kind: "dir", extension: null }))).toBe("folder");
    expect(categoryOf(entry("j", { kind: "junction", extension: null }))).toBe("folder");
    expect(typeLabel(entry("x.pdf"))).toBe("PDF document");
    expect(typeLabel(entry("x.rs"))).toBe("RS file");
    expect(typeLabel(entry("noext"))).toBe("File");
    expect(typeLabel(entry("j", { kind: "junction", extension: null }))).toBe("Junction");
  });
  it("flags executables", () => {
    expect(isExecutable(entry("setup.exe"))).toBe(true);
    expect(isExecutable(entry("run.ps1"))).toBe(true);
    expect(isExecutable(entry("photo.png"))).toBe(false);
  });
});

describe("paths", () => {
  it("builds breadcrumbs for drive and UNC paths", () => {
    expect(breadcrumbs("C:\\Users\\me")).toEqual([
      { label: "C:", path: "C:\\" },
      { label: "Users", path: "C:\\Users" },
      { label: "me", path: "C:\\Users\\me" },
    ]);
    expect(breadcrumbs("C:\\")).toEqual([{ label: "C:", path: "C:\\" }]);
    expect(breadcrumbs("\\\\srv\\share\\docs").map((c) => c.path)).toEqual(["\\\\srv\\share", "\\\\srv\\share\\docs"]);
  });
  it("compares and names paths", () => {
    expect(samePath("C:\\A\\", "c:/a")).toBe(true);
    expect(baseName("C:\\Users\\me\\")).toBe("me");
    expect(baseName("C:\\")).toBe("C:");
    expect(uniqueName("New folder", ["new folder", "New folder (2)"])).toBe("New folder (3)");
    expect(uniqueName("New folder", [])).toBe("New folder");
  });
  it("mirrors backend name validation", () => {
    expect(validateName("ok name.txt")).toBeNull();
    expect(validateName("")).not.toBeNull();
    expect(validateName("a:b")).not.toBeNull();
    expect(validateName("trailing.")).not.toBeNull();
    expect(validateName("CON.txt")).not.toBeNull();
    expect(validateName("console.txt")).toBeNull();
  });
});

describe("sort", () => {
  const items = [
    entry("file10.txt", { size: 10, modified: 3 }),
    entry("File2.txt", { size: 300, modified: 1 }),
    entry("zeta", { kind: "dir", size: null, extension: null }),
    entry("alpha", { kind: "dir", size: null, extension: null }),
  ];
  it("keeps folders first and sorts naturally", () => {
    expect(sortEntries(items, { key: "name", direction: "asc" }).map((e) => e.name)).toEqual(["alpha", "zeta", "File2.txt", "file10.txt"]);
    expect(sortEntries(items, { key: "name", direction: "desc" }).map((e) => e.name)).toEqual(["zeta", "alpha", "file10.txt", "File2.txt"]);
    expect(sortEntries(items, { key: "size", direction: "desc" }).map((e) => e.name)).toEqual(["alpha", "zeta", "File2.txt", "file10.txt"]);
    expect(sortEntries(items, { key: "modified", direction: "asc" }).slice(2).map((e) => e.name)).toEqual(["File2.txt", "file10.txt"]);
  });
  it("filters case-insensitively", () => {
    expect(filterEntries(items, "FILE").map((e) => e.name)).toEqual(["file10.txt", "File2.txt"]);
    expect(filterEntries(items, "  ")).toHaveLength(4);
  });
});

describe("selection", () => {
  const order = ["a", "b", "c", "d", "e"];
  it("handles click, ctrl and shift", () => {
    let s = clickSelect(emptySelection, order, "b", { ctrl: false, shift: false });
    expect([...s.selected]).toEqual(["b"]);
    s = clickSelect(s, order, "d", { ctrl: false, shift: true });
    expect([...s.selected]).toEqual(["b", "c", "d"]);
    s = clickSelect(s, order, "c", { ctrl: true, shift: false });
    expect([...s.selected].sort()).toEqual(["b", "d"]);
  });
  it("moves focus with keyboard and range extension", () => {
    let s = moveFocus(emptySelection, order, 1, { shift: false, ctrl: false });
    expect(s.focus).toBe("a");
    s = moveFocus(s, order, 2, { shift: true, ctrl: false });
    expect([...s.selected]).toEqual(["a", "b", "c"]);
    s = moveFocus(s, order, 0, { shift: false, ctrl: false, absolute: 99 });
    expect(s.focus).toBe("e");
    expect(selectAll(order).selected.size).toBe(5);
  });
  it("prunes vanished items", () => {
    const s = pruneSelection(selectAll(order), ["a", "c"]);
    expect([...s.selected]).toEqual(["a", "c"]);
    expect(s.focus).toBe("a");
  });
});
