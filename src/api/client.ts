import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AuthStatus,
  ChangelogView,
  CliStatus,
  CliUpdateInfo,
  Editor,
  EditorInfo,
  InstallPreview,
  JobStarted,
  JobUpdate,
  LicenseStatus,
  Module,
  Project,
  Release,
  RunningEditors,
  SettingsView,
  UnityError,
} from "./types";

export const api = {
  getCliStatus: () => invoke<CliStatus>("get_cli_status"),
  checkCliUpdate: () => invoke<CliUpdateInfo>("check_cli_update"),
  getAuthStatus: () => invoke<AuthStatus>("get_auth_status"),
  getLicenseStatus: () => invoke<LicenseStatus>("get_license_status"),
  getInstallPath: () => invoke<string>("get_install_path"),
  listRunningEditors: () => invoke<RunningEditors>("list_running_editors"),

  listEditors: () => invoke<Editor[]>("list_editors"),
  listReleases: (limit = 25, skip = 0, stream?: string) =>
    invoke<Release[]>("list_releases", { limit, skip, stream: stream || null }),
  listModules: (version: string) => invoke<Module[]>("list_modules", { version }),
  getEditorInfo: (version: string) => invoke<EditorInfo>("get_editor_info", { version }),
  dryRunInstall: (version: string, modules: string[]) =>
    invoke<InstallPreview>("dry_run_install", { version, modules }),

  listProjects: () => invoke<Project[]>("list_projects"),

  getChangelog: (target?: string | null) =>
    invoke<ChangelogView>("get_changelog", { target: target || null }),

  installCli: (channel?: string | null) =>
    invoke<JobStarted>("install_cli", { channel: channel || null }),
  updateCli: () => invoke<JobStarted>("update_cli"),
  installEditor: (version: string, modules: string[], acceptEula: boolean) =>
    invoke<JobStarted>("install_editor", { version, modules, acceptEula }),
  installModules: (version: string, modules: string[], acceptEula: boolean) =>
    invoke<JobStarted>("install_modules", { version, modules, acceptEula }),
  upgradeProject: (path: string, version: string) =>
    invoke<JobStarted>("upgrade_project", { path, version }),
  cancelJob: (jobId: string) => invoke<void>("cancel_job", { jobId }),

  getSettings: () => invoke<SettingsView>("get_settings"),
  setCliPath: (path: string | null) => invoke<SettingsView>("set_cli_path", { path }),
};

export function subscribeToJobs(onUpdate: (update: JobUpdate) => void): Promise<UnlistenFn> {
  return listen<JobUpdate>("job:update", (event) => onUpdate(event.payload));
}

export function normalizeError(error: unknown): UnityError {
  if (typeof error === "string") {
    return { code: "UNKNOWN", message: error, retryable: false };
  }
  if (error && typeof error === "object") {
    const candidate = error as Partial<UnityError> & { message?: string };
    if (typeof candidate.message === "string") {
      return {
        code: typeof candidate.code === "string" ? candidate.code : "UNKNOWN",
        message: candidate.message,
        detail: typeof candidate.detail === "string" ? candidate.detail : null,
        exitCode: typeof candidate.exitCode === "number" ? candidate.exitCode : null,
        retryable: Boolean(candidate.retryable),
        extra: candidate.extra,
      };
    }
  }
  return {
    code: "UNKNOWN",
    message: "Something went wrong.",
    retryable: false,
  };
}
