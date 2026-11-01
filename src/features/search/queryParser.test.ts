import { describe, expect, it } from "vitest";
import { parseQuery } from "./queryParser";

const NOW = new Date(2026, 9, 30, 15, 0, 0); // 30 Oct 2026, a Friday
const GB = 1024 ** 3;
const DAY = 86_400_000;

describe("natural-language search parser", () => {
  it("understands the documented examples", () => {
    const videos = parseQuery("Find all videos larger than 2 GB.", NOW);
    expect(videos.categories).toEqual(["video"]);
    expect(videos.minSize).toBe(2 * GB);
    expect(videos.text).toBeNull();

    const pdfs = parseQuery("PDF files in Downloads", NOW);
    expect(pdfs.extensions).toEqual(["pdf"]);
    expect(pdfs.scope).toBe("downloads");
    expect(pdfs.text).toBeNull();

    const zips = parseQuery("ZIP files over 1 GB", NOW);
    expect(zips.extensions).toEqual(["zip"]);
    expect(zips.minSize).toBe(GB);

    const images = parseQuery("images from last month", NOW);
    expect(images.categories).toEqual(["image"]);
    expect(images.modifiedAfter).toBe(NOW.getTime() - 30 * DAY);
  });

  it("parses sizes with decimals and units", () => {
    expect(parseQuery("files smaller than 1.5 mb", NOW).maxSize).toBe(Math.round(1.5 * 1024 ** 2));
    expect(parseQuery("bigger than 500 kilobytes", NOW).minSize).toBe(500 * 1024);
    expect(parseQuery("over 2,5 GB", NOW).minSize).toBe(Math.round(2.5 * GB));
    // Unknown unit: left as name text, no size filter.
    const q = parseQuery("larger than 5 apples", NOW);
    expect(q.minSize).toBeNull();
  });

  it("parses relative and absolute dates", () => {
    const today = new Date(2026, 9, 30).getTime();
    expect(parseQuery("modified today", NOW).modifiedAfter).toBe(today);
    const y = parseQuery("changed yesterday", NOW);
    expect([y.modifiedAfter, y.modifiedBefore]).toEqual([today - DAY, today]);
    expect(parseQuery("this week", NOW).modifiedAfter).toBe(new Date(2026, 9, 26).getTime());
    expect(parseQuery("logs older than 30 days", NOW).modifiedBefore).toBe(NOW.getTime() - 30 * DAY);
    expect(parseQuery("in the last 3 weeks", NOW).modifiedAfter).toBe(NOW.getTime() - 21 * DAY);
    const year = parseQuery("photos from 2024", NOW);
    expect([year.modifiedAfter, year.modifiedBefore]).toEqual([new Date(2024, 0, 1).getTime(), new Date(2025, 0, 1).getTime()]);
    expect(parseQuery("before 2020", NOW).modifiedBefore).toBe(new Date(2020, 0, 1).getTime());
    expect(parseQuery("after 2026-05-01", NOW).modifiedAfter).toBe(new Date(2026, 4, 2).getTime());
  });

  it("keeps unknown words as name text and supports wildcards and quotes", () => {
    const q = parseQuery("show me invoice pdfs", NOW);
    expect(q.text).toBe("invoice");
    expect(q.extensions).toEqual(["pdf"]);
    expect(parseQuery("server log", NOW)).toMatchObject({ text: "server log", extensions: [] });
    expect(parseQuery("*.log", NOW).extensions).toEqual(["log"]);
    expect(parseQuery("IMG_????.jpg", NOW).text).toBe("IMG_????.jpg");
    expect(parseQuery('files named "Q3 report"', NOW).text).toBe("Q3 report");
    expect(parseQuery("folders named src", NOW)).toMatchObject({ kind: "folders", text: "src" });
  });

  it("recognises scopes and paths", () => {
    expect(parseQuery("music in my Music folder", NOW).scope).toBe("music");
    expect(parseQuery("anything in D:\\Backups", NOW).scope).toBe("D:\\Backups");
  });

  it("explains its interpretation with chips", () => {
    const q = parseQuery("videos larger than 2 GB in Downloads", NOW);
    expect(q.chips).toEqual(expect.arrayContaining(["In: Downloads", "Size > 2 GB", "Type: Videos"]));
  });

  it("treats plain words as a name search", () => {
    expect(parseQuery("holiday", NOW)).toMatchObject({ text: "holiday", kind: "any", categories: [], extensions: [] });
    expect(parseQuery("", NOW).text).toBeNull();
  });
});
