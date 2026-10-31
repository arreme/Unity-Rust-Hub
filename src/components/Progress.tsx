export function ProgressBar({
  value,
  indeterminate = false,
}: {
  value: number;
  indeterminate?: boolean;
}) {
  const clamped = Math.max(0, Math.min(100, value));
  return (
    <div
      className={`progress ${indeterminate ? "progress-indeterminate" : ""}`.trim()}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : Math.round(clamped)}
    >
      <div className="progress-fill" style={{ width: indeterminate ? undefined : `${clamped}%` }} />
    </div>
  );
}

export function Spinner({ label }: { label?: string }) {
  return (
    <span className="inline-spinner">
      <span className="spinner" />
      {label ? <span>{label}</span> : null}
    </span>
  );
}
