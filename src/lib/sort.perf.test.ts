// Timing check for very large folders. Node's ICU collator is close to WebView2's, so this
// is a useful upper-bound estimate, not a UI frame measurement.
import { describe, expect, it } from "vitest";
import type { FileEntry } from "../types/fs";
import { filterEntries, sortEntries, type SortKey } from "./sort";

const EXT = ["txt", "jpg", "pdf", "log", "mp4", "docx", "rs", "zip", null];

function entries(n: number): FileEntry[] {
  return Array.from({ length: n }, (_, i) => {
    const ext = EXT[i % EXT.length] ?? null;
    const dir = i % 50 === 0;
    return {
      name: `${dir ? "Folder" : "IMG_report"} ${(i * 7919) % n}${ext && !dir ? "." + ext : ""}`,
      path: `C:\\data\\${i}`,
      kind: dir ? "dir" : "file",
      size: dir ? null : (i * 31) % 100_000,
      modified: 1_700_000_000_000 + ((i * 104_729) % 1_000_000),
      created: null,
      extension: dir ? null : ext,
      hidden: false,
      system: false,
      readonly: false,
    };
  });
}

function time(fn: () => void): number {
  const start = performance.now();
  fn();
  return performance.now() - start;
}

describe("sorting and filtering 100,000 entries", () => {
  const list = entries(100_000);

  for (const key of ["name", "type", "size", "modified"] as SortKey[]) {
    it(`sorts by ${key} within budget`, () => {
      let result: FileEntry[] = [];
      const ms = time(() => {
        result = sortEntries(list, { key, direction: "asc" });
      });
      console.log(`PERF sort 100k by ${key}: ${ms.toFixed(0)} ms`);
      expect(result).toHaveLength(100_000);
      expect(ms).toBeLessThan(5_000);
    });
  }

  it("filters within budget", () => {
    let result: FileEntry[] = [];
    const ms = time(() => {
      result = filterEntries(list, "report 12");
    });
    console.log(`PERF filter 100k: ${ms.toFixed(1)} ms`);
    expect(result.length).toBeGreaterThan(0);
    expect(ms).toBeLessThan(1_000);
  });
});
