export interface UnityError {
  code: string;
  message: string;
  detail?: string | null;
  exitCode?: number | null;
  retryable: boolean;
  extra?: Record<string, unknown>;
}

export interface CliVersion {
  version: string;
  channel: string;
  commit?: string | null;
  buildDate?: string | null;
  platform: string;
  arch: string;
  runtime?: string | null;
}

export interface CliStatus {
  installed: boolean;
  path?: string | null;
  pathSource?: string | null;
  version?: CliVersion | null;
  installPath?: string | null;
  error?: UnityError | null;
}

export interface CliUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  platform: string;
  updateAvailable: boolean;
}

export interface Editor {
  version: string;
  alias: string;
  architecture: string;
  location: string;
  moduleNames: string[];
  modules: string;
  default: boolean;
}

export interface Release {
  version: string;
  architecture: string;
  stream: string;
  lts: boolean;
  installed: boolean;
  revisionHash?: string | null;
}

export interface Module {
  id: string;
  aliases: string;
  name: string;
  category: string;
  status: string;
  download: string;
  installed: string;
  downloadBytes: number;
  installedBytes: number;
  isInstalled: boolean;
}

export interface ModuleSize {
  value: number;
  unit: string;
}

export interface EditorInfoModule {
  id: string;
  name: string;
  description: string;
  downloadSize?: ModuleSize | null;
  installedSize?: ModuleSize | null;
  required: boolean;
  preSelected: boolean;
  hidden: boolean;
}

export interface EditorInfo {
  version: string;
  changeset?: string | null;
  stream: string;
  releaseDate?: string | null;
  releaseNotesUrl?: string | null;
  downloadUrl?: string | null;
  modules: EditorInfoModule[];
}

export interface Project {
  title: string;
  path: string;
  version: string;
  architecture: string;
  changeset?: string | null;
  isFavorite: boolean;
  cloudEnabled: boolean;
  vcsProvider?: string | null;
  renderPipeline?: string | null;
  buildTarget?: string | null;
  lastModified?: number | null;
  projectName?: string | null;
  organizationName?: string | null;
  repositoryName?: string | null;
  cloudProjectId?: string | null;
}

export interface RunningEditor {
  projectPath?: string | null;
  editorVersion?: string | null;
  pid?: number | null;
  port?: number | null;
  [key: string]: unknown;
}

export interface RunningEditors {
  count: number;
  instances: RunningEditor[];
}

export interface AuthUser {
  name: string;
  email: string;
}

export interface AuthStatus {
  loggedIn: boolean;
  sessionState?: string | null;
  mode?: string | null;
  credentialSource?: string | null;
  user?: AuthUser | null;
}

export interface License {
  product: string;
  type: string;
  organization: string;
  expires: string;
}

export interface LicenseStatus {
  active: boolean;
  signedIn: boolean;
  authMode?: string | null;
  licenses: License[];
  machineId?: string | null;
}

export interface ChangelogSection {
  title: string;
  entries: string[];
}

export interface BreakingChange {
  text: string;
  reason: string;
  section: string;
}

export interface ChangelogView {
  version: string;
  sections: ChangelogSection[];
  breakingChanges: BreakingChange[];
  entryCount: number;
  markdown: string;
  source: string;
  sourceDetail: string;
}

export interface JobStarted {
  jobId: string;
  kind: string;
  command: string;
}

export type JobState =
  | "starting"
  | "started"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

export interface JobUpdate {
  jobId: string;
  kind: string;
  state: JobState;
  stream?: "stdout" | "stderr" | null;
  line?: string | null;
  progress?: number | null;
  message?: string | null;
  result?: unknown;
  error?: UnityError | null;
  at: number;
}

export interface InstallPreviewEditor {
  version: string;
  architecture?: string;
  downloadSize?: number;
  checksum?: string;
}

export interface InstallPreviewModule {
  id: string;
  name?: string;
  downloadSize?: number;
  installedSize?: number;
}

export interface InstallPreview {
  alreadyInstalled?: boolean;
  editor?: InstallPreviewEditor;
  modules?: InstallPreviewModule[];
  totalDownloadSize?: number;
}

export interface SettingsView {
  cliPathOverride?: string | null;
  configDir?: string | null;
  resolvedPath?: string | null;
  resolvedSource?: string | null;
}

export type Page = "overview" | "editors" | "projects" | "changelog" | "settings";
