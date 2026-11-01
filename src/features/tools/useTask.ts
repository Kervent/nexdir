import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../../services/api";
import type { TaskProgress } from "../../types/tools";

/**
 * Runs one background task at a time for a view: tracks its progress events, fetches the
 * result when it ends (also partial results after cancellation) and cancels it when the
 * view goes away.
 */
export function useTask<R>() {
  const [progress, setProgress] = useState<TaskProgress | null>(null);
  const [result, setResult] = useState<R | null>(null);
  const [error, setError] = useState<string | null>(null);
  const taskId = useRef<number | null>(null);

  const finish = useCallback((p: TaskProgress) => {
    setProgress(p);
    if (p.state === "failed") setError(p.error ?? "The task failed.");
    api
      .taskResult<R>(p.id)
      .then((r) => taskId.current === p.id && setResult(r))
      .catch((e) => setError(errorMessage(e)));
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    api
      .onTaskProgress((p) => {
        if (p.id !== taskId.current) return;
        if (p.state === "running") setProgress(p);
        else finish(p);
      })
      .then((fn) => (disposed ? fn() : (unlisten = fn)))
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
      // Leaving the view stops its work.
      if (taskId.current !== null) void api.cancelTask(taskId.current).catch(() => {});
    };
  }, [finish]);

  const start = useCallback(
    async (launch: () => Promise<number>) => {
      if (taskId.current !== null) void api.cancelTask(taskId.current).catch(() => {});
      setResult(null);
      setError(null);
      setProgress(null);
      try {
        const id = await launch();
        taskId.current = id;
        // The task may already be done before the first event arrives (tiny folders).
        const status = await api.taskStatus(id);
        if (status && taskId.current === id) {
          if (status.state === "running") setProgress((p) => p ?? status);
          else finish(status);
        }
      } catch (e) {
        setError(errorMessage(e));
      }
    },
    [finish],
  );

  const cancel = useCallback(() => {
    if (taskId.current !== null) void api.cancelTask(taskId.current);
  }, []);

  const running = progress?.state === "running";
  return { start, cancel, progress, result, error, running, setResult };
}
