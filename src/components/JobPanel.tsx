import { useState } from "react";
import type { ActiveJob } from "../hooks/useJob";
import { Badge } from "./Badge";
import { Button } from "./Button";
import { LogPanel } from "./LogPanel";
import { ProgressBar } from "./Progress";
import { IconStop, IconTerminal, IconX } from "./icons";

const stateTone = {
  starting: "blue",
  started: "blue",
  running: "blue",
  succeeded: "green",
  failed: "red",
  cancelled: "amber",
} as const;

export function JobPanel({
  job,
  onCancel,
  onDismiss,
}: {
  job: ActiveJob | null;
  onCancel: () => void;
  onDismiss: () => void;
}) {
  const [showLog, setShowLog] = useState(false);

  if (!job) {
    return null;
  }

  const active = job.state === "starting" || job.state === "started" || job.state === "running";
  const indeterminate = active && job.progress <= 0;

  return (
    <section className="job-panel">
      <header className="job-header">
        <div className="job-heading">
          <span className="job-icon">
            <IconTerminal />
          </span>
          <div>
            <div className="job-title-row">
              <h3>{job.kind}</h3>
              <Badge tone={stateTone[job.state]}>{job.state}</Badge>
              {job.progress > 0 ? (
                <span className="job-percent">{Math.round(job.progress)}%</span>
              ) : null}
            </div>
            {job.message ? <p className="job-message">{job.message}</p> : null}
          </div>
        </div>
        <div className="job-actions">
          {active ? (
            <Button
              size="sm"
              variant="danger"
              icon={<IconStop size={14} />}
              onClick={onCancel}
              disabled={!job.id}
            >
              Cancel
            </Button>
          ) : (
            <button className="icon-button" onClick={onDismiss} aria-label="Dismiss job">
              <IconX />
            </button>
          )}
        </div>
      </header>

      {active ? (
        <div className="job-progress">
          <ProgressBar value={job.progress} indeterminate={indeterminate} />
        </div>
      ) : null}

      {job.error ? (
        <div className="job-error">
          <Badge tone="red" mono>
            {job.error.code}
          </Badge>
          <span>{job.error.message}</span>
          {job.error.detail ? <details><summary>Details</summary><pre>{job.error.detail}</pre></details> : null}
        </div>
      ) : null}

      {job.command ? (
        <div className="job-command">
          <code>{job.command}</code>
        </div>
      ) : null}

      <div className="job-log-toggle">
        <button className="link-button" onClick={() => setShowLog((value) => !value)}>
          {showLog ? "Hide raw log" : `Show raw log (${job.logs.length})`}
        </button>
      </div>
      {showLog ? <LogPanel logs={job.logs} /> : null}
    </section>
  );
}
