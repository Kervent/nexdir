import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../services/api";
import type { OperationProgress } from "../../types/fs";

export interface TransferRate {
  bytesPerSecond: number;
  /** Estimated seconds remaining, or null while the rate is not yet meaningful. */
  etaSeconds: number | null;
}

/** Average throughput since the job started copying (scan time excluded). */
export function computeRate(start: { at: number; bytes: number }, now: number, p: OperationProgress): TransferRate | null {
  const elapsed = (now - start.at) / 1000;
  const copied = p.doneBytes - start.bytes;
  if (elapsed < 1 || copied <= 0) return null;
  const bytesPerSecond = copied / elapsed;
  const remaining = Math.max(0, p.totalBytes - p.doneBytes);
  return { bytesPerSecond, etaSeconds: bytesPerSecond > 0 ? Math.ceil(remaining / bytesPerSecond) : null };
}

/** Live state of copy/move jobs, fed by backend progress events. */
export function useOperations(onFinished: (p: OperationProgress) => void) {
  const [operations, setOperations] = useState<Map<number, OperationProgress>>(new Map());
  const starts = useRef(new Map<number, { at: number; bytes: number }>());
  const onFinishedRef = useRef(onFinished);
  onFinishedRef.current = onFinished;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    api
      .onOperationProgress((p) => {
        if (p.state === "running" && !starts.current.has(p.id)) starts.current.set(p.id, { at: Date.now(), bytes: p.doneBytes });
        setOperations((prev) => new Map(prev).set(p.id, p));
        if (p.state === "completed" || p.state === "cancelled") {
          starts.current.delete(p.id);
          onFinishedRef.current(p);
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    api
      .listOperations()
      .then((list) => !disposed && setOperations(new Map(list.map((p) => [p.id, p]))))
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const dismiss = useCallback((id: number) => {
    setOperations((prev) => {
      const next = new Map(prev);
      next.delete(id);
      return next;
    });
  }, []);

  const rateOf = useCallback((p: OperationProgress): TransferRate | null => {
    const start = starts.current.get(p.id);
    return start && p.state === "running" ? computeRate(start, Date.now(), p) : null;
  }, []);

  const list = [...operations.values()].sort((a, b) => b.id - a.id);
  const active = list.filter((p) => p.state === "queued" || p.state === "scanning" || p.state === "running");
  return { operations: list, active, dismiss, rateOf };
}
