import { useState } from "react";
import { api } from "./api/client";
import type { Page } from "./api/types";
import { Badge } from "./components/Badge";
import { ChangelogPage } from "./features/changelog/ChangelogPage";
import { EditorsPage } from "./features/editors/EditorsPage";
import { Overview } from "./features/overview/Overview";
import { ProjectsPage } from "./features/projects/ProjectsPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import {
  IconBox,
  IconFolder,
  IconGrid,
  IconRefresh,
  IconScroll,
  IconSettings,
} from "./components/icons";
import { useAsync } from "./hooks/useAsync";

const pages: { id: Page; label: string; icon: typeof IconGrid }[] = [
  { id: "overview", label: "Overview", icon: IconGrid },
  { id: "editors", label: "Editors", icon: IconBox },
  { id: "projects", label: "Projects", icon: IconFolder },
  { id: "changelog", label: "Changelog", icon: IconScroll },
  { id: "settings", label: "Settings", icon: IconSettings },
];

export default function App() {
  const [page, setPage] = useState<Page>("overview");
  const status = useAsync(() => api.getCliStatus(), []);

  const installed = Boolean(status.data?.installed);
  const version = status.data?.version?.version;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">U</div>
          <div>
            <div className="brand-name">Unity Rust Hub</div>
            <div className="brand-sub">Unity CLI, tamed.</div>
          </div>
        </div>

        <nav className="sidebar-nav" aria-label="Primary">
          {pages.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                className={`nav-item ${page === item.id ? "active" : ""}`.trim()}
                onClick={() => setPage(item.id)}
              >
                <Icon size={17} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>

        <div className="sidebar-footer">
          <div className="cli-pill">
            <span className={`status-dot ${installed ? "status-green" : "status-red"}`} />
            <div>
              <span>{installed ? "CLI connected" : "CLI missing"}</span>
              <small>{version ?? "not detected"}</small>
            </div>
          </div>
          <p className="sidebar-note">
            Built on Tauri v2. All mutations go through the official Unity CLI.
          </p>
        </div>
      </aside>

      <div className="main">
        <header className="topbar">
          <div className="topbar-crumb">
            <span>{pages.find((item) => item.id === page)?.label}</span>
            <span className="topbar-dot">/</span>
            <span className="muted">local machine</span>
          </div>
          <div className="topbar-actions">
            {status.loading ? (
              <Badge tone="blue">checking CLI…</Badge>
            ) : installed ? (
              <Badge tone="green" mono>{version}</Badge>
            ) : (
              <Badge tone="red">CLI not found</Badge>
            )}
            <button
              className="icon-button"
              onClick={() => {
                status.reload();
              }}
              aria-label="Refresh CLI status"
              title="Refresh CLI status"
            >
              <IconRefresh size={16} />
            </button>
          </div>
        </header>

        <main className="content">
          {page === "overview" ? (
            <Overview
              status={status.data}
              loading={status.loading}
              error={status.error}
              reload={status.reload}
              navigate={setPage}
            />
          ) : page === "editors" ? (
            <EditorsPage />
          ) : page === "projects" ? (
            <ProjectsPage navigate={setPage} />
          ) : page === "changelog" ? (
            <ChangelogPage />
          ) : (
            <SettingsPage onCliChanged={status.reload} />
          )}
        </main>
      </div>
    </div>
  );
}
