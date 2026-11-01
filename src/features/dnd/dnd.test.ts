import { describe, expect, it } from "vitest";
import { canDrop, chooseEffect, volumeOf } from "./dnd";
import { computeRate } from "../transfers/useOperations";
import { formatDuration } from "../transfers/OperationsPanel";
import type { OperationProgress } from "../../types/fs";

const none = { ctrlKey: false, shiftKey: false };

describe("drag and drop rules", () => {
  it("identifies volumes", () => {
    expect(volumeOf("c:\\x")).toBe("C");
    expect(volumeOf("\\\\Srv\\Share\\a")).toBe("\\\\srv\\share");
  });

  it("moves on the same drive and copies across drives, with Ctrl/Shift overrides", () => {
    expect(chooseEffect(none, ["C:\\a\\x.txt"], "C:\\b")).toBe("move");
    expect(chooseEffect(none, ["C:\\a\\x.txt"], "D:\\b")).toBe("copy");
    expect(chooseEffect({ ctrlKey: true, shiftKey: false }, ["C:\\a\\x.txt"], "C:\\b")).toBe("copy");
    expect(chooseEffect({ ctrlKey: false, shiftKey: true }, ["C:\\a\\x.txt"], "D:\\b")).toBe("move");
  });

  it("rejects drops onto the item itself, into its own subtree or its current folder", () => {
    expect(canDrop(["C:\\a\\dir"], "C:\\a\\dir")).toBe(false);
    expect(canDrop(["C:\\a\\dir"], "C:\\a\\dir\\sub")).toBe(false);
    expect(canDrop(["C:\\a\\x.txt"], "C:\\a")).toBe(false);
    expect(canDrop(["C:\\a\\x.txt"], "C:\\b")).toBe(true);
    expect(canDrop(["C:\\a\\dir"], "C:\\a\\directory")).toBe(true);
    expect(canDrop([], "C:\\b")).toBe(false);
  });
});

describe("transfer rate", () => {
  const p = (doneBytes: number, totalBytes: number) => ({ doneBytes, totalBytes, state: "running" }) as OperationProgress;
  it("needs at least a second of data", () => {
    expect(computeRate({ at: 0, bytes: 0 }, 500, p(100, 1000))).toBeNull();
  });
  it("computes speed and remaining time", () => {
    expect(computeRate({ at: 0, bytes: 0 }, 2000, p(200, 1000))).toEqual({ bytesPerSecond: 100, etaSeconds: 8 });
  });
  it("formats durations", () => {
    expect(formatDuration(42)).toBe("42 s");
    expect(formatDuration(125)).toBe("2 min 5 s");
    expect(formatDuration(3720)).toBe("1 h 2 min");
  });
});
