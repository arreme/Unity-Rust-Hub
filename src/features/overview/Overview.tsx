import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { CliStatus, Page, UnityError } from "../../api/types";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { EmptyState } from "../../components/EmptyState";
import { ErrorBanner } from "../../components/ErrorBanner";
import { JobPanel } from "../../components/JobPanel";
import { Modal } from "../../components/Modal";
import { Spinner } from "../../components/Progress";
import { useToast } from "../../components/Toast";
import {
  IconAlert,
  IconBox,
  IconDownload,
  IconExternal,
  IconFolder,
  IconPlay,
  IconRefresh,
  IconScroll,
} from "../../components/icons";
import { useAsync } from "../../hooks/useAsync";
import { useJob } from "../../hooks/useJob";
import { isWindows } from "../../lib/format";

interface OverviewProps {
  status: CliStatus | null;
  loading: boolean;
  error: UnityError | null;
  reload: () => void;
  navigate: (page: Page) => void;
}

export function Overview({ status, loading, error, reload, navigate }: OverviewProps) {
  const { push } = useToast();
  const [installOpen, setInstallOpen] = useState(false);
  const [channel, setChannel] = useState("stable");
  const [consent, setConsent] = useState(false);
  const cliJob = useJob();

  const installed = Boolean(status?.installed);

  const auth = useAsync(
    () => (installed ? api.getAuthStatus() : Promise.resolve(null)),
    [installed],
  );
  const license = useAsync(
    () => (installed ? api.getLicenseStatus() : Promise.resolve(null)),
    [installed],
  );
  const running = useAsync(
    () => (installed ? api.listRunningEditors() : Promise.resolve(null)),
    [installed],
  );
  const update = useAsync(
    () => (installed ? api.checkCliUpdate() : Promise.resolve(null)),
    [installed],
  );

  const jobState = cliJob.job?.state;
  useEffect(() => {
    if (jobState === "succeeded") {
      push("success", "Unity CLI operation finished.");
      reload();
      update.reload();
      auth.reload();
      license.reload();
      running.reload();
    } else if (jobState === "failed" && cliJob.job?.error) {
      push("error", cliJob.job.error.message);
    }
    // The reload functions are stable; the job state drives the transition.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jobState]);

  useEffect(() => {
    if (status?.version?.channel) {
      setChannel(status.version.channel);
    }
  }, [status?.version?.channel]);

  const openInstall = () => {
    setConsent(false);
    setInstallOpen(true);
  };

  const startInstall = async () => {
    const started = await cliJob.start(
      () => api.installCli(channel),
      "Installing the Unity CLI",
    );
    if (started) {
      setInstallOpen(false);
    }
  };

  const startUpdate = async () => {
    await cliJob.start(() => api.updateCli(), "Updating the Unity CLI");
  };

  return (
    <div className="page-stack">
      {error ? <ErrorBanner error={error} onRetry={reload} /> : null}

      <section className="hero">
        <div className="hero-copy">
          <span className="eyebrow">Local Unity toolchain</span>
          <h1>One desk for the Unity CLI.</h1>
          <p>
            Check the CLI, manage Editors and modules, upgrade project versions, and read
            release notes — without leaving the GUI.
          </p>
        </div>
        <div className="hero-mark" aria-hidden="true">
          <span>U</span>
        </div>
      </section>

      <div className="overview-grid">
        <Card
          title="Unity CLI"
          subtitle={loading ? "Checking the local installation…" : "The engine behind every action in this app."}
          className="span-2"
          actions={
            <Button
              size="sm"
              variant="ghost"
              icon={<IconRefresh size={15} />}
              onClick={reload}
              loading={loading}
            >
              Re-check
            </Button>
          }
        >
          {loading ? (
            <Spinner label="Resolving the Unity CLI" />
          ) : status?.installed ? (
            <div className="cli-summary">
              <div className="cli-summary-head">
                <div className="cli-version">
                  <span className="cli-version-value">{status.version?.version ?? "unknown"}</span>
                  {status.version?.channel ? (
                    <Badge tone="orange">{status.version.channel}</Badge>
                  ) : null}
                  {update.loading ? (
                    <Badge tone="blue">checking updates</Badge>
                  ) : update.error ? (
                    <Badge tone="neutral">update check failed</Badge>
                  ) : update.data?.updateAvailable ? (
                    <Badge tone="amber">update available</Badge>
                  ) : (
                    <Badge tone="green">up to date</Badge>
                  )}
                </div>
                <div className="inline-actions">
                  {update.data?.updateAvailable ? (
                    <Button
                      size="sm"
                      variant="primary"
                      icon={<IconDownload size={15} />}
                      onClick={startUpdate}
                    >
                      Update to {update.data.latestVersion}
                    </Button>
                  ) : null}
                  <Button
                    size="sm"
                    variant="secondary"
                    icon={<IconRefresh size={15} />}
                    onClick={() => update.reload()}
                    loading={update.loading}
                  >
                    Check again
                  </Button>
                </div>
              </div>

              <dl className="detail-grid">
                <Detail label="Platform" value={`${status.version?.platform ?? "—"} ${status.version?.arch ?? ""}`} />
                <Detail label="Runtime" value={status.version?.runtime ?? "—"} />
                <Detail label="Executable" value={status.path ?? "—"} mono />
                <Detail label="Resolved via" value={status.pathSource ?? "—"} />
                <Detail label="Editor install path" value={status.installPath ?? "—"} mono />
                <Detail label="Build date" value={formatIsoDate(status.version?.buildDate)} />
              </dl>

              {status.error ? <ErrorBanner error={status.error} compact /> : null}
              {update.error ? <ErrorBanner error={update.error} compact onRetry={() => update.reload()} /> : null}
            </div>
          ) : (
            <div className="install-callout">
              <div className="install-callout-icon">
                <IconAlert />
              </div>
              <div>
                <h3>The Unity CLI is not installed.</h3>
                <p>
                  This app is a GUI over <code>unity</code>. Install Unity's official CLI to
                  unlock Editors, projects, and release notes.
                </p>
                {status?.error ? <ErrorBanner error={status.error} compact /> : null}
              </div>
              <Button variant="primary" icon={<IconDownload size={16} />} onClick={openInstall}>
                Install Unity CLI
              </Button>
            </div>
          )}
        </Card>

        <Card title="Account" subtitle="Read-only sign-in state">
          {auth.loading ? (
            <Spinner label="Checking sign-in" />
          ) : auth.error ? (
            <p className="muted">Sign-in status unavailable. {auth.error.message}</p>
          ) : auth.data?.loggedIn ? (
            <div className="account-card">
              <div className="account-avatar">
                {(auth.data.user?.name || "U").slice(0, 1).toUpperCase()}
              </div>
              <div>
                <strong>{auth.data.user?.name ?? "Signed in"}</strong>
                <p className="muted">{auth.data.user?.email ?? "—"}</p>
                <Badge tone={auth.data.sessionState === "valid" ? "green" : "amber"}>
                  {auth.data.sessionState ?? "signed in"}
                </Badge>
              </div>
            </div>
          ) : (
            <p className="muted">Not signed in. The CLI may ask for a Unity sign-in when needed.</p>
          )}

          <div className="divider" />

          {license.loading ? (
            <Spinner label="Checking license" />
          ) : license.error ? (
            <p className="muted">License status unavailable. {license.error.message}</p>
          ) : license.data ? (
            <div className="license-list">
              <div className="inline-between">
                <span className="muted">License</span>
                <Badge tone={license.data.active ? "green" : "red"}>
                  {license.data.active ? "active" : "inactive"}
                </Badge>
              </div>
              {license.data.licenses.length === 0 ? (
                <p className="muted">No licenses reported.</p>
              ) : (
                license.data.licenses.map((item) => (
                  <div className="license-row" key={`${item.product}-${item.type}`}>
                    <span>{item.product}</span>
                    <Badge tone="neutral">{item.type}</Badge>
                  </div>
                ))
              )}
            </div>
          ) : null}
        </Card>

        <Card
          title="Running Editors"
          subtitle="Detected through the Unity Pipeline connection"
          actions={
            <Button size="sm" variant="ghost" icon={<IconRefresh size={15} />} onClick={() => running.reload()} />
          }
        >
          {running.loading ? (
            <Spinner />
          ) : running.error ? (
            <p className="muted">{running.error.message}</p>
          ) : running.data && running.data.count > 0 ? (
            <div className="running-list">
              {running.data.instances.map((instance, index) => (
                <div className="running-row" key={`${instance.projectPath ?? "editor"}-${index}`}>
                  <span className="status-dot status-green" />
                  <div>
                    <strong>{instance.projectPath ?? "Unity Editor"}</strong>
                    <p className="muted">
                      {instance.editorVersion ?? "unknown version"}
                      {instance.pid ? ` · pid ${instance.pid}` : ""}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <EmptyState
              icon={<IconBox size={26} />}
              title="No Editor running"
              description="Open a project and install the Pipeline package to see live Editors here."
            />
          )}
        </Card>

        <Card title="Jump back in" subtitle="The three screens you will use most">
          <div className="quick-links">
            <button className="quick-link" onClick={() => navigate("editors")}>
              <span className="quick-icon"><IconBox size={18} /></span>
              <span>
                <strong>Editors</strong>
                <small>Installed versions, modules, releases</small>
              </span>
              <IconPlay size={14} />
            </button>
            <button className="quick-link" onClick={() => navigate("projects")}>
              <span className="quick-icon"><IconFolder size={18} /></span>
              <span>
                <strong>Projects</strong>
                <small>Versions, upgrade targets, paths</small>
              </span>
              <IconPlay size={14} />
            </button>
            <button className="quick-link" onClick={() => navigate("changelog")}>
              <span className="quick-icon"><IconScroll size={18} /></span>
              <span>
                <strong>Changelog</strong>
                <small>Beta notes and breaking changes</small>
              </span>
              <IconPlay size={14} />
            </button>
          </div>
        </Card>
      </div>

      <JobPanel job={cliJob.job} onCancel={cliJob.cancel} onDismiss={cliJob.reset} />

      <Modal
        open={installOpen}
        title="Install the Unity CLI"
        subtitle="Unity's official installer runs locally. This app does not bundle the CLI."
        onClose={() => setInstallOpen(false)}
        footer={
          <>
            <Button variant="ghost" onClick={() => setInstallOpen(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              icon={<IconDownload size={16} />}
              disabled={!consent}
              onClick={startInstall}
            >
              Run installer
            </Button>
          </>
        }
      >
        <div className="form-stack">
          <label className="field">
            <span>Release channel</span>
            <select value={channel} onChange={(event) => setChannel(event.target.value)}>
              <option value="stable">Stable</option>
              <option value="beta">Beta</option>
              <option value="alpha">Alpha</option>
            </select>
          </label>

          <div className="command-preview">
            <div className="command-preview-head">
              <span>Official installer command</span>
              <Badge tone={isWindows() ? "blue" : "neutral"}>
                {isWindows() ? "Windows" : "Linux / macOS"}
              </Badge>
            </div>
            <pre>{isWindows()
              ? "irm https://unity.com/install.ps1 | iex"
              : "curl -fsSL https://unity.com/install.sh | bash"}</pre>
            <p className="muted small">
              The app adds <code>UNITY_CLI_SKIP_SETUP=1</code> so the installer's interactive
              agent setup does not block inside the GUI.
            </p>
          </div>

          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={consent}
              onChange={(event) => setConsent(event.target.checked)}
            />
            <span>
              I understand this downloads and executes Unity's installer script from
              unity.com. The installer may modify PATH and will place a binary under my user
              profile.
            </span>
          </label>

          <a
            className="external-link"
            href="https://docs.unity.com/en-us/unity-cli/unity-cli"
            target="_blank"
            rel="noreferrer"
          >
            Unity CLI documentation <IconExternal size={14} />
          </a>
        </div>
      </Modal>
    </div>
  );
}

function Detail({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="detail-item">
      <dt>{label}</dt>
      <dd className={mono ? "mono" : undefined} title={value}>
        {value || "—"}
      </dd>
    </div>
  );
}

function formatIsoDate(value?: string | null): string {
  if (!value) {
    return "—";
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleDateString(undefined, {
        year: "numeric",
        month: "short",
        day: "numeric",
      });
}

