import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import type { Page, Project } from "../../api/types";
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
  IconFolder,
  IconPlay,
  IconRefresh,
  IconSearch,
} from "../../components/icons";
import { useAsync } from "../../hooks/useAsync";
import { useJob } from "../../hooks/useJob";
import { formatEpochMillis, shortPath } from "../../lib/format";

export function ProjectsPage({ navigate }: { navigate: (page: Page) => void }) {
  const { push } = useToast();
  const projects = useAsync(() => api.listProjects(), []);
  const editors = useAsync(() => api.listEditors(), []);
  const job = useJob();

  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<Project | null>(null);
  const [targetVersion, setTargetVersion] = useState("");
  const [confirmed, setConfirmed] = useState(false);

  const installedVersions = useMemo(
    () => (editors.data ?? []).map((editor) => editor.version),
    [editors.data],
  );

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase();
    if (!needle) {
      return projects.data ?? [];
    }
    return (projects.data ?? []).filter((project) =>
      [project.title, project.projectName, project.path, project.version]
        .filter(Boolean)
        .some((value) => String(value).toLowerCase().includes(needle)),
    );
  }, [projects.data, search]);

  const jobState = job.job?.state;
  useEffect(() => {
    if (jobState === "succeeded") {
      push("success", "Project upgrade started and finished.");
      projects.reload();
    } else if (jobState === "failed" && job.job?.error) {
      push("error", job.job.error.message);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jobState]);

  const openUpgrade = (project: Project) => {
    setSelected(project);
    const candidates = installedVersions.filter((version) => version !== project.version);
    setTargetVersion(candidates[0] ?? installedVersions[0] ?? project.version);
    setConfirmed(false);
  };

  const startUpgrade = async () => {
    if (!selected || !targetVersion) {
      return;
    }
    const started = await job.start(
      () => api.upgradeProject(selected.path, targetVersion),
      `Upgrading ${selected.title}`,
    );
    if (started) {
      setSelected(null);
    }
  };

  const missingEditor = (project: Project) =>
    installedVersions.length > 0 && !installedVersions.includes(project.version);

  return (
    <div className="page-stack">
      <header className="page-header">
        <div>
          <span className="eyebrow">Hub registry</span>
          <h1>Projects</h1>
          <p>Every project the Unity CLI knows about, its Editor version, and a safe path to upgrade.</p>
        </div>
        <div className="inline-actions">
          <Button
            variant="secondary"
            icon={<IconRefresh size={15} />}
            onClick={() => {
              projects.reload();
              editors.reload();
            }}
            loading={projects.loading || editors.loading}
          >
            Refresh
          </Button>
        </div>
      </header>

      {projects.error ? <ErrorBanner error={projects.error} onRetry={projects.reload} /> : null}

      <Card
        title="Registered projects"
        subtitle={`${filtered.length} of ${projects.data?.length ?? 0} shown`}
        flush
        actions={
          <div className="search-field">
            <IconSearch size={15} />
            <input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Filter by name, path, or version"
              spellCheck={false}
            />
          </div>
        }
      >
        {projects.loading ? (
          <div className="pad"><Spinner label="Reading projects" /></div>
        ) : (projects.data?.length ?? 0) === 0 ? (
          <EmptyState
            icon={<IconFolder size={28} />}
            title="No Unity projects registered"
            description="Add a project folder to the Unity Hub registry, or create a project with the Unity CLI, then refresh."
            action={
              <Button variant="secondary" onClick={() => projects.reload()}>
                Refresh
              </Button>
            }
          />
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={<IconSearch size={26} />}
            title="No matching projects"
            description="Try a shorter search or clear the filter."
            action={
              <Button variant="ghost" onClick={() => setSearch("")}>
                Clear filter
              </Button>
            }
          />
        ) : (
          <div className="table-wrap">
            <table className="data-table projects-table">
              <thead>
                <tr>
                  <th>Project</th>
                  <th>Editor version</th>
                  <th>Pipeline</th>
                  <th>Target</th>
                  <th>Path</th>
                  <th>Last modified</th>
                  <th className="align-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((project) => (
                  <tr key={project.path}>
                    <td>
                      <div className="cell-title">
                        <span className="strong">
                          {project.isFavorite ? <span className="favorite" title="Favorite">★</span> : null}
                          {project.title || project.projectName || "Untitled project"}
                        </span>
                        <div className="badge-row">
                          {project.cloudEnabled ? <Badge tone="blue">Unity Cloud</Badge> : null}
                          {project.vcsProvider ? <Badge tone="neutral">{project.vcsProvider}</Badge> : null}
                          {missingEditor(project) ? (
                            <Badge tone="amber" >
                              <IconAlert size={12} /> Editor not installed
                            </Badge>
                          ) : null}
                        </div>
                      </div>
                    </td>
                    <td><span className="mono strong">{project.version}</span></td>
                    <td>{project.renderPipeline ? <Badge tone="violet">{project.renderPipeline}</Badge> : <span className="muted">—</span>}</td>
                    <td>{project.buildTarget ? <span className="mono small">{project.buildTarget}</span> : <span className="muted">—</span>}</td>
                    <td className="mono muted small" title={project.path}>{shortPath(project.path, 54)}</td>
                    <td className="muted small">{formatEpochMillis(project.lastModified)}</td>
                    <td className="align-right">
                      <Button
                        size="sm"
                        variant="secondary"
                        icon={<IconPlay size={14} />}
                        onClick={() => openUpgrade(project)}
                      >
                        Change version
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      {editors.error ? (
        <ErrorBanner error={editors.error} compact onRetry={editors.reload} />
      ) : null}

      <JobPanel job={job.job} onCancel={job.cancel} onDismiss={job.reset} />

      <Modal
        open={Boolean(selected)}
        title="Change project Editor version"
        subtitle={selected?.title}
        onClose={() => setSelected(null)}
        footer={
          <>
            <Button variant="ghost" onClick={() => setSelected(null)}>Cancel</Button>
            <Button
              variant="primary"
              icon={<IconPlay size={15} />}
              disabled={!confirmed || !targetVersion || targetVersion === selected?.version}
              onClick={startUpgrade}
            >
              Upgrade project
            </Button>
          </>
        }
      >
        {selected ? (
          <div className="form-stack">
            <dl className="detail-grid">
              <div className="detail-item">
                <dt>Current version</dt>
                <dd className="mono strong">{selected.version}</dd>
              </div>
              <div className="detail-item">
                <dt>Project path</dt>
                <dd className="mono small">{selected.path}</dd>
              </div>
            </dl>

            {installedVersions.length === 0 ? (
              <div className="install-callout">
                <div className="install-callout-icon"><IconAlert /></div>
                <div>
                  <h3>No Editors installed</h3>
                  <p>Install a target Editor version before upgrading a project.</p>
                </div>
                <Button variant="secondary" onClick={() => navigate("editors")}>
                  Go to Editors
                </Button>
              </div>
            ) : (
              <label className="field">
                <span>Target Editor version</span>
                <select
                  value={targetVersion}
                  onChange={(event) => {
                    setTargetVersion(event.target.value);
                    setConfirmed(false);
                  }}
                >
                  {installedVersions.map((version) => (
                    <option key={version} value={version}>
                      {version}
                      {version === selected.version ? " (current)" : ""}
                    </option>
                  ))}
                </select>
                <span className="hint">
                  Only installed Editors are offered. Install another version on the Editors
                  screen first if you need it.
                </span>
              </label>
            )}

            <div className="warning-box">
              <IconAlert size={18} />
              <div>
                <strong>This changes the project on disk.</strong>
                <p>
                  The CLI runs <code>unity projects upgrade &lt;path&gt; --to {targetVersion || "<version>"} --yes</code>.
                  It can update <code>ProjectVersion.txt</code>, open the Editor, and run API
                  upgrades. Commit or back up the project first.
                </p>
              </div>
            </div>

            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={confirmed}
                onChange={(event) => setConfirmed(event.target.checked)}
              />
              <span>I understand and want to upgrade this project now.</span>
            </label>
          </div>
        ) : null}
      </Modal>
    </div>
  );
}
