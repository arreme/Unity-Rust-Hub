import { useEffect, useState } from "react";
import { api, normalizeError } from "../../api/client";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Spinner } from "../../components/Progress";
import { useToast } from "../../components/Toast";
import { IconCheck, IconExternal, IconSettings } from "../../components/icons";
import { useAsync } from "../../hooks/useAsync";

export function SettingsPage({ onCliChanged }: { onCliChanged: () => void }) {
  const { push } = useToast();
  const settings = useAsync(() => api.getSettings(), []);
  const [path, setPath] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<ReturnType<typeof normalizeError> | null>(null);

  useEffect(() => {
    setPath(settings.data?.cliPathOverride ?? "");
  }, [settings.data?.cliPathOverride]);

  const save = async (value: string | null) => {
    setSaving(true);
    setError(null);
    try {
      const next = await api.setCliPath(value);
      settings.setData(next);
      onCliChanged();
      push("success", value ? "CLI path saved." : "CLI path override cleared.");
    } catch (caught) {
      const normalized = normalizeError(caught);
      setError(normalized);
      push("error", normalized.message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="page-stack">
      <header className="page-header">
        <div>
          <span className="eyebrow">Configuration</span>
          <h1>Settings</h1>
          <p>Point the app at a specific CLI binary when auto-detection is not enough.</p>
        </div>
      </header>

      {settings.error ? <ErrorBanner error={settings.error} onRetry={settings.reload} /> : null}
      {error ? <ErrorBanner error={error} /> : null}

      <div className="two-column settings-columns">
        <Card title="Unity CLI location" subtitle="Resolution order used on every call">
          {settings.loading ? (
            <Spinner label="Reading settings" />
          ) : (
            <div className="form-stack">
              <dl className="detail-grid">
                <div className="detail-item">
                  <dt>Resolved executable</dt>
                  <dd className="mono small">{settings.data?.resolvedPath ?? "Not found"}</dd>
                </div>
                <div className="detail-item">
                  <dt>Resolved via</dt>
                  <dd>{settings.data?.resolvedSource ?? "—"}</dd>
                </div>
              </dl>

              <label className="field">
                <span>Manual CLI path override</span>
                <input
                  value={path}
                  onChange={(event) => setPath(event.target.value)}
                  placeholder="/home/you/.local/bin/unity"
                  spellCheck={false}
                />
                <span className="hint">
                  Leave empty to use the install receipt, <code>UNITY_CLI_HOME</code>, platform
                  defaults, and <code>PATH</code>.
                </span>
              </label>

              <div className="inline-actions wrap">
                <Button
                  variant="primary"
                  icon={<IconCheck size={15} />}
                  loading={saving}
                  onClick={() => save(path.trim() || null)}
                >
                  Save path
                </Button>
                <Button
                  variant="ghost"
                  loading={saving}
                  onClick={() => {
                    setPath("");
                    void save(null);
                  }}
                  disabled={!settings.data?.cliPathOverride && !path}
                >
                  Clear override
                </Button>
              </div>
            </div>
          )}
        </Card>

        <Card title="App data" subtitle="Where this app keeps its settings">
          <dl className="detail-grid">
            <div className="detail-item">
              <dt>Config directory</dt>
              <dd className="mono small">{settings.data?.configDir ?? "—"}</dd>
            </div>
            <div className="detail-item">
              <dt>Settings file</dt>
              <dd className="mono small">
                {settings.data?.configDir ? `${settings.data.configDir}/settings.json` : "—"}
              </dd>
            </div>
          </dl>
          <p className="muted small">
            Nothing is stored except the optional CLI path. Credentials and Unity account
            state stay in the CLI and the OS keyring.
          </p>
        </Card>

        <Card title="About" subtitle="Unity Rust Hub">
          <div className="about-list">
            <div className="inline-between">
              <span className="muted">App version</span>
              <Badge tone="neutral" mono>0.1.0</Badge>
            </div>
            <div className="inline-between">
              <span className="muted">UI</span>
              <span>React + TypeScript + Vite</span>
            </div>
            <div className="inline-between">
              <span className="muted">Desktop shell</span>
              <span>Tauri v2 + Rust</span>
            </div>
            <div className="inline-between">
              <span className="muted">CLI adapter</span>
              <span><code>unity-core</code> crate</span>
            </div>
          </div>

          <div className="divider" />

          <div className="about-links">
            <a href="https://docs.unity.com/en-us/unity-cli/unity-cli" target="_blank" rel="noreferrer">
              Unity CLI documentation <IconExternal size={14} />
            </a>
            <a href="https://docs.unity.com/en-us/unity-cli/unity-cli-reference" target="_blank" rel="noreferrer">
              Unity CLI reference <IconExternal size={14} />
            </a>
            <a href="https://docs.unity.com/en-us/unity-cli/release-notes" target="_blank" rel="noreferrer">
              Unity CLI release notes <IconExternal size={14} />
            </a>
          </div>

          <div className="divider" />
          <p className="muted small">
            <IconSettings size={13} /> Linux builds run on WebKitGTK 4.1
            (<code>webkit2gtk-4.1</code>). Windows uses WebView2; macOS uses the system
            WKWebView.
          </p>
        </Card>
      </div>
    </div>
  );
}
