import { useState } from "react";
import { api } from "../../api/client";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { EmptyState } from "../../components/EmptyState";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Spinner } from "../../components/Progress";
import { useToast } from "../../components/Toast";
import {
  IconAlert,
  IconCheckCircle,
  IconCopy,
  IconExternal,
  IconRefresh,
  IconScroll,
  IconTerminal,
} from "../../components/icons";
import { useAsync } from "../../hooks/useAsync";

const sectionTone = (title: string) => {
  const value = title.toLowerCase();
  if (value.includes("removed")) return "red";
  if (value.includes("changed") || value.includes("deprecated")) return "amber";
  if (value.includes("added")) return "green";
  if (value.includes("security")) return "violet";
  if (value.includes("fixed")) return "blue";
  return "neutral";
};

export function ChangelogPage() {
  const { push } = useToast();
  const [loaded, setLoaded] = useState(false);
  const [target, setTarget] = useState<string | null>(null);
  const [showRaw, setShowRaw] = useState(false);

  const update = useAsync(() => api.checkCliUpdate(), []);
  const changelog = useAsync(
    () => (loaded ? api.getChangelog(target) : Promise.resolve(null)),
    [loaded, target],
  );

  const data = changelog.data;
  const copyMarkdown = async () => {
    if (!data) {
      return;
    }
    try {
      await navigator.clipboard.writeText(data.markdown);
      push("success", "Release notes copied to the clipboard.");
    } catch {
      push("error", "Could not copy to the clipboard.");
    }
  };

  return (
    <div className="page-stack">
      <header className="page-header">
        <div>
          <span className="eyebrow">Beta watch</span>
          <h1>Changelog &amp; breaking changes</h1>
          <p>
            Release notes come from the installed CLI. Breaking-change flags are a heuristic
            pass over the notes, designed to be replaced by a remote API provider later.
          </p>
        </div>
        <div className="inline-actions">
          {loaded ? (
            <Button
              variant="secondary"
              icon={<IconRefresh size={15} />}
              onClick={changelog.reload}
              loading={changelog.loading}
            >
              Refresh notes
            </Button>
          ) : null}
          <Button
            variant="primary"
            icon={<IconScroll size={15} />}
            onClick={() => setLoaded(true)}
            loading={loaded && changelog.loading && !data}
          >
            {loaded ? "Reload changelog" : "Show changelog"}
          </Button>
        </div>
      </header>

      <Card
        title="Release selection"
        subtitle="Read notes for the installed CLI or for the pending update."
      >
        <div className="release-select-row">
          <label className="field">
            <span>Version</span>
            <select
              value={target ?? ""}
              onChange={(event) => {
                setTarget(event.target.value || null);
                setLoaded(true);
              }}
              disabled={!loaded}
            >
              <option value="">Installed CLI{update.data ? ` (${update.data.currentVersion})` : ""}</option>
              {update.data?.updateAvailable ? (
                <option value={update.data.latestVersion}>
                  Latest update ({update.data.latestVersion})
                </option>
              ) : null}
            </select>
          </label>
          <div className="source-pill">
            <IconTerminal size={15} />
            <span>Provider: {data?.source ?? "unity-changelog"}</span>
          </div>
        </div>
        {update.error ? (
          <p className="muted small">
            Could not check for a newer CLI: {update.error.message}
          </p>
        ) : update.data?.updateAvailable ? (
          <p className="muted small">
            A newer CLI is available ({update.data.currentVersion} → {update.data.latestVersion}).
            Pick it above to read the pending notes.
          </p>
        ) : null}
      </Card>

      {!loaded ? (
        <EmptyState
          icon={<IconScroll size={30} />}
          title="Changelog not loaded yet"
          description="Press “Show changelog” to read the installed CLI's release notes and flag the entries that may break workflows."
          action={
            <Button variant="primary" icon={<IconScroll size={15} />} onClick={() => setLoaded(true)}>
              Show changelog
            </Button>
          }
        />
      ) : changelog.loading && !data ? (
        <Card><Spinner label="Reading release notes" /></Card>
      ) : changelog.error ? (
        <ErrorBanner error={changelog.error} onRetry={changelog.reload} />
      ) : data ? (
        <>
          <div className="changelog-heading">
            <div>
              <span className="eyebrow">Release notes</span>
              <h2>Unity CLI {data.version}</h2>
              <p className="muted">{data.sourceDetail} · {data.entryCount} entries parsed</p>
            </div>
            <div className="inline-actions">
              <Button variant="secondary" icon={<IconCopy size={15} />} onClick={copyMarkdown}>
                Copy Markdown
              </Button>
              <Button variant="ghost" onClick={() => setShowRaw((value) => !value)}>
                {showRaw ? "Hide raw" : "Show raw"}
              </Button>
            </div>
          </div>

          <Card
            title="Possible breaking changes"
            subtitle="Heuristic: removed/deprecated sections plus removal, rename, and incompatibility wording."
            className={data.breakingChanges.length > 0 ? "breaking-card" : "safe-card"}
          >
            {data.breakingChanges.length === 0 ? (
              <div className="safe-message">
                <IconCheckCircle size={20} />
                <div>
                  <strong>No likely breaking changes detected.</strong>
                  <p className="muted small">
                    This is a text heuristic, not a guarantee. Read the Changed and Removed
                    sections below before updating the CLI.
                  </p>
                </div>
              </div>
            ) : (
              <ul className="breaking-list">
                {data.breakingChanges.map((change, index) => (
                  <li key={`${change.text}-${index}`}>
                    <IconAlert size={16} />
                    <div>
                      <p>{change.text}</p>
                      <div className="badge-row">
                        <Badge tone="red">{change.section}</Badge>
                        <Badge tone="amber">{change.reason}</Badge>
                      </div>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </Card>

          <div className="changelog-sections">
            {data.sections.map((section) => (
              <Card
                key={section.title}
                title={
                  <span className="section-title">
                    {section.title}
                    <Badge tone={sectionTone(section.title)}>{section.entries.length}</Badge>
                  </span>
                }
              >
                {section.entries.length === 0 ? (
                  <p className="muted">No entries.</p>
                ) : (
                  <ul className="notes-list">
                    {section.entries.map((entry, index) => (
                      <li key={`${entry.slice(0, 40)}-${index}`}>{entry}</li>
                    ))}
                  </ul>
                )}
              </Card>
            ))}
          </div>

          {showRaw ? (
            <Card title="Raw Markdown" subtitle="Exactly what `unity changelog --format json` returned.">
              <pre className="raw-markdown">{data.markdown}</pre>
            </Card>
          ) : null}

          <div className="provider-note">
            <IconExternal size={15} />
            <span>
              Provider interface: <code>unity-changelog</code> today. A remote JSON API can be
              added behind the same response shape without changing this screen.{" "}
              <a
                href="https://docs.unity.com/en-us/unity-cli/release-notes"
                target="_blank"
                rel="noreferrer"
              >
                Unity CLI release notes
              </a>
            </span>
          </div>
        </>
      ) : (
        <EmptyState title="No changelog data" />
      )}
    </div>
  );
}
