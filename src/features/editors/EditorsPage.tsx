import { useEffect, useMemo, useState } from "react";
import { api, normalizeError } from "../../api/client";
import type { Editor, InstallPreview, Module, UnityError } from "../../api/types";
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
  IconCheck,
  IconDownload,
  IconExternal,
  IconPlay,
  IconRefresh,
  IconSettings,
} from "../../components/icons";
import { useAsync } from "../../hooks/useAsync";
import { useJob } from "../../hooks/useJob";
import { formatBytes } from "../../lib/format";

const streamTone = (stream: string) => {
  const value = stream.toLowerCase();
  if (value.includes("lts") || value.includes("supported")) return "green";
  if (value.includes("beta")) return "amber";
  if (value.includes("alpha")) return "red";
  return "neutral";
};

export function EditorsPage() {
  const { push } = useToast();
  const editors = useAsync(() => api.listEditors(), []);
  const releases = useAsync(() => api.listReleases(30, 0), []);
  const job = useJob();

  const [installVersion, setInstallVersion] = useState("lts");
  const [installModules, setInstallModules] = useState<Module[]>([]);
  const [installModulesLoading, setInstallModulesLoading] = useState(false);
  const [installModulesError, setInstallModulesError] = useState<UnityError | null>(null);
  const [selectedModules, setSelectedModules] = useState<Set<string>>(new Set());
  const [acceptEula, setAcceptEula] = useState(false);
  const [preview, setPreview] = useState<InstallPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  const [moduleEditor, setModuleEditor] = useState<Editor | null>(null);
  const [dialogModules, setDialogModules] = useState<Module[]>([]);
  const [dialogLoading, setDialogLoading] = useState(false);
  const [dialogError, setDialogError] = useState<UnityError | null>(null);
  const [dialogSelected, setDialogSelected] = useState<Set<string>>(new Set());
  const [dialogEula, setDialogEula] = useState(false);

  const installedVersions = useMemo(
    () => new Set((editors.data ?? []).map((editor) => editor.version)),
    [editors.data],
  );

  const jobState = job.job?.state;
  useEffect(() => {
    if (jobState === "succeeded") {
      push("success", "Unity operation finished.");
      editors.reload();
      releases.reload();
      setPreview(null);
      setSelectedModules(new Set());
      setAcceptEula(false);
    } else if (jobState === "failed" && job.job?.error) {
      push("error", job.job.error.message);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jobState]);

  useEffect(() => {
    if (!moduleEditor) {
      return;
    }
    let alive = true;
    setDialogLoading(true);
    setDialogError(null);
    setDialogSelected(new Set());
    setDialogEula(false);
    api
      .listModules(moduleEditor.version)
      .then((modules) => {
        if (alive) {
          setDialogModules(modules);
        }
      })
      .catch((error: unknown) => {
        if (alive) {
          setDialogError(normalizeError(error));
          setDialogModules([]);
        }
      })
      .finally(() => {
        if (alive) {
          setDialogLoading(false);
        }
      });
    return () => {
      alive = false;
    };
  }, [moduleEditor]);

  const loadInstallModules = async (version: string) => {
    setInstallModulesLoading(true);
    setInstallModulesError(null);
    setInstallModules([]);
    setSelectedModules(new Set());
    setPreview(null);
    try {
      const modules = await api.listModules(version);
      setInstallModules(modules);
    } catch (error) {
      setInstallModulesError(normalizeError(error));
    } finally {
      setInstallModulesLoading(false);
    }
  };

  const useRelease = (version: string) => {
    setInstallVersion(version);
    void loadInstallModules(version);
  };

  const toggleInstallModule = (id: string) => {
    setSelectedModules((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    setPreview(null);
  };

  const toggleDialogModule = (id: string) => {
    setDialogSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const previewInstall = async () => {
    setPreviewLoading(true);
    setInstallModulesError(null);
    try {
      const data = await api.dryRunInstall(installVersion, [...selectedModules]);
      setPreview(data);
    } catch (error) {
      setInstallModulesError(normalizeError(error));
    } finally {
      setPreviewLoading(false);
    }
  };

  const startEditorInstall = async () => {
    await job.start(
      () => api.installEditor(installVersion, [...selectedModules], acceptEula),
      `Installing Unity ${installVersion}`,
    );
  };

  const startModuleInstall = async () => {
    if (!moduleEditor) {
      return;
    }
    const started = await job.start(
      () => api.installModules(moduleEditor.version, [...dialogSelected], dialogEula),
      `Installing modules for ${moduleEditor.version}`,
    );
    if (started) {
      setModuleEditor(null);
    }
  };

  return (
    <div className="page-stack">
      <header className="page-header">
        <div>
          <span className="eyebrow">Toolchain</span>
          <h1>Editors &amp; modules</h1>
          <p>See what is installed, add modules, and install a new Editor version.</p>
        </div>
        <div className="inline-actions">
          <Button
            variant="secondary"
            icon={<IconRefresh size={15} />}
            onClick={() => {
              editors.reload();
              releases.reload();
            }}
            loading={editors.loading || releases.loading}
          >
            Refresh
          </Button>
        </div>
      </header>

      {editors.error ? <ErrorBanner error={editors.error} onRetry={editors.reload} /> : null}

      <Card
        title="Installed Editors"
        subtitle={`${editors.data?.length ?? 0} version${(editors.data?.length ?? 0) === 1 ? "" : "s"} available locally`}
        flush
        actions={
          <Button size="sm" variant="ghost" icon={<IconRefresh size={15} />} onClick={editors.reload} />
        }
      >
        {editors.loading ? (
          <div className="pad"><Spinner label="Reading installed Editors" /></div>
        ) : (editors.data?.length ?? 0) === 0 ? (
          <EmptyState
            icon={<IconBox size={28} />}
            title="No Editors installed"
            description="Use the installer below to add a Unity version. LTS is the safest default."
            action={
              <Button
                variant="primary"
                icon={<IconDownload size={15} />}
                onClick={() => document.getElementById("install-editor")?.scrollIntoView({ behavior: "smooth" })}
              >
                Install an Editor
              </Button>
            }
          />
        ) : (
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Version</th>
                  <th>Architecture</th>
                  <th>Default</th>
                  <th>Installed modules</th>
                  <th>Location</th>
                  <th className="align-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {editors.data?.map((editor) => (
                  <tr key={`${editor.version}-${editor.architecture}`}>
                    <td>
                      <div className="cell-title">
                        <span className="mono strong">{editor.version}</span>
                        {editor.alias && editor.alias !== editor.version ? (
                          <span className="muted small">{editor.alias}</span>
                        ) : null}
                      </div>
                    </td>
                    <td><Badge tone="neutral">{editor.architecture || "—"}</Badge></td>
                    <td>
                      {editor.default ? <Badge tone="green">default</Badge> : <span className="muted">—</span>}
                    </td>
                    <td>
                      {editor.moduleNames.length === 0 ? (
                        <span className="muted">No modules reported</span>
                      ) : (
                        <div className="badge-row">
                          {editor.moduleNames.slice(0, 3).map((name) => (
                            <Badge key={name} tone="blue">{name}</Badge>
                          ))}
                          {editor.moduleNames.length > 3 ? (
                            <Badge tone="neutral">+{editor.moduleNames.length - 3}</Badge>
                          ) : null}
                        </div>
                      )}
                    </td>
                    <td className="mono muted small" title={editor.location}>{editor.location}</td>
                    <td className="align-right">
                      <Button
                        size="sm"
                        variant="secondary"
                        icon={<IconSettings size={14} />}
                        onClick={() => setModuleEditor(editor)}
                      >
                        Modules
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <div className="two-column">
        <Card
          id="install-editor"
          title="Install a new Editor"
          subtitle="The CLI downloads the Editor and any modules you select."
          className="install-card"
        >
          <div className="form-stack">
            <label className="field">
              <span>Editor version or alias</span>
              <div className="input-row">
                <input
                  value={installVersion}
                  onChange={(event) => setInstallVersion(event.target.value)}
                  placeholder="lts, latest, 6000.3, 6000.3.15f1"
                  spellCheck={false}
                />
                <Button
                  variant="secondary"
                  onClick={() => loadInstallModules(installVersion)}
                  loading={installModulesLoading}
                >
                  Load modules
                </Button>
              </div>
              <span className="hint">
                Aliases: <code>lts</code>, <code>latest</code>, or a stream such as{" "}
                <code>6000.3</code>. Exact versions also work.
              </span>
            </label>

            <div className="alias-row">
              {releases.data?.slice(0, 6).map((release) => (
                <button
                  key={release.version}
                  className="alias-chip"
                  onClick={() => useRelease(release.version)}
                >
                  {release.version}
                  <Badge tone={streamTone(release.stream)}>{release.stream || "release"}</Badge>
                </button>
              ))}
            </div>

            {installModulesError ? (
              <ErrorBanner error={installModulesError} compact />
            ) : null}

            {installModulesLoading ? (
              <Spinner label="Loading module catalog" />
            ) : installModules.length > 0 ? (
              <>
                <div className="module-toolbar">
                  <span className="muted">{installModules.length} modules available</span>
                  <div className="inline-actions">
                    <button
                      className="link-button"
                      onClick={() => setSelectedModules(new Set(installModules.filter((m) => !m.isInstalled).map((m) => m.id)))}
                    >
                      Select all
                    </button>
                    <button className="link-button" onClick={() => setSelectedModules(new Set())}>
                      Clear
                    </button>
                  </div>
                </div>
                <ModulePicker
                  modules={installModules}
                  selected={selectedModules}
                  onToggle={toggleInstallModule}
                />
              </>
            ) : (
              <p className="muted small">Load a version to choose modules. Installing without modules is allowed.</p>
            )}

            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={acceptEula}
                onChange={(event) => setAcceptEula(event.target.checked)}
              />
              <span>
                Accept the module license agreements for this install (passes{" "}
                <code>--accept-eula</code>). Leave unchecked to stop and read them in the CLI.
              </span>
            </label>

            <div className="inline-actions wrap">
              <Button
                variant="secondary"
                icon={<IconPlay size={15} />}
                onClick={previewInstall}
                loading={previewLoading}
              >
                Preview download
              </Button>
              <Button
                variant="primary"
                icon={<IconDownload size={15} />}
                onClick={startEditorInstall}
                disabled={selectedModules.size > 0 && !acceptEula}
              >
                Install Editor
              </Button>
            </div>

            {preview ? (
              <div className="preview-details">
                <div className="preview-summary">
                  <div>
                    <span className="muted small">Editor</span>
                    <strong className="mono">{preview.editor?.version ?? installVersion}</strong>
                    {preview.editor?.architecture ? (
                      <Badge tone="neutral">{preview.editor.architecture}</Badge>
                    ) : null}
                  </div>
                  <div>
                    <span className="muted small">Total download</span>
                    <strong>{formatBytes(preview.totalDownloadSize ?? preview.editor?.downloadSize ?? 0)}</strong>
                  </div>
                  <div>
                    <span className="muted small">Modules</span>
                    <strong>{preview.modules?.length ?? selectedModules.size}</strong>
                  </div>
                  {preview.alreadyInstalled ? <Badge tone="amber">already installed</Badge> : null}
                </div>
                <details className="preview-raw">
                  <summary>Raw dry-run payload</summary>
                  <pre>{JSON.stringify(preview, null, 2)}</pre>
                </details>
              </div>
            ) : null}
          </div>
        </Card>

        <Card
          title="Available releases"
          subtitle="The newest releases from Unity's release feed."
          actions={
            <Button size="sm" variant="ghost" icon={<IconRefresh size={15} />} onClick={releases.reload} />
          }
        >
          {releases.loading ? (
            <Spinner label="Fetching releases" />
          ) : releases.error ? (
            <ErrorBanner error={releases.error} onRetry={releases.reload} compact />
          ) : releases.data?.length === 0 ? (
            <EmptyState icon={<IconAlert size={24} />} title="No releases returned" />
          ) : (
            <div className="release-list">
              {releases.data?.map((release) => (
                <div className="release-row" key={release.version}>
                  <div>
                    <span className="mono strong">{release.version}</span>
                    <div className="badge-row">
                      <Badge tone={streamTone(release.stream)}>{release.stream || "release"}</Badge>
                      {release.lts ? <Badge tone="green">LTS</Badge> : null}
                      {release.installed || installedVersions.has(release.version) ? (
                        <Badge tone="blue">
                          <IconCheck size={12} /> installed
                        </Badge>
                      ) : null}
                    </div>
                  </div>
                  <Button size="sm" variant="ghost" onClick={() => useRelease(release.version)}>
                    Use
                  </Button>
                </div>
              ))}
            </div>
          )}
          <a
            className="external-link"
            href="https://unity.com/releases/editor/archive"
            target="_blank"
            rel="noreferrer"
          >
            Unity Editor archive <IconExternal size={14} />
          </a>
        </Card>
      </div>

      <JobPanel job={job.job} onCancel={job.cancel} onDismiss={job.reset} />

      <Modal
        open={Boolean(moduleEditor)}
        title={`Modules for Unity ${moduleEditor?.version ?? ""}`}
        subtitle="Modules are installed by `unity install-modules`. Existing modules stay untouched."
        onClose={() => setModuleEditor(null)}
        wide
        footer={
          <>
            <Button variant="ghost" onClick={() => setModuleEditor(null)}>Cancel</Button>
            <Button
              variant="primary"
              icon={<IconDownload size={15} />}
              onClick={startModuleInstall}
              disabled={dialogSelected.size === 0 || !dialogEula}
            >
              Install {dialogSelected.size || ""} module{dialogSelected.size === 1 ? "" : "s"}
            </Button>
          </>
        }
      >
        {moduleEditor ? (
          <div className="form-stack">
            <div>
              <span className="muted small">Currently installed</span>
              <div className="badge-row">
                {moduleEditor.moduleNames.length === 0 ? (
                  <span className="muted">No modules reported for this Editor.</span>
                ) : (
                  moduleEditor.moduleNames.map((name) => (
                    <Badge key={name} tone="green">
                      <IconCheck size={12} /> {name}
                    </Badge>
                  ))
                )}
              </div>
            </div>

            {dialogError ? <ErrorBanner error={dialogError} compact /> : null}
            {dialogLoading ? (
              <Spinner label="Loading available modules" />
            ) : (
              <ModulePicker
                modules={dialogModules}
                selected={dialogSelected}
                onToggle={toggleDialogModule}
                lockInstalled
              />
            )}

            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={dialogEula}
                onChange={(event) => setDialogEula(event.target.checked)}
              />
              <span>I accept the module license agreements (`--accept-eula`).</span>
            </label>
          </div>
        ) : null}
      </Modal>
    </div>
  );
}

function ModulePicker({
  modules,
  selected,
  onToggle,
  lockInstalled = false,
}: {
  modules: Module[];
  selected: Set<string>;
  onToggle: (id: string) => void;
  lockInstalled?: boolean;
}) {
  const grouped = useMemo(() => {
    const map = new Map<string, Module[]>();
    for (const module of modules) {
      if (module.status && module.status.toLowerCase().includes("hidden")) {
        continue;
      }
      const category = module.category || "Other";
      const list = map.get(category) ?? [];
      list.push(module);
      map.set(category, list);
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [modules]);

  if (modules.length === 0) {
    return <p className="muted small">No modules returned for this version.</p>;
  }

  return (
    <div className="module-picker">
      {grouped.map(([category, entries]) => (
        <div className="module-group" key={category}>
          <div className="module-group-title">{category}</div>
          <div className="module-grid">
            {entries.map((module) => {
              const installed = module.isInstalled;
              const locked = lockInstalled && installed;
              return (
                <label
                  key={module.id}
                  className={`module-option ${locked ? "module-option-locked" : ""}`.trim()}
                  title={module.aliases ? `Aliases: ${module.aliases}` : module.id}
                >
                  <input
                    type="checkbox"
                    checked={selected.has(module.id) || locked}
                    disabled={locked}
                    onChange={() => onToggle(module.id)}
                  />
                  <span className="module-option-copy">
                    <span className="module-option-name">
                      {module.name || module.id}
                      {installed ? <Badge tone="green">installed</Badge> : null}
                    </span>
                    <span className="muted small">
                      {module.id} · {formatBytes(module.downloadBytes)} download
                    </span>
                  </span>
                </label>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}
