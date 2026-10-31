import { useState } from "react";
import type { UnityError } from "../api/types";
import { Badge } from "./Badge";
import { Button } from "./Button";
import { IconAlert } from "./icons";

export function ErrorBanner({
  error,
  onRetry,
  compact = false,
}: {
  error: UnityError;
  onRetry?: () => void;
  compact?: boolean;
}) {
  const [showDetail, setShowDetail] = useState(false);
  return (
    <div className={`error-banner ${compact ? "error-banner-compact" : ""}`.trim()}>
      <div className="error-icon">
        <IconAlert size={18} />
      </div>
      <div className="error-copy">
        <div className="error-title">
          <Badge tone="red" mono>
            {error.code}
          </Badge>
          <span>{error.message}</span>
          {error.retryable ? <Badge tone="amber">retryable</Badge> : null}
        </div>
        {error.detail ? (
          <>
            <button className="link-button error-detail-toggle" onClick={() => setShowDetail((v) => !v)}>
              {showDetail ? "Hide details" : "Show details"}
            </button>
            {showDetail ? <pre className="error-detail">{error.detail}</pre> : null}
          </>
        ) : null}
      </div>
      {onRetry ? (
        <Button size="sm" variant="secondary" onClick={onRetry}>
          Retry
        </Button>
      ) : null}
    </div>
  );
}
