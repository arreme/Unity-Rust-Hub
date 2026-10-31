import { useCallback, useEffect, useRef, useState } from "react";
import { api, normalizeError, subscribeToJobs } from "../api/client";
import type { JobStarted, JobUpdate, UnityError } from "../api/types";

export interface JobLogLine {
  stream: "stdout" | "stderr";
  text: string;
}

export interface ActiveJob {
  id: string | null;
  kind: string;
  command?: string;
  state: JobUpdate["state"];
  progress: number;
  message?: string;
  logs: JobLogLine[];
  result?: unknown;
  error?: UnityError;
}

const MAX_LOG_LINES = 600;

function initialState(kind: string): ActiveJob {
  return {
    id: null,
    kind,
    state: "starting",
    progress: 0,
    logs: [],
  };
}

export function useJob() {
  const [job, setJob] = useState<ActiveJob | null>(null);
  const activeId = useRef<string | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    subscribeToJobs((update) => {
      setJob((previous) => {
        if (!previous) {
          return previous;
        }
        // The started event can arrive before invoke() resolves. Accept it
        // while the id is still unknown, then pin the job id.
        if (previous.id && previous.id !== update.jobId) {
          return previous;
        }
        if (!previous.id) {
          activeId.current = update.jobId;
        }
        const logs = update.line
          ? [
              ...previous.logs,
              {
                stream: (update.stream ?? "stdout") as JobLogLine["stream"],
                text: update.line,
              },
            ].slice(-MAX_LOG_LINES)
          : previous.logs;
        return {
          ...previous,
          id: update.jobId,
          state: update.state,
          progress:
            typeof update.progress === "number" ? update.progress : previous.progress,
          message: update.message ?? previous.message,
          logs,
          result: update.result ?? previous.result,
          error: update.error ?? previous.error,
        };
      });
    }).then((stop) => {
      if (disposed) {
        stop();
      } else {
        unlisten = stop;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const start = useCallback(
    async (starter: () => Promise<JobStarted>, label: string): Promise<JobStarted | null> => {
      activeId.current = null;
      setJob(initialState(label));
      try {
        const started = await starter();
        if (!activeId.current) {
          activeId.current = started.jobId;
        }
        setJob((previous) =>
          previous
            ? {
                ...previous,
                id: started.jobId,
                kind: started.kind,
                command: started.command,
              }
            : previous,
        );
        return started;
      } catch (error) {
        setJob((previous) =>
          previous
            ? { ...previous, state: "failed", error: normalizeError(error) }
            : previous,
        );
        return null;
      }
    },
    [],
  );

  const cancel = useCallback(async () => {
    if (!job?.id) {
      return;
    }
    try {
      await api.cancelJob(job.id);
    } catch (error) {
      setJob((previous) =>
        previous ? { ...previous, error: normalizeError(error) } : previous,
      );
    }
  }, [job?.id]);

  const reset = useCallback(() => setJob(null), []);

  return { job, start, cancel, reset };
}
