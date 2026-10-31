import { useEffect, useRef } from "react";
import type { JobLogLine } from "../hooks/useJob";

export function LogPanel({
  logs,
  height = 220,
}: {
  logs: JobLogLine[];
  height?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = ref.current;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [logs.length]);

  return (
    <div className="log-panel" ref={ref} style={{ maxHeight: height }}>
      {logs.length === 0 ? (
        <span className="log-empty">Waiting for output…</span>
      ) : (
        logs.map((line, index) => (
          <div
            key={`${index}-${line.text.slice(0, 24)}`}
            className={`log-line ${line.stream === "stderr" ? "log-stderr" : ""}`.trim()}
          >
            {line.text}
          </div>
        ))
      )}
    </div>
  );
}
