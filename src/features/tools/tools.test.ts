import { describe, expect, it } from "vitest";
import { squarify } from "./treemap";
import { fullySelectedGroups, selectAllButOne } from "./DuplicatesView";
import { describeRule, newRule } from "./OrganizeView";
import { buildQuery } from "./SearchView";
import type { DuplicateGroup } from "../../types/tools";
import type { KnownFolder } from "../../types/fs";

describe("treemap", () => {
  it("fills the area proportionally without overlap", () => {
    const values = [60, 25, 10, 5];
    const rects = squarify(values, { x: 0, y: 0, w: 200, h: 100 });
    const area = rects.reduce((s, r) => s + r.w * r.h, 0);
    expect(area).toBeCloseTo(20000, 3);
    rects.forEach((r, i) => expect(r.w * r.h).toBeCloseTo((values[i]! / 100) * 20000, 3));
    for (const r of rects) {
      expect(r.x).toBeGreaterThanOrEqual(-1e-9);
      expect(r.y).toBeGreaterThanOrEqual(-1e-9);
      expect(r.x + r.w).toBeLessThanOrEqual(200 + 1e-6);
      expect(r.y + r.h).toBeLessThanOrEqual(100 + 1e-6);
    }
  });
  it("handles empty input", () => {
    expect(squarify([], { x: 0, y: 0, w: 10, h: 10 })).toEqual([]);
    expect(squarify([0, 0], { x: 0, y: 0, w: 10, h: 10 }).every((r) => r.w === 0)).toBe(true);
  });
});

describe("duplicate selection safety", () => {
  const groups: DuplicateGroup[] = [
    { size: 10, hash: "a", files: [{ path: "C:\\a1", name: "a1", modified: 1 }, { path: "C:\\a2", name: "a2", modified: 5 }, { path: "C:\\a3", name: "a3", modified: 3 }] },
    { size: 20, hash: "b", files: [{ path: "C:\\b1", name: "b1", modified: 9 }, { path: "C:\\b2", name: "b2", modified: 2 }] },
  ];
  it("keeps exactly one file per group", () => {
    expect([...selectAllButOne(groups, "oldest")].sort()).toEqual(["C:\\a2", "C:\\a3", "C:\\b1"]);
    expect([...selectAllButOne(groups, "newest")].sort()).toEqual(["C:\\a1", "C:\\a3", "C:\\b2"]);
    expect(fullySelectedGroups(groups, selectAllButOne(groups, "oldest"))).toEqual([]);
  });
  it("detects groups where every copy is selected", () => {
    expect(fullySelectedGroups(groups, new Set(["C:\\b1", "C:\\b2", "C:\\a1"])).map((g) => g.hash)).toEqual(["b"]);
  });
});

const known: KnownFolder[] = [
  { id: "home", label: "Home folder", path: "C:\\Users\\me" },
  { id: "downloads", label: "Downloads", path: "C:\\Users\\me\\Downloads" },
  { id: "documents", label: "Documents", path: "C:\\Users\\me\\Documents" },
];

describe("organize rules", () => {
  it("creates a sensible default rule and describes rules", () => {
    const r = newRule({ knownFolders: known });
    expect(r.source).toBe("C:\\Users\\me\\Downloads");
    expect(r.destination).toBe("C:\\Users\\me\\Documents\\PDF");
    expect(describeRule(r)).toBe("Move .pdf files from C:\\Users\\me\\Downloads to C:\\Users\\me\\Documents\\PDF");
    expect(describeRule({ ...r, action: "copy", extensions: [], olderThanDays: 30, includeSubfolders: true })).toBe(
      "Copy all files (older than 30 days) from C:\\Users\\me\\Downloads and subfolders to C:\\Users\\me\\Documents\\PDF",
    );
  });
});

describe("search query building", () => {
  it("maps the parsed request to a backend query", () => {
    const { query, chips } = buildQuery("pdf files in Downloads larger than 1 MB", "C:\\Users\\me", { includeHidden: false, recursive: true }, { knownFolders: known });
    expect(query).toMatchObject({ root: "C:\\Users\\me\\Downloads", extensions: ["pdf"], minSize: 1024 ** 2, recursive: true, text: null });
    expect(chips).toContain("In: Downloads");
  });
  it("falls back to the chosen folder", () => {
    const { query } = buildQuery("report", "D:\\Work", { includeHidden: true, recursive: false }, { knownFolders: known });
    expect(query).toMatchObject({ root: "D:\\Work", text: "report", includeHidden: true, recursive: false });
  });
});
