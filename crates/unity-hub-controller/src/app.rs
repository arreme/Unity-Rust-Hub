use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::{Subscription, Task};
use serde_json::{Map, Value};
use unity_core::bootstrap;
use unity_core::changelog::{
    ChangelogEntry, ChangelogRequest, ChangelogView, CliChangelogProvider, DocsTopicKind,
};
use unity_core::docs::{DocsLookup, DocsPage, DocsScope, DOCS_NOT_FOUND};
use unity_core::errors::{redact_proxy, remediation_for, RemediationKind};
use unity_core::models::{
    AuthStatus, CliStatus, CliUpdateInfo, CreateProjectOptions, CreateProjectResult, DoctorCheck,
    Editor, EnvironmentInfo, EnvironmentProxy, InstallPreview, LicenseStatus, Module, Project,
    PruneCandidate, PruneReport, Release, RunningEditor, RunningEditors, SkillClientInfo,
    SkillRefreshRow, TemplateBrief, TemplateCreateOptions, TemplateFilter, TemplateInfo,
    TemplatePackOptions, TrackedSkillInstall, VerifyComponent,
};
use unity_core::surface::{cli_code, error_code};
use unity_core::versions::{
    validate_archive_path, validate_output_dir, validate_package_id, validate_project_name,
    validate_project_path, validate_template_version,
};
use unity_core::{
    CommandSpec, DiagnoseTopic as CliDiagnose, DoctorRead, EditorVerifyRead, InstallOptions,
    ModuleInstallOptions, SkillInstallOptions, SkillRefreshOptions, UnityCli, UnityError,
};
use unity_hub_model::{
    AccountCardVm, ArchitectureOptionVm, AuthVm, BadgeVm, BreakingChangeVm, ChangelogDataVm,
    ChangelogEntryVm, ChangelogSectionVm, ChangelogVm, CheckStatus, CliCardVm, CliChangelogVm,
    CliConnectionVm, CliDetailsVm, CliUpdateFlowVm, DiagnoseReportVm, DiagnoseTopic, DiagnosticsVm,
    DocsScopeVm, DoctorCheckVm, DoctorVm, EditorModulesVm, EditorRowVm, EditorsVm, EntryDocsVm,
    EnvironmentVm, ErrorActionKind, ErrorActionVm, InstallAdvancedVm, InstallCliVm,
    InstallEditorVm, InstallPreviewVm, JobPhase, JobVm, LicenseRowVm, LicenseVm, LoadState,
    ModalVm, ModuleOptionVm, NewProjectEditorVm, NewProjectField, NewProjectFormVm,
    NewProjectPreflightState, NewProjectPreflightVm, NewProjectResultVm, NewProjectStep,
    NewProjectVm, OverviewVm, Page, PageVm, ProcessMetricsVm, ProcessRowVm, ProcessesVm,
    ProjectRowVm, ProjectsVm, ProxyVm, PruneCandidateVm, PruneEditorsVm, PruneReportVm,
    ReleaseRowVm, RemoveProjectVm, ResumeInstallVm, RollbackCliVm, RunningEditorVm, SectionKind,
    SettingsVm, ShellVm, SkillClientVm, SkillForceVm, SkillInstallOptionsVm, SkillInstallVm,
    SkillOutcomeRowVm, SkillOutcomeVm, SkillRunVm, SkillsVm, TemplateAuthoringMode,
    TemplateAuthoringVm, TemplateDetailVm, TemplateField, TemplateFormVm, TemplateKindFilter,
    TemplateRowVm, TemplatesVm, TextScaleOptionVm, ToastVm, Tone, UiAction, UiError, UpdateVm,
    UpgradeProjectVm, VerifyCheckVm, VerifyEditorVm, VerifyResultVm, ViewModel,
};

use crate::jobs::{DownloadStatus, JobManager, JobStarted, JobState, StreamKind};
use crate::metrics::{ProcessMetrics, Sampler};
use crate::notifications::{remediation_args, ActionState, NotificationLevel, NotificationStore};
use crate::settings::{self, AppSettings, TextSize};
use crate::util::{
    error_text, format_bytes, format_duration, format_epoch_millis, format_iso, open_external_url,
    open_folder, project_context, run_blocking, short_path,
};

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    InstallCli,
    EditorModules(Editor),
    UpgradeProject(Project),
    RemoveProject(Project),
    NewProject(Box<NewProjectState>),
    TemplateAuthoring(Box<TemplateAuthoringState>),
    PruneEditors,
    VerifyEditor {
        version: String,
        architecture: String,
    },
    RollbackCli,
}

#[derive(Debug, Clone)]
pub struct NewProjectState {
    pub step: NewProjectStep,
    pub form: NewProjectFormVm,
    pub editors: Loadable<Vec<Editor>>,
    pub templates: Loadable<Vec<TemplateBrief>>,
    pub selected_template: Option<TemplateInfo>,
    pub license: Loadable<LicenseStatus>,
    pub issues: Vec<String>,
    pub result: Option<CreateProjectResult>,
}

#[derive(Debug, Clone, Default)]
pub struct TemplateForm {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct TemplateAuthoringState {
    pub mode: TemplateAuthoringMode,
    pub source: String,
    pub form: TemplateForm,
    pub overwrite_confirmed: bool,
    pub issues: Vec<String>,
    pub result: Option<String>,
}

impl NewProjectState {
    fn new() -> Self {
        Self {
            step: NewProjectStep::Details,
            form: NewProjectFormVm::default(),
            editors: Loadable::default(),
            templates: Loadable::default(),
            selected_template: None,
            license: Loadable::default(),
            issues: Vec::new(),
            result: None,
        }
    }

    /// Inline validation for the current step. Nothing mutates before the final
    /// confirm, so every rule here is checkable without the CLI.
    fn validation_issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        let check_details = matches!(self.step, NewProjectStep::Details | NewProjectStep::Review);
        let check_editor = matches!(self.step, NewProjectStep::Editor | NewProjectStep::Review);
        let check_template = matches!(self.step, NewProjectStep::Template | NewProjectStep::Review);

        if check_details {
            let name_ok = validate_project_name(&self.form.name).is_ok();
            if let Err(error) = validate_project_name(&self.form.name) {
                issues.push(error.message);
            }
            let parent = self.form.parent_dir.trim();
            if parent.is_empty() {
                issues.push("Choose the folder that will contain the project.".to_string());
            } else {
                let parent = std::path::Path::new(parent);
                if !parent.exists() {
                    issues.push(format!(
                        "The parent folder does not exist: {}",
                        parent.display()
                    ));
                } else if !parent.is_dir() {
                    issues.push(format!(
                        "The parent path is not a folder: {}",
                        parent.display()
                    ));
                } else if name_ok {
                    let target = parent.join(self.form.name.trim());
                    if target.exists() {
                        if target.is_file() {
                            issues.push(format!(
                                "A file already exists at the target path: {}",
                                target.display()
                            ));
                        } else if folder_is_not_empty(&target) {
                            issues.push(format!(
                                "The target folder already exists and is not empty: {}",
                                target.display()
                            ));
                        }
                    }
                }
            }
        }

        if check_editor {
            if self.form.editor_version.trim().is_empty() {
                issues.push("Choose an installed Unity Editor version.".to_string());
            }
            if !matches!(self.form.architecture.trim(), "x86_64" | "arm64") {
                issues.push("Choose the x86_64 or arm64 Editor architecture.".to_string());
            }
        }

        if check_template && self.form.template.trim().is_empty() {
            issues.push("Choose a project template.".to_string());
        }

        issues
    }
}

impl TemplateAuthoringState {
    fn new(mode: TemplateAuthoringMode) -> Self {
        Self {
            mode,
            source: String::new(),
            form: TemplateForm::default(),
            overwrite_confirmed: false,
            issues: Vec::new(),
            result: None,
        }
    }

    fn create_options(&self) -> TemplateCreateOptions {
        TemplateCreateOptions {
            project_path: self.source.clone(),
            name: self.form.name.trim().to_string(),
            display_name: self.form.display_name.trim().to_string(),
            description: self.form.description.trim().to_string(),
            version: self.form.version.trim().to_string(),
            output_dir: self.form.output.trim().to_string(),
            overwrite: self.overwrite_confirmed,
        }
    }

    fn pack_options(&self) -> TemplatePackOptions {
        TemplatePackOptions {
            project_path: self.source.clone(),
            output: self.form.output.trim().to_string(),
            name: self.form.name.trim().to_string(),
            display_name: self.form.display_name.trim().to_string(),
            description: self.form.description.trim().to_string(),
            version: self.form.version.trim().to_string(),
            overwrite: self.overwrite_confirmed,
        }
    }
}

fn folder_is_not_empty(path: &std::path::Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(true)
}

fn path_is_inside(path: &std::path::Path, parent: &std::path::Path) -> bool {
    let absolute = |value: &std::path::Path| {
        value
            .canonicalize()
            .or_else(|_| std::path::absolute(value))
            .unwrap_or_else(|_| value.to_path_buf())
    };
    absolute(path).starts_with(absolute(parent))
}

#[derive(Debug, Clone)]
pub enum ProjectMutation {
    Added,
    Removed(String),
    Pinned(String, bool),
}

#[derive(Debug, Clone)]
pub struct Loadable<T> {
    pub data: Option<T>,
    pub loading: bool,
    pub error: Option<UnityError>,
}

impl<T> Default for Loadable<T> {
    fn default() -> Self {
        Self {
            data: None,
            loading: false,
            error: None,
        }
    }
}

impl<T> Loadable<T> {
    pub fn begin(&mut self) {
        self.loading = true;
        self.error = None;
    }

    pub fn succeeded(&mut self, data: T) {
        self.data = Some(data);
        self.loading = false;
        self.error = None;
    }

    pub fn failed(&mut self, error: UnityError) {
        self.loading = false;
        self.error = Some(error);
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Whether an automatic page load should start this read.
    ///
    /// A failed read waits for the user's Retry action or an explicit Refresh.
    /// Re-running it on every navigation would turn a denied authorization
    /// into a prompt/retry storm.
    fn should_start(&self, force: bool) -> bool {
        !self.loading && (force || (self.data.is_none() && self.error.is_none()))
    }
}

/// Which read a failed `Loadable` belongs to. The id is stable across view
/// model rebuilds, so a Retry action never targets a row index or a closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadKind {
    Status,
    Update,
    Auth,
    License,
    Running,
    Editors,
    Releases,
    Projects,
    Modules,
    DialogModules,
    PreviewInstall,
    Changelog,
    RawChangelog,
    Doctor,
    Environment,
    DiagnoseProxy,
    DiagnoseAccelerator,
    DiagnoseUpdate,
    Prune,
    EditorVerify,
    CliChangelog,
    CliDryRun,
    SkillClients,
    SkillInstalls,
}

impl ReadKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Status => "read:status",
            Self::Update => "read:update",
            Self::Auth => "read:auth",
            Self::License => "read:license",
            Self::Running => "read:running",
            Self::Editors => "read:editors",
            Self::Releases => "read:releases",
            Self::Projects => "read:projects",
            Self::Modules => "read:modules",
            Self::DialogModules => "read:dialog-modules",
            Self::PreviewInstall => "read:preview-install",
            Self::Changelog => "read:changelog",
            Self::RawChangelog => "read:raw-changelog",
            Self::Doctor => "read:doctor",
            Self::Environment => "read:environment",
            Self::DiagnoseProxy => "read:diagnose-proxy",
            Self::DiagnoseAccelerator => "read:diagnose-accelerator",
            Self::DiagnoseUpdate => "read:diagnose-update",
            Self::Prune => "read:prune",
            Self::EditorVerify => "read:editor-verify",
            Self::CliChangelog => "read:cli-changelog",
            Self::CliDryRun => "read:cli-dry-run",
            Self::SkillClients => "read:skill-clients",
            Self::SkillInstalls => "read:skill-installs",
        }
    }
}

/// A read the controller can repeat after a retryable failure. Parameterised
/// reads keep the request they were started with, so Retry re-runs the exact
/// query the user saw fail instead of whatever the form contains now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryRequest {
    Read(ReadKind),
    Modules {
        version: String,
        architecture: Option<String>,
    },
    DialogModules {
        version: String,
        architecture: String,
    },
    PreviewInstall {
        version: String,
        architecture: Option<String>,
        modules: Vec<String>,
        headless: bool,
    },
    EditorVerify {
        version: String,
        architecture: String,
    },
    CliChangelog {
        target: Option<String>,
        channel: Option<String>,
    },
    CliDryRun {
        target: Option<String>,
        channel: Option<String>,
    },
}

/// Context for turning a `UnityError` into a `UiError`. Retry is only offered
/// when the controller has a matching target; job errors never get one for a
/// mutation.
#[derive(Debug, Clone)]
enum ErrorContext {
    Read(ReadKind),
    Job { kind: String },
}

/// Lower-cased project fields, computed once per projects load so filtering
/// does not allocate strings for every row on every keystroke.
#[derive(Debug, Clone)]
pub struct ProjectSearchKey {
    title: String,
    path: String,
    version: String,
}

impl ProjectSearchKey {
    fn new(project: &Project) -> Self {
        Self {
            title: project.title.to_lowercase(),
            path: project.path.to_lowercase(),
            version: project.version.to_lowercase(),
        }
    }

    fn matches(&self, needle: &str) -> bool {
        self.title.contains(needle) || self.path.contains(needle) || self.version.contains(needle)
    }
}

fn project_search_keys(projects: &[Project]) -> Vec<ProjectSearchKey> {
    projects.iter().map(ProjectSearchKey::new).collect()
}

fn project_matches_fallback(project: &Project, needle: &str) -> bool {
    project.title.to_lowercase().contains(needle)
        || project.path.to_lowercase().contains(needle)
        || project.version.to_lowercase().contains(needle)
}

/// The CLI's resident sign-in broker serves one session to every command. On
/// Linux that is what keeps a locked Secret Service collection from showing an
/// unlock prompt once per command. `UNITY_NO_AUTH_BROKER` turns it off, in
/// which case there is nothing for the app to start.
fn needs_auth_broker() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("UNITY_NO_AUTH_BROKER").is_none()
}

fn broker_error(error: &UnityError) -> UnityError {
    let mut adapted = error.clone();
    adapted.message = format!(
        "The Unity CLI could not start its sign-in service, so each command may ask to unlock the system keyring. {}",
        error.message
    );
    adapted
}

#[cfg(target_os = "linux")]
fn broker_env_message() -> Option<&'static str> {
    std::env::var_os("UNITY_NO_AUTH_BROKER").map(|_| {
        "UNITY_NO_AUTH_BROKER is set, so every Unity CLI command reads the system keyring directly and may ask to unlock it repeatedly. Unset it to let the CLI share one sign-in session."
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub kind: NoticeKind,
    pub text: String,
}

/// A clipboard writer that returns a real result. The controller owns one so a
/// test can substitute a failing backend without a display server; the
/// production implementation is the OS clipboard through `arboard`.
trait ClipboardBackend {
    fn write(&self, text: &str) -> Result<(), String>;
}

struct OperatingSystemClipboard {
    /// The clipboard owner must outlive the write. On X11, dropping it destroys
    /// the selection window after only a best-effort handover to a clipboard
    /// manager; a session without one would show a success notice and paste
    /// nothing. Lazily create one and reuse it for the app's lifetime.
    clipboard: RefCell<Option<arboard::Clipboard>>,
}

impl OperatingSystemClipboard {
    fn new() -> Self {
        Self {
            clipboard: RefCell::new(None),
        }
    }
}

impl ClipboardBackend for OperatingSystemClipboard {
    fn write(&self, text: &str) -> Result<(), String> {
        let mut slot = self.clipboard.borrow_mut();
        if slot.is_none() {
            *slot = Some(
                arboard::Clipboard::new()
                    .map_err(|error| format!("clipboard unavailable: {error}"))?,
            );
        }
        slot.as_mut()
            .expect("the clipboard was just initialized")
            .set_text(text.to_owned())
            .map_err(|error| format!("clipboard write failed: {error}"))
    }
}

impl Notice {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            kind: NoticeKind::Info,
            text: text.into(),
        }
    }

    pub fn success(text: impl Into<String>) -> Self {
        Self {
            kind: NoticeKind::Success,
            text: text.into(),
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            kind: NoticeKind::Error,
            text: text.into(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn broker_env_message() -> Option<&'static str> {
    None
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub stream: StreamKind,
    pub text: String,
}

/// The log scrollback is bounded so a chatty install cannot grow the view model
/// or the shaped text without limit.
const MAX_LOG_LINES: usize = 300;
const MAX_LOG_CHARS: usize = 1000;

#[derive(Debug, Clone)]
pub struct ActiveJob {
    pub id: String,
    pub kind: String,
    pub command: String,
    pub state: JobState,
    pub progress: f32,
    /// Last percentage seen for each file in a parallel install, keyed by the
    /// CLI's `sha256` (or its fallback identity). Empty for other jobs.
    pub downloads: BTreeMap<String, f32>,
    pub message: Option<String>,
    pub logs: VecDeque<LogLine>,
    /// The rendered form of `logs`, rebuilt only when the buffer changes so the
    /// view can hand iced a stable string.
    pub log_text: Arc<str>,
    pub error: Option<UnityError>,
    /// The terminal NDJSON `result` frame. Streaming writes read created paths
    /// and `steps[]` from it after the job ends, a download can report where
    /// its files landed, and a failed install can prove a cache is resumable.
    pub result: Option<Value>,
}

impl ActiveJob {
    fn new(started: JobStarted) -> Self {
        Self {
            id: started.id,
            kind: started.kind,
            command: started.command,
            state: JobState::Running,
            progress: 0.0,
            downloads: BTreeMap::new(),
            message: None,
            logs: VecDeque::new(),
            log_text: Arc::from(""),
            error: None,
            result: None,
        }
    }

    fn push_log(&mut self, stream: StreamKind, text: String) {
        self.logs.push_back(LogLine { stream, text });

        if self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
        }

        // Rendering is a single pass over a bounded buffer (300 lines), and it
        // happens only when a line arrives, never per frame.
        let mut rendered = String::with_capacity(self.log_text.len() + 64);

        for line in &self.logs {
            push_log_line(&mut rendered, line);
        }

        self.log_text = Arc::from(rendered);
    }

    fn is_running(&self) -> bool {
        self.state == JobState::Running
    }
}

fn push_log_line(buffer: &mut String, line: &LogLine) {
    if line.stream == StreamKind::Stderr {
        buffer.push_str("! ");
    }
    buffer.push_str(&line.text);
    buffer.push('\n');
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    pub version: String,
    pub architecture: Option<String>,
    pub modules: Vec<String>,
    pub headless: bool,
    pub accept_eula: bool,
    pub options: InstallOptions,
}

/// A resume action remembered from the last interrupted install.
#[derive(Debug, Clone)]
pub struct ResumeOffer {
    pub request: InstallRequest,
    pub reason: String,
}

/// What one skill install/refresh job was asked to do.
///
/// Kept after the job so a `conflict` row can offer `--force` without the user
/// having to reconstruct the form. The scope is the exact request, not a row
/// index: a refresh between the click and the force action cannot retarget it.
#[derive(Debug, Clone)]
pub struct SkillRun {
    pub scope: SkillScope,
    pub dry_run: bool,
    pub force: bool,
    /// The project context `--local` was checked against at start time.
    pub project_context: Option<String>,
}

impl SkillRun {
    fn scope_local(&self) -> bool {
        self.scope.local()
    }
}

#[derive(Debug, Clone)]
pub enum SkillScope {
    Install {
        client: Option<String>,
        all: bool,
        local: bool,
        skill: Option<String>,
        yes: bool,
    },
    Refresh {
        yes: bool,
    },
}

impl SkillScope {
    /// True when the run carried `--local`.
    fn local(&self) -> bool {
        matches!(self, Self::Install { local: true, .. })
    }
}

/// One finished run's terminal rows, before they become the view model.
#[derive(Debug, Clone)]
pub struct SkillResult {
    pub rows: Vec<SkillRefreshRow>,
    pub dry_run: bool,
}

/// Session cache key for a docs lookup: topic, loaded changelog version, and
/// the resolved scope. The package name is part of `DocsScope::Package`, so
/// two package lookups never share an answer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DocsKey {
    topic: String,
    version: String,
    scope: DocsScope,
}

pub struct App {
    pub cli: UnityCli,
    pub jobs: JobManager,
    clipboard: Box<dyn ClipboardBackend>,
    pub page: Page,
    pub cli_status: Loadable<CliStatus>,
    pub update: Loadable<CliUpdateInfo>,
    pub update_channel: String,
    /// Set once the user picks a channel. A later status read must not move
    /// the update flow to the channel the CLI happens to track.
    pub update_channel_touched: bool,
    pub update_target: String,
    pub update_changelog: Loadable<unity_core::models::RawChangelog>,
    pub update_dry_run: Loadable<String>,
    pub rollback_target: String,
    pub rollback_confirmed: bool,
    pub rollback_error: Option<UnityError>,
    pub auth: Loadable<AuthStatus>,
    pub license: Loadable<LicenseStatus>,
    pub running: Loadable<RunningEditors>,
    pub editors: Loadable<Vec<Editor>>,
    /// The last report-only `unity editors prune` scan.
    pub prune: Loadable<PruneReport>,
    pub prune_confirmed: bool,
    pub prune_missing_confirmed: bool,
    pub prune_force: bool,
    /// Set once a remove run fails because the running-Editor check could not
    /// complete. `--force` is offered only after that.
    pub prune_force_available: bool,
    pub prune_error: Option<UnityError>,
    /// The build the verify dialog is showing.
    pub verify_version: String,
    pub verify_architecture: String,
    pub verify: Loadable<EditorVerifyRead>,
    pub verify_show_raw: bool,
    pub releases: Loadable<Vec<Release>>,
    pub projects: Loadable<Vec<Project>>,
    pub templates: Loadable<Vec<TemplateBrief>>,
    pub template_detail: Loadable<TemplateInfo>,
    pub template_editor: Option<String>,
    pub template_filter: TemplateKindFilter,
    pub template_installed_only: bool,
    pub template_selected: Option<String>,
    pub processes: Loadable<RunningEditors>,
    pub process_sampler: Sampler,
    pub process_metrics: HashMap<u32, ProcessMetrics>,
    pub process_last_refreshed: Option<String>,
    pub changelog: Loadable<ChangelogView>,
    pub changelog_loaded: bool,
    pub changelog_target: Option<String>,
    pub show_raw_changelog: bool,
    pub raw_changelog: Loadable<String>,
    /// Off by default; the checkbox label counts the entries it keeps.
    pub changelog_app_impact_only: bool,
    /// The advanced docs collection. `Automatic` follows each entry's kind.
    pub changelog_docs_scope: DocsScopeVm,
    pub changelog_docs_package: String,
    /// Session cache for resolved docs URLs, keyed by topic + version + scope.
    docs_cache: HashMap<DocsKey, DocsPage>,
    /// Failed lookups stay beside the entry as a quiet inline message. A miss
    /// (`DOCS_NOT_FOUND`) is kept separately through its code.
    docs_errors: HashMap<DocsKey, UnityError>,
    docs_loading: HashSet<DocsKey>,
    pub doctor: Loadable<DoctorRead>,
    pub environment: Loadable<EnvironmentInfo>,
    pub diagnose_proxy: Loadable<String>,
    pub diagnose_accelerator: Loadable<String>,
    pub diagnose_update: Loadable<String>,
    /// Supported AI clients from `unity skill install --list`.
    pub skill_clients: Loadable<Vec<SkillClientInfo>>,
    /// Tracked skill installs from `unity skills`.
    pub skill_installs: Loadable<Vec<TrackedSkillInstall>>,
    /// `--local`: write into the current project.
    pub skill_install_local: bool,
    /// `--yes`: replace an outdated copy instead of letting the CLI skip it.
    pub skill_install_overwrite: bool,
    /// `--dry-run` for an install.
    pub skill_install_dry_run: bool,
    /// The optional `--skill <name>` filter.
    pub skill_install_skill: String,
    /// False until a dry-run refresh has landed. While false, the first
    /// refresh click is a dry run; after a preview, clicks apply.
    pub skill_refresh_previewed: bool,
    /// The last run's terminal rows, kept verbatim.
    pub skill_result: Option<SkillResult>,
    /// The exact request the last skill job started with, for `--force`.
    pub skill_last_run: Option<SkillRun>,
    /// The Unity project `--local` may write into, when the app was launched
    /// from one. `None` disables the option with an explanation.
    pub project_context: Option<String>,
    pub modules: Loadable<Vec<Module>>,
    pub dialog_modules: Loadable<Vec<Module>>,
    pub modal: Modal,
    pub cli_channel: String,
    pub cli_consent: bool,
    pub install_version: String,
    /// `None` keeps the host default and omits `--architecture`.
    pub install_architecture: Option<String>,
    pub selected_modules: BTreeSet<String>,
    pub install_headless: bool,
    pub accept_eula: bool,
    pub install_advanced_expanded: bool,
    pub install_resume: bool,
    pub install_download_only: bool,
    pub install_no_wait: bool,
    pub install_timeout: String,
    pub install_set_default: bool,
    pub install_retries: String,
    pub install_advanced_note: Option<String>,
    /// The request the last editor-install job started with, so Resume reuses
    /// exactly the same build, modules and cache.
    pub last_install: Option<InstallRequest>,
    pub resume_offer: Option<ResumeOffer>,
    pub download_report: Option<String>,
    pub dialog_retries: String,
    pub dialog_download_directory: String,
    pub preview: Option<InstallPreview>,
    pub preview_raw: Option<Value>,
    pub preview_error: Option<UiError>,
    pub preview_loading: bool,
    pub dialog_selected: BTreeSet<String>,
    pub dialog_eula: bool,
    pub project_search: String,
    pub project_search_index: Vec<ProjectSearchKey>,
    pub upgrade_target: Option<String>,
    pub upgrade_confirmed: bool,
    pub app_settings: AppSettings,
    pub cli_path_input: String,
    pub config_dir: Option<String>,
    pub active_job: Option<ActiveJob>,
    pub notifications: NotificationStore,
    /// The notification whose remediation command is currently running.
    pub remediating: Option<u64>,
    pub notice: Option<Notice>,
    /// Retry targets for failed reads, keyed by the stable id the view model
    /// carries in `ErrorActionKind::Retry`.
    pub retry_targets: std::collections::HashMap<String, RetryRequest>,
    pub boot_background_started: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    // UI intents. These are constructed from `UiAction` so the visual layer
    // never needs to know about the controller's async message variants.
    Navigate(Page),
    Refresh,
    DismissJob,
    CancelJob,

    // Notification centre
    ToggleNotificationRail,
    DismissNotification(u64),
    ClearNotifications,
    DismissToast(u64),
    CopyNotificationCommand(u64),
    RunNotificationCommand(u64),
    ConfirmNotificationCommand(u64),
    CancelNotificationCommand(u64),
    ToastExpired(u64),

    DismissNotice,
    RunErrorAction(ErrorActionVm),
    CopyDiagnosticsReport,
    LoadDoctor,
    CopyDiagnoseReport(DiagnoseTopic),
    SignIn,
    TextSizeChanged(TextSize),
    FontFamilyChanged(String),
    OpenInstallCli,
    CliChannelChanged(String),
    CliConsentToggled(bool),
    InstallCli,
    UpdateCli,
    CliUpdateChannelChanged(String),
    CliUpdateTargetChanged(String),
    LoadCliUpdateChangelog,
    PreviewCliUpdate,
    StartCliUpdate,
    OpenCliRollback,
    CliRollbackVersionChanged(String),
    CliRollbackConfirmed(bool),
    StartCliRollback,
    InstallVersionChanged(String),
    InstallArchitectureChanged(Option<String>),
    LoadModules,
    ToggleModule(String, bool),
    SelectAllModules(bool),
    HeadlessToggled(bool),
    AcceptEulaToggled(bool),
    PreviewInstall,
    StartInstallEditor,
    InstallAdvancedToggled(bool),
    InstallResumeToggled(bool),
    InstallDownloadOnlyToggled(bool),
    InstallNoWaitToggled(bool),
    InstallTimeoutChanged(String),
    InstallSetDefaultToggled(bool),
    InstallRetriesChanged(String),
    ResumeInstallEditor,
    DialogRetriesChanged(String),
    DialogDownloadDirectoryChanged(String),
    UseRelease(String, String),
    OpenModules(String, String),
    SetDefaultEditor(String, String),
    ClearDefaultEditor,
    OpenPrune,
    PruneScan,
    PruneConfirmed(bool),
    PruneMissingConfirmed(bool),
    PruneForceToggled(bool),
    StartPruneRemove,
    StartPruneRemoveMissing,
    OpenVerify(String, String),
    ToggleVerifyRaw,
    CloseModal,
    ToggleDialogModule(String, bool),
    DialogEulaToggled(bool),
    StartInstallModules,
    ProjectSearchChanged(String),
    AddProject,
    ProjectFolderPicked(Option<PathBuf>),
    OpenProjectRemove(String),
    ConfirmProjectRemove,
    ToggleProjectFavorite(String, bool),
    OpenProject(String),
    OpenUpgrade(String),
    UpgradeTargetChanged(String),
    UpgradeConfirmed(bool),
    StartUpgrade,
    InstallSkill(String),
    InstallAllSkills,
    SkillLocalToggled(bool),
    SkillOverwriteToggled(bool),
    SkillDryRunToggled(bool),
    SkillNameChanged(String),
    StartSkillRefresh,
    PreviewSkillRefresh,
    ForceSkillLastRun,
    ChangelogTargetChanged(String),
    LoadChangelog,
    ToggleRawChangelog,
    ToggleChangelogImpact(bool),
    ChangelogDocsScopeChanged(DocsScopeVm),
    ChangelogDocsPackageChanged(String),
    LookupChangelogDocs {
        topic: String,
        scope: DocsScopeVm,
    },
    OpenExternalUrl(String),
    CliPathChanged(String),
    SaveCliPath,
    ClearCliPath,

    // Templates
    TemplatesEditorChanged(String),
    TemplateFilterChanged(TemplateKindFilter),
    TemplateInstalledOnlyToggled(bool),
    OpenTemplateDetail(String),
    OpenTemplateAuthoring(TemplateAuthoringMode),
    TemplateAuthoringProjectChanged(String),
    TemplateFieldChanged(TemplateField, String),
    TemplateOverwriteToggled(bool),
    StartTemplateAuthoring,

    // New Project
    OpenNewProject,
    NewProjectFieldChanged(NewProjectField, String),
    NewProjectBrowseParent,
    NewProjectFolderPicked(Option<PathBuf>),
    NewProjectEditorChanged(String),
    NewProjectArchitectureChanged(String),
    NewProjectTemplateChanged(String),
    NewProjectBuiltinToggled(bool),
    NewProjectOpenToggled(bool),
    NewProjectNext,
    NewProjectBack,
    StartNewProject,

    // Processes
    RefreshProcesses,
    ProcessMetricsToggled(bool),
    CopyText(String),
    OpenFolder(String),

    // Async results and job lifecycle stay controller-private.
    StatusLoaded(Result<CliStatus, UnityError>),
    UpdateLoaded(Result<CliUpdateInfo, UnityError>),
    AuthLoaded(Result<AuthStatus, UnityError>),
    LicenseLoaded(Result<LicenseStatus, UnityError>),
    RunningLoaded(Result<RunningEditors, UnityError>),
    EditorsLoaded(Result<Vec<Editor>, UnityError>),
    ReleasesLoaded(Result<Vec<Release>, UnityError>),
    ProjectsLoaded(Result<Vec<Project>, UnityError>),
    TemplatesLoaded(Result<Vec<TemplateBrief>, UnityError>),
    TemplateInfoLoaded(Result<TemplateInfo, UnityError>),
    ProcessesLoaded(Result<RunningEditors, UnityError>),
    NewProjectLicenseLoaded(Result<LicenseStatus, UnityError>),
    NewProjectEditorsLoaded(Result<Vec<Editor>, UnityError>),
    NewProjectTemplatesLoaded(Result<Vec<TemplateBrief>, UnityError>),
    NewProjectTemplateInfoLoaded(Result<TemplateInfo, UnityError>),
    ProcessTick,
    ModulesLoaded(Result<Vec<Module>, UnityError>),
    DialogModulesLoaded(Result<Vec<Module>, UnityError>),
    PreviewLoaded(Result<Value, UnityError>),
    ChangelogLoaded(Result<ChangelogView, UnityError>),
    DocsLoaded {
        topic: String,
        version: String,
        scope: DocsScope,
        result: Result<DocsPage, UnityError>,
    },
    RawChangelogLoaded(Result<String, UnityError>),
    CliChangelogLoaded(Result<unity_core::models::RawChangelog, UnityError>),
    CliDryRunLoaded(Result<Value, UnityError>),
    DoctorLoaded(Result<DoctorRead, UnityError>),
    EnvironmentLoaded(Result<EnvironmentInfo, UnityError>),
    DiagnoseProxyLoaded(Result<String, UnityError>),
    DiagnoseAcceleratorLoaded(Result<String, UnityError>),
    DiagnoseUpdateLoaded(Result<String, UnityError>),
    SkillClientsLoaded(Result<Vec<SkillClientInfo>, UnityError>),
    SkillInstallsLoaded(Result<Vec<TrackedSkillInstall>, UnityError>),
    PruneLoaded(Result<PruneReport, UnityError>),
    VerifyLoaded(Result<EditorVerifyRead, UnityError>),
    PollJobs,
    ProjectOpened(Result<(), UnityError>),
    /// The Linux sign-in broker was started (or failed to start) before the
    /// first credential-reading batch.
    AuthBrokerEnsured(Result<(), UnityError>),
    ProjectMutationDone(ProjectMutation, Result<(), UnityError>),
}

impl From<UiAction> for Message {
    fn from(action: UiAction) -> Self {
        match action {
            UiAction::Navigate(page) => Self::Navigate(page),
            UiAction::Refresh => Self::Refresh,
            UiAction::RunErrorAction(action) => Self::RunErrorAction(action),
            UiAction::CopyDiagnosticsReport => Self::CopyDiagnosticsReport,
            UiAction::LoadDoctor => Self::LoadDoctor,
            UiAction::CopyDiagnoseReport(topic) => Self::CopyDiagnoseReport(topic),
            UiAction::DismissJob => Self::DismissJob,
            UiAction::CancelJob => Self::CancelJob,
            UiAction::ToggleNotificationRail => Self::ToggleNotificationRail,
            UiAction::DismissNotification(id) => Self::DismissNotification(id),
            UiAction::ClearNotifications => Self::ClearNotifications,
            UiAction::DismissToast(id) => Self::DismissToast(id),
            UiAction::CopyNotificationCommand(id) => Self::CopyNotificationCommand(id),
            UiAction::RunNotificationCommand(id) => Self::RunNotificationCommand(id),
            UiAction::ConfirmNotificationCommand(id) => Self::ConfirmNotificationCommand(id),
            UiAction::CancelNotificationCommand(id) => Self::CancelNotificationCommand(id),
            UiAction::SignIn => Self::SignIn,
            UiAction::TextScaleChanged(percent) => {
                Self::TextSizeChanged(TextSize::from_percent(percent))
            }
            UiAction::FontFamilyChanged(family) => Self::FontFamilyChanged(family),
            UiAction::OpenInstallCli => Self::OpenInstallCli,
            UiAction::CliChannelChanged(channel) => Self::CliChannelChanged(channel),
            UiAction::CliConsentToggled(value) => Self::CliConsentToggled(value),
            UiAction::InstallCli => Self::InstallCli,
            UiAction::UpdateCli => Self::UpdateCli,
            UiAction::CliUpdateChannelChanged(channel) => Self::CliUpdateChannelChanged(channel),
            UiAction::CliUpdateTargetChanged(target) => Self::CliUpdateTargetChanged(target),
            UiAction::LoadCliUpdateChangelog => Self::LoadCliUpdateChangelog,
            UiAction::PreviewCliUpdate => Self::PreviewCliUpdate,
            UiAction::StartCliUpdate => Self::StartCliUpdate,
            UiAction::OpenCliRollback => Self::OpenCliRollback,
            UiAction::CliRollbackVersionChanged(version) => {
                Self::CliRollbackVersionChanged(version)
            }
            UiAction::CliRollbackConfirmed(value) => Self::CliRollbackConfirmed(value),
            UiAction::StartCliRollback => Self::StartCliRollback,
            UiAction::InstallVersionChanged(version) => Self::InstallVersionChanged(version),
            UiAction::InstallArchitectureChanged(architecture) => {
                Self::InstallArchitectureChanged(architecture)
            }
            UiAction::LoadModules => Self::LoadModules,
            UiAction::ToggleModule(id, value) => Self::ToggleModule(id, value),
            UiAction::SelectAllModules(value) => Self::SelectAllModules(value),
            UiAction::HeadlessToggled(value) => Self::HeadlessToggled(value),
            UiAction::AcceptEulaToggled(value) => Self::AcceptEulaToggled(value),
            UiAction::PreviewInstall => Self::PreviewInstall,
            UiAction::StartInstallEditor => Self::StartInstallEditor,
            UiAction::InstallAdvancedToggled(value) => Self::InstallAdvancedToggled(value),
            UiAction::InstallResumeToggled(value) => Self::InstallResumeToggled(value),
            UiAction::InstallDownloadOnlyToggled(value) => Self::InstallDownloadOnlyToggled(value),
            UiAction::InstallNoWaitToggled(value) => Self::InstallNoWaitToggled(value),
            UiAction::InstallTimeoutChanged(value) => Self::InstallTimeoutChanged(value),
            UiAction::InstallSetDefaultToggled(value) => Self::InstallSetDefaultToggled(value),
            UiAction::InstallRetriesChanged(value) => Self::InstallRetriesChanged(value),
            UiAction::ResumeInstallEditor => Self::ResumeInstallEditor,
            UiAction::DialogRetriesChanged(value) => Self::DialogRetriesChanged(value),
            UiAction::DialogDownloadDirectoryChanged(value) => {
                Self::DialogDownloadDirectoryChanged(value)
            }
            UiAction::UseRelease(version, architecture) => Self::UseRelease(version, architecture),
            UiAction::OpenEditorModules(version, architecture) => {
                Self::OpenModules(version, architecture)
            }
            UiAction::SetDefaultEditor(version, architecture) => {
                Self::SetDefaultEditor(version, architecture)
            }
            UiAction::ClearDefaultEditor => Self::ClearDefaultEditor,
            UiAction::OpenPrune => Self::OpenPrune,
            UiAction::PruneScan => Self::PruneScan,
            UiAction::PruneConfirmed(value) => Self::PruneConfirmed(value),
            UiAction::PruneMissingConfirmed(value) => Self::PruneMissingConfirmed(value),
            UiAction::PruneForceToggled(value) => Self::PruneForceToggled(value),
            UiAction::StartPruneRemove => Self::StartPruneRemove,
            UiAction::StartPruneRemoveMissing => Self::StartPruneRemoveMissing,
            UiAction::OpenVerify(version, architecture) => Self::OpenVerify(version, architecture),
            UiAction::ToggleVerifyRaw => Self::ToggleVerifyRaw,
            UiAction::CloseModal => Self::CloseModal,
            UiAction::ToggleDialogModule(id, value) => Self::ToggleDialogModule(id, value),
            UiAction::DialogEulaToggled(value) => Self::DialogEulaToggled(value),
            UiAction::StartInstallModules => Self::StartInstallModules,
            UiAction::ProjectSearchChanged(value) => Self::ProjectSearchChanged(value),
            UiAction::AddProject => Self::AddProject,
            UiAction::OpenProjectRemove(path) => Self::OpenProjectRemove(path),
            UiAction::ConfirmProjectRemove => Self::ConfirmProjectRemove,
            UiAction::ToggleProjectFavorite(path, favorite) => {
                Self::ToggleProjectFavorite(path, favorite)
            }
            UiAction::OpenProject(path) => Self::OpenProject(path),
            UiAction::OpenProjectUpgrade(path) => Self::OpenUpgrade(path),
            UiAction::UpgradeTargetChanged(version) => Self::UpgradeTargetChanged(version),
            UiAction::UpgradeConfirmed(value) => Self::UpgradeConfirmed(value),
            UiAction::StartUpgrade => Self::StartUpgrade,
            UiAction::InstallSkill(client) => Self::InstallSkill(client),
            UiAction::InstallAllSkills => Self::InstallAllSkills,
            UiAction::SkillLocalToggled(value) => Self::SkillLocalToggled(value),
            UiAction::SkillOverwriteToggled(value) => Self::SkillOverwriteToggled(value),
            UiAction::SkillDryRunToggled(value) => Self::SkillDryRunToggled(value),
            UiAction::SkillNameChanged(value) => Self::SkillNameChanged(value),
            UiAction::StartSkillRefresh => Self::StartSkillRefresh,
            UiAction::PreviewSkillRefresh => Self::PreviewSkillRefresh,
            UiAction::ForceSkillLastRun => Self::ForceSkillLastRun,
            UiAction::ChangelogTargetChanged(target) => Self::ChangelogTargetChanged(target),
            UiAction::LoadChangelog => Self::LoadChangelog,
            UiAction::ToggleRawChangelog => Self::ToggleRawChangelog,
            UiAction::ToggleChangelogImpact(value) => Self::ToggleChangelogImpact(value),
            UiAction::ChangelogDocsScopeChanged(scope) => Self::ChangelogDocsScopeChanged(scope),
            UiAction::ChangelogDocsPackageChanged(package) => {
                Self::ChangelogDocsPackageChanged(package)
            }
            UiAction::LookupChangelogDocs { topic, scope } => {
                Self::LookupChangelogDocs { topic, scope }
            }
            UiAction::OpenExternalUrl(url) => Self::OpenExternalUrl(url),
            UiAction::CliPathChanged(value) => Self::CliPathChanged(value),
            UiAction::SaveCliPath => Self::SaveCliPath,
            UiAction::ClearCliPath => Self::ClearCliPath,
            UiAction::TemplatesEditorChanged(version) => Self::TemplatesEditorChanged(version),
            UiAction::TemplateFilterChanged(filter) => Self::TemplateFilterChanged(filter),
            UiAction::TemplateInstalledOnlyToggled(value) => {
                Self::TemplateInstalledOnlyToggled(value)
            }
            UiAction::OpenTemplateDetail(name) => Self::OpenTemplateDetail(name),
            UiAction::OpenTemplateAuthoring(mode) => Self::OpenTemplateAuthoring(mode),
            UiAction::TemplateAuthoringProjectChanged(path) => {
                Self::TemplateAuthoringProjectChanged(path)
            }
            UiAction::TemplateFieldChanged(field, value) => {
                Self::TemplateFieldChanged(field, value)
            }
            UiAction::TemplateOverwriteToggled(value) => Self::TemplateOverwriteToggled(value),
            UiAction::StartTemplateAuthoring => Self::StartTemplateAuthoring,
            UiAction::OpenNewProject => Self::OpenNewProject,
            UiAction::NewProjectFieldChanged(field, value) => {
                Self::NewProjectFieldChanged(field, value)
            }
            UiAction::NewProjectBrowseParent => Self::NewProjectBrowseParent,
            UiAction::NewProjectEditorChanged(version) => Self::NewProjectEditorChanged(version),
            UiAction::NewProjectArchitectureChanged(arch) => {
                Self::NewProjectArchitectureChanged(arch)
            }
            UiAction::NewProjectTemplateChanged(name) => Self::NewProjectTemplateChanged(name),
            UiAction::NewProjectBuiltinToggled(value) => Self::NewProjectBuiltinToggled(value),
            UiAction::NewProjectOpenToggled(value) => Self::NewProjectOpenToggled(value),
            UiAction::NewProjectNext => Self::NewProjectNext,
            UiAction::NewProjectBack => Self::NewProjectBack,
            UiAction::StartNewProject => Self::StartNewProject,
            UiAction::RefreshProcesses => Self::RefreshProcesses,
            UiAction::ProcessMetricsToggled(value) => Self::ProcessMetricsToggled(value),
            UiAction::CopyText(text) => Self::CopyText(text),
            UiAction::OpenFolder(path) => Self::OpenFolder(path),
        }
    }
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let app_settings = settings::load();
        let cli = UnityCli::new();
        if let Some(path) = &app_settings.cli_path {
            let path = PathBuf::from(path);
            if path.is_file() {
                cli.set_override_path(Some(path));
            }
        }

        let mut app = Self {
            cli: cli.clone(),
            jobs: JobManager::with_sink(cli.advisory_sink()),
            clipboard: Box::new(OperatingSystemClipboard::new()),
            page: Page::Overview,
            cli_status: Loadable::default(),
            update: Loadable::default(),
            update_channel: "stable".to_string(),
            update_channel_touched: false,
            update_target: String::new(),
            update_changelog: Loadable::default(),
            update_dry_run: Loadable::default(),
            rollback_target: String::new(),
            rollback_confirmed: false,
            rollback_error: None,
            auth: Loadable::default(),
            license: Loadable::default(),
            running: Loadable::default(),
            editors: Loadable::default(),
            prune: Loadable::default(),
            prune_confirmed: false,
            prune_missing_confirmed: false,
            prune_force: false,
            prune_force_available: false,
            prune_error: None,
            verify_version: String::new(),
            verify_architecture: String::new(),
            verify: Loadable::default(),
            verify_show_raw: false,
            releases: Loadable::default(),
            projects: Loadable::default(),
            templates: Loadable::default(),
            template_detail: Loadable::default(),
            template_editor: None,
            template_filter: TemplateKindFilter::All,
            template_installed_only: false,
            template_selected: None,
            processes: Loadable::default(),
            process_sampler: Sampler::new(),
            process_metrics: HashMap::new(),
            process_last_refreshed: None,
            changelog: Loadable::default(),
            changelog_loaded: false,
            changelog_target: None,
            show_raw_changelog: false,
            raw_changelog: Loadable::default(),
            changelog_app_impact_only: false,
            changelog_docs_scope: DocsScopeVm::Automatic,
            changelog_docs_package: String::new(),
            docs_cache: HashMap::new(),
            docs_errors: HashMap::new(),
            docs_loading: HashSet::new(),
            doctor: Loadable::default(),
            environment: Loadable::default(),
            diagnose_proxy: Loadable::default(),
            diagnose_accelerator: Loadable::default(),
            diagnose_update: Loadable::default(),
            skill_clients: Loadable::default(),
            skill_installs: Loadable::default(),
            skill_install_local: false,
            skill_install_overwrite: false,
            skill_install_dry_run: false,
            skill_install_skill: String::new(),
            skill_refresh_previewed: false,
            skill_result: None,
            skill_last_run: None,
            project_context: project_context(),
            modules: Loadable::default(),
            dialog_modules: Loadable::default(),
            modal: Modal::None,
            cli_channel: "stable".to_string(),
            cli_consent: false,
            install_version: "lts".to_string(),
            install_architecture: None,
            selected_modules: BTreeSet::new(),
            install_headless: false,
            accept_eula: false,
            install_advanced_expanded: false,
            install_resume: false,
            install_download_only: false,
            install_no_wait: false,
            install_timeout: String::new(),
            install_set_default: false,
            install_retries: String::new(),
            install_advanced_note: None,
            last_install: None,
            resume_offer: None,
            download_report: None,
            dialog_retries: String::new(),
            dialog_download_directory: String::new(),
            preview: None,
            preview_raw: None,
            preview_error: None,
            preview_loading: false,
            dialog_selected: BTreeSet::new(),
            dialog_eula: false,
            project_search: String::new(),
            project_search_index: Vec::new(),
            upgrade_target: None,
            upgrade_confirmed: false,
            cli_path_input: app_settings.cli_path.clone().unwrap_or_default(),
            app_settings,
            config_dir: settings::config_dir().map(|path| path.display().to_string()),
            active_job: None,
            notifications: NotificationStore::new(),
            remediating: None,
            notice: None,
            retry_targets: std::collections::HashMap::new(),
            boot_background_started: false,
        };
        // The boot status read is the first failing read the user can retry.
        app.retry_targets.insert(
            ReadKind::Status.id().to_string(),
            RetryRequest::Read(ReadKind::Status),
        );

        if let Some(message) = broker_env_message() {
            app.notifications
                .push_event(NotificationLevel::Info, "Sign-in broker", message);
        }

        // The status read is the only critical boot work. The rest of the
        // Overview is loaded after it succeeds, one CLI process at a time.
        (app, load_status(cli))
    }

    pub fn text_scale(&self) -> f32 {
        self.app_settings.text_size().scale()
    }

    pub fn font_family(&self) -> &str {
        &self.app_settings.font_family
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // Job events only matter while a job is streaming. An always-on timer
        // keeps the app awake for no reason and prevents the process from
        // idling when nothing is running.
        let job = if self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false)
        {
            Subscription::run_with(self.jobs.clone(), |jobs| {
                let (notifier, wakeups) = std::sync::mpsc::sync_channel(1);
                jobs.set_notifier(notifier);

                let (sender, receiver) = futures::channel::mpsc::unbounded();
                std::thread::spawn(move || {
                    // Drain anything that arrived before this subscription existed.
                    if sender.unbounded_send(Message::PollJobs).is_err() {
                        return;
                    }

                    while wakeups.recv().is_ok() {
                        std::thread::sleep(Duration::from_millis(100));
                        while wakeups.try_recv().is_ok() {}

                        if sender.unbounded_send(Message::PollJobs).is_err() {
                            break;
                        }
                    }
                });
                receiver
            })
        } else {
            Subscription::none()
        };

        // The Processes page refreshes only while it is visible. Dropping the
        // subscription when the page changes closes the receiver, and the
        // producer thread exits on its next failed send.
        let processes = if self.page == Page::Processes {
            Subscription::run_with(Page::Processes, |_| {
                let (sender, receiver) = futures::channel::mpsc::unbounded();
                std::thread::spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(5));
                    if sender.unbounded_send(Message::ProcessTick).is_err() {
                        break;
                    }
                });
                receiver
            })
        } else {
            Subscription::none()
        };

        // A toast dismisses itself after a few seconds. The id keys the
        // subscription, so a newer toast replaces the pending timer.
        let toast = match self.notifications.toast() {
            Some(toast) => {
                let id = toast.id;
                Subscription::run_with(id, move |id| {
                    let id = *id;
                    let (sender, receiver) = futures::channel::mpsc::unbounded();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(5));
                        let _ = sender.unbounded_send(Message::ToastExpired(id));
                    });
                    receiver
                })
            }
            None => Subscription::none(),
        };

        Subscription::batch([job, processes, toast])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        // Every read and every polled job event funnels through `update`, so a
        // single drain here means no envelope can lose an advisory.
        self.absorb_advisories();
        task
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(page) => {
                self.page = page;
                self.ensure_page(page)
            }
            Message::Refresh => self.refresh_page(),
            Message::LoadDoctor => self.load_doctor_now(),
            Message::StatusLoaded(result) => {
                let previous_version = self.cli_version_key();
                let installed = match result {
                    Ok(status) => {
                        if let Some(version) = &status.version {
                            if version.channel == "beta" || version.channel == "alpha" {
                                self.cli_channel = version.channel.clone();
                            }
                            if (version.channel == "beta" || version.channel == "stable")
                                && !self.update_channel_touched
                                && self.update_channel != version.channel
                            {
                                // A status read can arrive after the user has
                                // already reviewed a different channel. Only a
                                // status-derived move goes through the same
                                // invalidation as a manual pick.
                                self.apply_update_channel(version.channel.clone());
                            }
                        }
                        let installed = status.installed;
                        let status_has_error = status.error.is_some();
                        self.cli_status.succeeded(status);
                        if status_has_error {
                            // `status` is Ok even when only the version probe
                            // failed, so keep its retry target.
                            self.note_read_failed(ReadKind::Status);
                        } else {
                            self.clear_retry(ReadKind::Status);
                        }
                        installed
                    }
                    Err(error) => {
                        self.cli_status.failed(error);
                        self.note_read_failed(ReadKind::Status);
                        false
                    }
                };

                self.on_cli_version_resolved(&previous_version);

                if self.boot_background_started {
                    Task::none()
                } else {
                    self.boot_background_started = true;
                    if !installed {
                        // Without a CLI there is nothing to read.
                        Task::none()
                    } else if !needs_auth_broker() {
                        self.load_overview_data(false)
                    } else {
                        // Start the sign-in broker before the Overview batch so
                        // its credential reads share one session instead of
                        // asking to unlock the keyring once per command.
                        Task::perform(
                            run_blocking({
                                let cli = self.cli.clone();
                                move || cli.ensure_auth_broker()
                            }),
                            Message::AuthBrokerEnsured,
                        )
                    }
                }
            }
            Message::AuthBrokerEnsured(result) => {
                if let Err(error) = &result {
                    if matches!(
                        error.code.as_str(),
                        cli_code::AUTH_BROKER_UNAVAILABLE
                            | cli_code::AUTH_BROKER_REFUSED
                            | cli_code::CREDENTIAL_STORE_ACCESS_DENIED
                    ) {
                        self.notifications.push_error(&broker_error(error));
                    }
                }
                // A missing broker only means each read may prompt again; the
                // Overview is still worth loading.
                self.load_overview_data(false)
            }
            Message::UpdateLoaded(result) => {
                match result {
                    Ok(info) => {
                        self.update.succeeded(info);
                        self.clear_retry(ReadKind::Update);
                    }
                    Err(error) => {
                        self.update.failed(error);
                        self.note_read_failed(ReadKind::Update);
                    }
                }
                Task::none()
            }
            Message::AuthLoaded(result) => {
                match result {
                    Ok(status) => {
                        self.auth.succeeded(status);
                        self.clear_retry(ReadKind::Auth);
                    }
                    Err(error) => {
                        self.auth.failed(error);
                        self.note_read_failed(ReadKind::Auth);
                    }
                }
                Task::none()
            }
            Message::LicenseLoaded(result) => {
                match result {
                    Ok(status) => {
                        self.license.succeeded(status);
                        self.clear_retry(ReadKind::License);
                    }
                    Err(error) => {
                        self.license.failed(error);
                        self.note_read_failed(ReadKind::License);
                    }
                }
                Task::none()
            }
            Message::RunningLoaded(result) => {
                match result {
                    Ok(status) => {
                        self.running.succeeded(status);
                        self.clear_retry(ReadKind::Running);
                    }
                    Err(error) => {
                        self.running.failed(error);
                        self.note_read_failed(ReadKind::Running);
                    }
                }
                Task::none()
            }
            Message::EditorsLoaded(result) => {
                let mut task = Task::none();
                match result {
                    Ok(editors) => {
                        self.editors.succeeded(editors);
                        self.clear_retry(ReadKind::Editors);
                        self.ensure_upgrade_target();
                        if self.page == Page::Templates
                            && self.template_editor.is_none()
                            && self.templates.data.is_none()
                        {
                            task = self.load_templates(false);
                        }
                    }
                    Err(error) => {
                        self.editors.failed(error);
                        self.note_read_failed(ReadKind::Editors);
                    }
                }
                task
            }
            Message::ReleasesLoaded(result) => {
                match result {
                    Ok(releases) => {
                        self.releases.succeeded(releases);
                        self.clear_retry(ReadKind::Releases);
                    }
                    Err(error) => {
                        self.releases.failed(error);
                        self.note_read_failed(ReadKind::Releases);
                    }
                }
                Task::none()
            }
            Message::ProjectsLoaded(result) => {
                match result {
                    Ok(projects) => {
                        self.project_search_index = project_search_keys(&projects);
                        self.projects.succeeded(projects);
                        if let Modal::TemplateAuthoring(state) = &mut self.modal {
                            if state.source.trim().is_empty() {
                                if let Some(project) = self
                                    .projects
                                    .data
                                    .as_ref()
                                    .and_then(|projects| projects.first())
                                {
                                    state.source = project.path.clone();
                                }
                            }
                        }
                        self.clear_retry(ReadKind::Projects);
                    }
                    Err(error) => {
                        self.projects.failed(error);
                        self.note_read_failed(ReadKind::Projects);
                    }
                }
                Task::none()
            }
            Message::ModulesLoaded(result) => {
                match result {
                    Ok(modules) => {
                        self.modules.succeeded(modules);
                        self.clear_retry(ReadKind::Modules);
                    }
                    Err(error) => {
                        self.modules.failed(error);
                        self.note_read_failed(ReadKind::Modules);
                    }
                }
                Task::none()
            }
            Message::DialogModulesLoaded(result) => {
                match result {
                    Ok(modules) => {
                        self.dialog_modules.succeeded(modules);
                        self.clear_retry(ReadKind::DialogModules);
                    }
                    Err(error) => {
                        self.dialog_modules.failed(error);
                        self.note_read_failed(ReadKind::DialogModules);
                    }
                }
                Task::none()
            }
            Message::PreviewLoaded(result) => {
                self.preview_loading = false;
                match result {
                    Ok(value) => {
                        self.preview_error = None;
                        self.clear_retry(ReadKind::PreviewInstall);
                        match serde_json::from_value::<InstallPreview>(value.clone()) {
                            Ok(preview) => {
                                self.preview = Some(preview);
                                self.preview_raw = None;
                            }
                            Err(_) => {
                                self.preview = None;
                                self.preview_raw = Some(value);
                            }
                        }
                    }
                    Err(error) => {
                        self.preview = None;
                        self.preview_raw = None;
                        self.note_read_failed(ReadKind::PreviewInstall);
                        self.preview_error = Some(
                            self.ui_error(&error, ErrorContext::Read(ReadKind::PreviewInstall)),
                        );
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::ChangelogLoaded(result) => {
                match result {
                    Ok(view) => {
                        self.changelog.succeeded(view);
                        self.clear_retry(ReadKind::Changelog);
                    }
                    Err(error) => {
                        self.changelog.failed(error);
                        self.note_read_failed(ReadKind::Changelog);
                    }
                }
                Task::none()
            }
            Message::DocsLoaded {
                topic,
                version,
                scope,
                result,
            } => {
                let key = DocsKey {
                    topic,
                    version,
                    scope,
                };
                self.docs_loading.remove(&key);
                match result {
                    Ok(page) => {
                        self.docs_errors.remove(&key);
                        self.docs_cache.insert(key, page);
                    }
                    Err(error) => {
                        self.docs_errors.insert(key, error);
                    }
                }
                Task::none()
            }
            Message::RawChangelogLoaded(result) => {
                match result {
                    Ok(markdown) => {
                        self.raw_changelog.succeeded(markdown);
                        self.clear_retry(ReadKind::RawChangelog);
                    }
                    Err(error) => {
                        self.raw_changelog.failed(error);
                        self.note_read_failed(ReadKind::RawChangelog);
                    }
                }
                Task::none()
            }
            Message::CliChangelogLoaded(result) => {
                match result {
                    Ok(raw) => {
                        self.update_changelog.succeeded(raw);
                        self.clear_retry(ReadKind::CliChangelog);
                    }
                    Err(error) => {
                        self.update_changelog.failed(error);
                        self.note_read_failed(ReadKind::CliChangelog);
                    }
                }
                Task::none()
            }
            Message::CliDryRunLoaded(result) => {
                match result {
                    Ok(value) => {
                        self.update_dry_run.succeeded(pretty_json(&value));
                        self.clear_retry(ReadKind::CliDryRun);
                    }
                    Err(error) => {
                        self.update_dry_run.failed(error);
                        self.note_read_failed(ReadKind::CliDryRun);
                    }
                }
                Task::none()
            }
            Message::DoctorLoaded(result) => {
                match result {
                    Ok(read) => {
                        self.doctor.succeeded(read);
                        self.clear_retry(ReadKind::Doctor);
                    }
                    Err(error) => {
                        self.doctor.failed(error);
                        self.note_read_failed(ReadKind::Doctor);
                    }
                }
                Task::none()
            }
            Message::EnvironmentLoaded(result) => {
                match result {
                    Ok(environment) => {
                        self.environment.succeeded(environment);
                        self.clear_retry(ReadKind::Environment);
                    }
                    Err(error) => {
                        self.environment.failed(error);
                        self.note_read_failed(ReadKind::Environment);
                    }
                }
                Task::none()
            }
            Message::DiagnoseProxyLoaded(result) => {
                self.note_diagnose_result(ReadKind::DiagnoseProxy, result);
                Task::none()
            }
            Message::DiagnoseAcceleratorLoaded(result) => {
                self.note_diagnose_result(ReadKind::DiagnoseAccelerator, result);
                Task::none()
            }
            Message::DiagnoseUpdateLoaded(result) => {
                self.note_diagnose_result(ReadKind::DiagnoseUpdate, result);
                Task::none()
            }
            Message::SkillClientsLoaded(result) => {
                match result {
                    Ok(clients) => {
                        self.skill_clients.succeeded(clients);
                        self.clear_retry(ReadKind::SkillClients);
                    }
                    Err(error) => {
                        self.skill_clients.failed(error);
                        self.note_read_failed(ReadKind::SkillClients);
                    }
                }
                Task::none()
            }
            Message::SkillInstallsLoaded(result) => {
                match result {
                    Ok(installs) => {
                        self.skill_installs.succeeded(installs);
                        self.clear_retry(ReadKind::SkillInstalls);
                    }
                    Err(error) => {
                        self.skill_installs.failed(error);
                        self.note_read_failed(ReadKind::SkillInstalls);
                    }
                }
                Task::none()
            }
            Message::InstallSkill(client) => self.start_skill_install(Some(client), false),
            Message::InstallAllSkills => self.start_skill_install(None, true),
            Message::SkillLocalToggled(value) => {
                if value && self.project_context.is_none() {
                    // The UI hides the control, but the controller is the trust
                    // boundary: a stray action must not enable `--local`.
                    self.skill_install_local = false;
                    self.notice = Some(Notice::error(
                        "`--local` needs a Unity project context. Launch the Hub from a project folder, then try again.",
                    ));
                } else {
                    self.skill_install_local = value;
                }
                Task::none()
            }
            Message::SkillOverwriteToggled(value) => {
                self.skill_install_overwrite = value;
                Task::none()
            }
            Message::SkillDryRunToggled(value) => {
                self.skill_install_dry_run = value;
                Task::none()
            }
            Message::SkillNameChanged(value) => {
                self.skill_install_skill = value;
                Task::none()
            }
            Message::StartSkillRefresh => {
                let dry_run = self.next_skill_refresh_dry_run();
                self.start_skill_refresh(dry_run)
            }
            Message::PreviewSkillRefresh => self.start_skill_refresh(true),
            Message::ForceSkillLastRun => self.force_skill_last_run(),
            Message::PruneLoaded(result) => {
                match result {
                    Ok(report) => {
                        self.prune.succeeded(report);
                        self.clear_retry(ReadKind::Prune);
                    }
                    Err(error) => {
                        self.prune.failed(error);
                        self.note_read_failed(ReadKind::Prune);
                    }
                }
                Task::none()
            }
            Message::VerifyLoaded(result) => {
                match result {
                    Ok(read) => {
                        // A failed verify still has a report; open the raw log
                        // so the failing paths and the CLI's own errors are
                        // visible without another click.
                        self.verify_show_raw = !read.report.ok;
                        self.verify.succeeded(read);
                        self.clear_retry(ReadKind::EditorVerify);
                    }
                    Err(error) => {
                        self.verify.failed(error);
                        self.note_read_failed(ReadKind::EditorVerify);
                    }
                }
                Task::none()
            }
            Message::PollJobs => self.poll_jobs(),
            Message::DismissJob => {
                self.active_job = None;
                Task::none()
            }
            Message::CancelJob => {
                if let Some(job) = &self.active_job {
                    if self.jobs.cancel(&job.id) {
                        self.notifications.push_event(
                            NotificationLevel::Info,
                            "Job",
                            "Cancelling the job…",
                        );
                    }
                }
                Task::none()
            }
            Message::ToggleNotificationRail => {
                let open = !self.notifications.rail_open();
                self.notifications.set_rail_open(open);
                Task::none()
            }
            Message::DismissNotification(id) => {
                self.notifications.dismiss(id);
                Task::none()
            }
            Message::ClearNotifications => {
                self.notifications.clear();
                Task::none()
            }
            Message::DismissToast(id) => {
                // A notice rendered as a toast uses id 0, which no stored
                // notification can ever carry.
                if id == 0 {
                    self.notice = None;
                }
                self.notifications.toast_expired(id);
                Task::none()
            }
            Message::ToastExpired(id) => {
                self.notifications.toast_expired(id);
                Task::none()
            }
            Message::CopyNotificationCommand(id) => {
                match self
                    .notifications
                    .get(id)
                    .and_then(|notification| notification.command.clone())
                {
                    Some(command) => self.copy_report(command, "Command copied to the clipboard."),
                    None => Task::none(),
                }
            }
            Message::RunNotificationCommand(id) => {
                if self
                    .notifications
                    .get(id)
                    .is_some_and(|notification| notification.can_run)
                {
                    self.notifications.set_state(id, ActionState::Confirming);
                }
                Task::none()
            }
            Message::CancelNotificationCommand(id) => {
                if self
                    .notifications
                    .get(id)
                    .is_some_and(|notification| notification.state == ActionState::Confirming)
                {
                    self.notifications.set_state(id, ActionState::Idle);
                }
                Task::none()
            }
            Message::ConfirmNotificationCommand(id) => self.confirm_notification(id),
            Message::DismissNotice => {
                self.notice = None;
                Task::none()
            }
            Message::RunErrorAction(action) => self.run_error_action(action),
            Message::CopyDiagnosticsReport => {
                let text = self.diagnostics_report_text();
                self.copy_report(text, "Diagnostics report copied to the clipboard.")
            }
            Message::CopyDiagnoseReport(topic) => {
                let text = self.diagnose_text(topic).unwrap_or_default();
                self.copy_report(
                    text,
                    format!("The {} report was copied to the clipboard.", topic.label()),
                )
            }
            Message::SignIn => {
                match self.cli.auth_login_spec() {
                    Ok(spec) => {
                        self.start_job("auth-login", spec);
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::TextSizeChanged(size) => {
                self.app_settings.text_scale_percent = size.percent();
                match settings::save(&self.app_settings) {
                    Ok(()) => {
                        self.notifications.push_event(
                            NotificationLevel::Success,
                            "Notification",
                            format!("Text size set to {size}."),
                        );
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::FontFamilyChanged(family) => {
                let family = family.trim();
                self.app_settings.font_family = if family.is_empty() {
                    settings::default_font_family()
                } else {
                    family.to_string()
                };
                match settings::save(&self.app_settings) {
                    Ok(()) => {
                        self.notifications.push_event(
                            NotificationLevel::Success,
                            "Notification",
                            format!("Font set to {}.", self.app_settings.font_family),
                        );
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::OpenInstallCli => {
                self.modal = Modal::InstallCli;
                self.cli_consent = false;
                Task::none()
            }
            Message::CliChannelChanged(channel) => {
                self.cli_channel = channel;
                Task::none()
            }
            Message::CliConsentToggled(value) => {
                self.cli_consent = value;
                Task::none()
            }
            Message::InstallCli => {
                if !self.cli_consent {
                    return Task::none();
                }
                let spec = bootstrap::install_cli_spec(Some(&self.cli_channel));
                let started = self.start_job("cli-install", spec);
                if started {
                    self.modal = Modal::None;
                }
                Task::none()
            }
            Message::UpdateCli => {
                // The Overview shortcut never applies blindly: it selects the
                // available target and loads its release notes. Apply stays
                // disabled until those notes have loaded.
                if let Some(update) = &self.update.data {
                    if !update.latest_version.trim().is_empty() {
                        self.update_target = update.latest_version.clone();
                    }
                }
                self.update_changelog = Loadable::default();
                self.update_dry_run = Loadable::default();
                self.load_update_changelog()
            }
            Message::CliUpdateChannelChanged(channel) => {
                self.update_channel_touched = true;
                self.apply_update_channel(channel.clone());
                // The badge must describe the selected channel, not whichever
                // channel the CLI happens to track by default.
                self.update.begin();
                load_update_on(self.cli.clone(), Some(channel))
            }
            Message::CliUpdateTargetChanged(target) => {
                self.update_target = target;
                self.update_changelog = Loadable::default();
                self.update_dry_run = Loadable::default();
                Task::none()
            }
            Message::LoadCliUpdateChangelog => self.load_update_changelog(),
            Message::PreviewCliUpdate => self.preview_cli_update(),
            Message::StartCliUpdate => {
                if !self.cli_update_can_apply() {
                    self.notice = Some(Notice::info(
                        "Load the target release notes before updating the CLI.",
                    ));
                    return Task::none();
                }
                let target = trimmed_option(&self.update_target);
                let channel = trimmed_option(&self.update_channel);
                match self
                    .cli
                    .self_update_apply_spec(target.as_deref(), channel.as_deref())
                {
                    Ok(spec) => {
                        self.start_job("cli-update", spec);
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::OpenCliRollback => {
                self.rollback_target.clear();
                self.rollback_confirmed = false;
                self.rollback_error = None;
                self.modal = Modal::RollbackCli;
                Task::none()
            }
            Message::CliRollbackVersionChanged(version) => {
                self.rollback_target = version;
                self.rollback_confirmed = false;
                Task::none()
            }
            Message::CliRollbackConfirmed(value) => {
                self.rollback_confirmed = value;
                Task::none()
            }
            Message::StartCliRollback => {
                if !matches!(self.modal, Modal::RollbackCli) || !self.rollback_confirmed {
                    return Task::none();
                }
                let target = trimmed_option(&self.rollback_target);
                match self.cli.self_update_rollback_spec(target.as_deref()) {
                    Ok(spec) => {
                        self.rollback_error = None;
                        self.start_job("cli-rollback", spec);
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::InstallVersionChanged(version) => {
                self.install_version = version;
                Task::none()
            }
            Message::InstallArchitectureChanged(architecture) => {
                let architecture = architecture
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                if self.install_architecture == architecture {
                    return Task::none();
                }
                self.install_architecture = architecture;
                // The catalog and the preview describe one build. Changing the
                // build invalidates both instead of carrying selections over.
                self.modules = Loadable::default();
                self.selected_modules.clear();
                self.preview = None;
                self.preview_raw = None;
                self.preview_error = None;
                self.preview_loading = false;
                Task::none()
            }
            Message::LoadModules => {
                self.modules.begin();
                self.selected_modules.clear();
                self.preview = None;
                self.preview_raw = None;
                self.preview_error = None;
                let version = self.install_version.trim().to_string();
                let architecture = self.install_architecture.clone();
                self.retry_targets.insert(
                    ReadKind::Modules.id().to_string(),
                    RetryRequest::Modules {
                        version: version.clone(),
                        architecture: architecture.clone(),
                    },
                );
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version, architecture.as_deref())
                    }),
                    Message::ModulesLoaded,
                )
            }
            Message::ToggleModule(id, value) => {
                if value {
                    self.selected_modules.insert(id);
                } else {
                    self.selected_modules.remove(&id);
                }
                Task::none()
            }
            Message::SelectAllModules(value) => {
                if value {
                    if let Some(modules) = &self.modules.data {
                        self.selected_modules = modules
                            .iter()
                            .filter(|module| !module.is_installed)
                            .map(|module| module.id.clone())
                            .collect();
                    }
                } else {
                    self.selected_modules.clear();
                }
                Task::none()
            }
            Message::HeadlessToggled(value) => {
                self.install_headless = value;
                Task::none()
            }
            Message::AcceptEulaToggled(value) => {
                self.accept_eula = value;
                Task::none()
            }
            Message::InstallAdvancedToggled(value) => {
                self.install_advanced_expanded = value;
                Task::none()
            }
            Message::InstallResumeToggled(value) => {
                self.install_resume = value;
                if value {
                    self.install_download_only = false;
                    self.install_set_default = false;
                    self.install_advanced_note = Some(
                        "Resume reuses the download cache, so download-only and set-default were cleared. Those combinations are usage errors."
                            .to_string(),
                    );
                }
                Task::none()
            }
            Message::InstallDownloadOnlyToggled(value) => {
                self.install_download_only = value;
                if value {
                    self.install_resume = false;
                    self.install_set_default = false;
                    self.install_advanced_note = Some(
                        "Download-only installs nothing and registers nothing, so resume and set-default were cleared."
                            .to_string(),
                    );
                }
                Task::none()
            }
            Message::InstallNoWaitToggled(value) => {
                self.install_no_wait = value;
                if value {
                    self.install_timeout.clear();
                    self.install_advanced_note = Some(
                        "No-wait fails immediately when another install holds the lock, so it cleared the timeout. The CLI refuses both at once."
                            .to_string(),
                    );
                }
                Task::none()
            }
            Message::InstallTimeoutChanged(value) => {
                self.install_no_wait = false;
                let had_value = !value.trim().is_empty();
                self.install_timeout = value;
                if had_value {
                    self.install_advanced_note = Some(
                        "A timeout bounds the wait for another install's lock, so it cleared no-wait. The CLI refuses both at once."
                            .to_string(),
                    );
                }
                Task::none()
            }
            Message::InstallSetDefaultToggled(value) => {
                self.install_set_default = value;
                if value {
                    self.install_resume = false;
                    self.install_download_only = false;
                    self.install_advanced_note = Some(
                        "Set-default stores the installed Editor after success; resume and download-only cannot store it, so they were cleared."
                            .to_string(),
                    );
                }
                Task::none()
            }
            Message::InstallRetriesChanged(value) => {
                self.install_retries = value;
                Task::none()
            }
            Message::ResumeInstallEditor => {
                let Some(offer) = self.resume_offer.clone() else {
                    return Task::none();
                };
                let mut request = offer.request;
                request.options = InstallOptions {
                    resume: true,
                    // Resume must not grow a fresh lock wait or a default write.
                    ..InstallOptions::default()
                };
                match self.cli.install_editor_spec(
                    &request.version,
                    request.architecture.as_deref(),
                    &request.modules,
                    request.headless,
                    request.accept_eula,
                    &request.options,
                ) {
                    Ok(spec) => {
                        self.last_install = Some(request);
                        if self.start_job("editor-install", spec) {
                            self.resume_offer = None;
                            self.download_report = None;
                        }
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::PreviewInstall => {
                self.preview_loading = true;
                self.preview_error = None;
                let version = self.install_version.trim().to_string();
                let architecture = self.install_architecture.clone();
                let modules: Vec<String> = self.selected_modules.iter().cloned().collect();
                let headless = self.install_headless;
                self.retry_targets.insert(
                    ReadKind::PreviewInstall.id().to_string(),
                    RetryRequest::PreviewInstall {
                        version: version.clone(),
                        architecture: architecture.clone(),
                        modules: modules.clone(),
                        headless,
                    },
                );
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || {
                            cli.dry_run_install(
                                &version,
                                architecture.as_deref(),
                                &modules,
                                headless,
                            )
                        }
                    }),
                    Message::PreviewLoaded,
                )
            }
            Message::StartInstallEditor => {
                let request = self.current_install_request();
                match self.install_editor_spec() {
                    Ok(spec) => {
                        if self.start_job("editor-install", spec) {
                            self.last_install = Some(request);
                            self.download_report = None;
                            self.resume_offer = None;
                            self.modules.data = None;
                            self.selected_modules.clear();
                            self.install_headless = false;
                            self.accept_eula = false;
                        }
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::UseRelease(version, architecture) => {
                self.install_version = version.clone();
                let architecture =
                    Some(architecture.trim().to_string()).filter(|value| !value.is_empty());
                self.install_architecture = architecture.clone();
                self.modules.begin();
                self.selected_modules.clear();
                self.preview = None;
                self.preview_raw = None;
                self.preview_error = None;
                self.retry_targets.insert(
                    ReadKind::Modules.id().to_string(),
                    RetryRequest::Modules {
                        version: version.clone(),
                        architecture: architecture.clone(),
                    },
                );
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version, architecture.as_deref())
                    }),
                    Message::ModulesLoaded,
                )
            }
            Message::OpenModules(version, architecture) => {
                // The row identity is `(version, architecture)`. A stale action
                // must do nothing rather than fall back to another build.
                let Some(editor) = self.editors.data.as_ref().and_then(|editors| {
                    editors
                        .iter()
                        .find(|editor| {
                            editor.version == version && editor.architecture == architecture
                        })
                        .cloned()
                }) else {
                    return Task::none();
                };
                self.dialog_modules.begin();
                self.dialog_selected.clear();
                self.dialog_eula = false;
                let version = editor.version.clone();
                let architecture = editor.architecture.clone();
                self.retry_targets.insert(
                    ReadKind::DialogModules.id().to_string(),
                    RetryRequest::DialogModules {
                        version: version.clone(),
                        architecture: architecture.clone(),
                    },
                );
                self.modal = Modal::EditorModules(editor);
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version, Some(&architecture))
                    }),
                    Message::DialogModulesLoaded,
                )
            }
            Message::CloseModal => {
                self.modal = Modal::None;
                self.prune_confirmed = false;
                self.prune_missing_confirmed = false;
                self.prune_force = false;
                self.prune_error = None;
                self.verify_show_raw = false;
                self.rollback_confirmed = false;
                Task::none()
            }
            Message::ToggleDialogModule(id, value) => {
                if value {
                    self.dialog_selected.insert(id);
                } else {
                    self.dialog_selected.remove(&id);
                }
                Task::none()
            }
            Message::DialogEulaToggled(value) => {
                self.dialog_eula = value;
                Task::none()
            }
            Message::DialogRetriesChanged(value) => {
                self.dialog_retries = value;
                Task::none()
            }
            Message::DialogDownloadDirectoryChanged(value) => {
                self.dialog_download_directory = value;
                Task::none()
            }
            Message::StartInstallModules => {
                let Modal::EditorModules(editor) = &self.modal else {
                    return Task::none();
                };
                if self.dialog_selected.is_empty() || !self.dialog_eula {
                    return Task::none();
                }
                let modules: Vec<String> = self.dialog_selected.iter().cloned().collect();
                let architecture = editor.architecture.clone();
                let version = editor.version.clone();
                let options = match self.dialog_module_options() {
                    Ok(options) => options,
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                        return Task::none();
                    }
                };
                match self.cli.install_modules_spec(
                    &version,
                    Some(&architecture),
                    &modules,
                    self.dialog_eula,
                    &options,
                ) {
                    Ok(spec) => {
                        if self.start_job("module-install", spec) {
                            self.modal = Modal::None;
                            self.dialog_selected.clear();
                        }
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::SetDefaultEditor(version, architecture) => {
                match self.editor_default_spec(&version, &architecture) {
                    Ok(spec) => {
                        self.start_job("editor-default", spec);
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::ClearDefaultEditor => {
                match self.cli.editor_default_unset_spec() {
                    Ok(spec) => {
                        self.start_job("editor-default", spec);
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::OpenPrune => self.open_prune(),
            Message::PruneScan => self.scan_prune(),
            Message::PruneConfirmed(value) => {
                // A confirmation is meaningful only for the Ready report the
                // user is looking at. A stale tick from an earlier scan must not
                // arm a removal that a newer report cannot describe.
                self.prune_confirmed = value && self.prune_report_is_ready();
                Task::none()
            }
            Message::PruneMissingConfirmed(value) => {
                self.prune_missing_confirmed = value && self.prune_report_is_ready();
                Task::none()
            }
            Message::PruneForceToggled(value) => {
                // `--force` only exists for a failed running-Editor check, so a
                // stale toggle cannot turn it on before that failure.
                self.prune_force = value && self.prune_force_available;
                Task::none()
            }
            Message::StartPruneRemove => {
                if !matches!(self.modal, Modal::PruneEditors)
                    || !self.prune_confirmed
                    || !self.prune_report_is_ready()
                {
                    return Task::none();
                }
                let has_candidates = self
                    .prune
                    .data
                    .as_ref()
                    .is_some_and(|report| !report.editors.is_empty());
                if !has_candidates {
                    return Task::none();
                }
                let force = self.prune_force && self.prune_force_available;
                match self.cli.editors_prune_remove_spec(force) {
                    Ok(spec) => {
                        if self.start_job("editor-prune", spec) {
                            self.prune_confirmed = false;
                        }
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::StartPruneRemoveMissing => {
                if !matches!(self.modal, Modal::PruneEditors)
                    || !self.prune_missing_confirmed
                    || !self.prune_report_is_ready()
                {
                    return Task::none();
                }
                let has_missing = self
                    .prune
                    .data
                    .as_ref()
                    .is_some_and(|report| !report.missing.is_empty());
                if !has_missing {
                    return Task::none();
                }
                match self.cli.editors_prune_remove_missing_spec() {
                    Ok(spec) => {
                        if self.start_job("editor-prune-missing", spec) {
                            self.prune_missing_confirmed = false;
                        }
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::OpenVerify(version, architecture) => self.open_verify(&version, &architecture),
            Message::ToggleVerifyRaw => {
                self.verify_show_raw = !self.verify_show_raw;
                Task::none()
            }
            Message::ProjectSearchChanged(value) => {
                self.project_search = value;
                Task::none()
            }
            Message::AddProject => {
                Task::perform(pick_project_folder(), Message::ProjectFolderPicked)
            }
            Message::ProjectFolderPicked(Some(path)) => {
                let path = path.display().to_string();
                self.notifications.push_event(
                    NotificationLevel::Info,
                    "Notification",
                    format!("Adding {path}…"),
                );
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        let path = path.clone();
                        move || cli.add_project(&path)
                    }),
                    |result| Message::ProjectMutationDone(ProjectMutation::Added, result),
                )
            }
            Message::ProjectFolderPicked(None) => Task::none(),
            Message::OpenProjectRemove(path) => {
                let Some(project) = self
                    .projects
                    .data
                    .as_ref()
                    .and_then(|projects| projects.iter().find(|project| project.path == path))
                    .cloned()
                else {
                    return Task::none();
                };
                self.modal = Modal::RemoveProject(project);
                Task::none()
            }
            Message::ConfirmProjectRemove => {
                let Modal::RemoveProject(project) = &self.modal else {
                    return Task::none();
                };
                let path = project.path.clone();
                let title = project.title.clone();
                self.modal = Modal::None;
                self.notifications.push_event(
                    NotificationLevel::Info,
                    "Notification",
                    format!("Removing {title} from the registry…"),
                );
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        let path = path.clone();
                        move || cli.remove_project(&path)
                    }),
                    move |result| {
                        Message::ProjectMutationDone(
                            ProjectMutation::Removed(title.clone()),
                            result,
                        )
                    },
                )
            }
            Message::ToggleProjectFavorite(path, favorite) => {
                let Some(project) = self
                    .projects
                    .data
                    .as_ref()
                    .and_then(|projects| projects.iter().find(|project| project.path == path))
                else {
                    return Task::none();
                };
                let title = project.title.clone();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        let path = path.clone();
                        move || cli.set_project_pinned(&path, favorite)
                    }),
                    move |result| {
                        Message::ProjectMutationDone(
                            ProjectMutation::Pinned(title.clone(), favorite),
                            result,
                        )
                    },
                )
            }
            Message::ProjectMutationDone(mutation, result) => match result {
                Ok(()) => {
                    self.notifications.push_event(
                        NotificationLevel::Success,
                        "Notification",
                        match mutation {
                            ProjectMutation::Added => "Project added to the registry.".to_string(),
                            ProjectMutation::Removed(title) => {
                                format!("{title} removed from the registry.")
                            }
                            ProjectMutation::Pinned(title, true) => {
                                format!("{title} marked as a favorite.")
                            }
                            ProjectMutation::Pinned(title, false) => {
                                format!("{title} removed from favorites.")
                            }
                        },
                    );
                    self.load_projects_page(true)
                }
                Err(error) => {
                    self.notifications.push_error(&error);
                    Task::none()
                }
            },
            Message::OpenProject(path) => {
                let title = self
                    .projects
                    .data
                    .as_ref()
                    .and_then(|projects| projects.iter().find(|project| project.path == path))
                    .map(|project| project.title.clone())
                    .unwrap_or_else(|| project_name_from_path(&path));
                self.notifications.push_event(
                    NotificationLevel::Info,
                    "Notification",
                    format!("Opening {title}…"),
                );
                let ensure_broker = needs_auth_broker();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || {
                            if ensure_broker {
                                // Best effort: `open` is the command that
                                // needs the session most, and its own error
                                // describes a real launch failure better.
                                let _ = cli.ensure_auth_broker();
                            }
                            cli.open_project(&path)
                        }
                    }),
                    Message::ProjectOpened,
                )
            }
            Message::ProjectOpened(result) => {
                match result {
                    Ok(()) => {
                        self.notifications.push_event(
                            NotificationLevel::Success,
                            "Notification",
                            "Unity Editor is launching.",
                        );
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::OpenUpgrade(path) => {
                let Some(project) = self
                    .projects
                    .data
                    .as_ref()
                    .and_then(|projects| projects.iter().find(|project| project.path == path))
                    .cloned()
                else {
                    return Task::none();
                };
                self.upgrade_target = self.first_other_editor_version(&project.version);
                self.upgrade_confirmed = false;
                self.modal = Modal::UpgradeProject(project);
                self.refresh_running_editors()
            }
            Message::UpgradeTargetChanged(version) => {
                self.upgrade_target = Some(version);
                self.upgrade_confirmed = false;
                Task::none()
            }
            Message::UpgradeConfirmed(value) => {
                self.upgrade_confirmed = value;
                Task::none()
            }
            Message::StartUpgrade => {
                let Modal::UpgradeProject(project) = &self.modal else {
                    return Task::none();
                };
                let project_path = project.path.clone();
                let Some(target) = self.upgrade_target.clone() else {
                    return Task::none();
                };
                if !self.upgrade_confirmed {
                    return Task::none();
                }
                // Best-effort UX guard. The CLI still owns the authoritative
                // check and reports exit 6 if the project is open.
                if let Some(editor) = self.running_editor_for_project(&project_path) {
                    let label = editor
                        .project_path
                        .clone()
                        .unwrap_or_else(|| project_path.clone());
                    self.notifications.push_event(
                        NotificationLevel::Danger,
                        "Error",
                        format!(
                            "Close the running Unity Editor for {label} before upgrading this project."
                        ),
                    );
                    return Task::none();
                }
                match self.cli.upgrade_project_spec(&project_path, &target) {
                    Ok(spec) => {
                        if self.start_job("project-upgrade", spec) {
                            self.modal = Modal::None;
                        }
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                    }
                }
                Task::none()
            }
            Message::ChangelogTargetChanged(target) => {
                self.changelog_target = Some(target);
                self.raw_changelog = Loadable::default();
                self.show_raw_changelog = false;
                if self.changelog_loaded {
                    self.load_changelog_page(true)
                } else {
                    Task::none()
                }
            }
            Message::LoadChangelog => {
                self.changelog_loaded = true;
                self.raw_changelog = Loadable::default();
                self.show_raw_changelog = false;
                self.load_changelog_page(true)
            }
            Message::ToggleRawChangelog => {
                if self.show_raw_changelog {
                    self.show_raw_changelog = false;
                    self.raw_changelog = Loadable::default();
                    return Task::none();
                }

                self.show_raw_changelog = true;
                if self.raw_changelog.loading || self.raw_changelog.data.is_some() {
                    return Task::none();
                }

                self.raw_changelog.begin();
                let cli = self.cli.clone();
                let target = self.changelog_target.clone();
                Task::perform(
                    run_blocking(move || cli.changelog(target.as_deref()).map(|raw| raw.changelog)),
                    Message::RawChangelogLoaded,
                )
            }
            Message::ToggleChangelogImpact(value) => {
                self.changelog_app_impact_only = value;
                Task::none()
            }
            Message::ChangelogDocsScopeChanged(scope) => {
                self.changelog_docs_scope = scope;
                Task::none()
            }
            Message::ChangelogDocsPackageChanged(package) => {
                self.changelog_docs_package = package;
                Task::none()
            }
            Message::LookupChangelogDocs { topic, scope } => {
                self.lookup_changelog_docs(topic, scope)
            }
            Message::OpenExternalUrl(url) => {
                match open_external_url(&url) {
                    Ok(()) => {
                        self.notice = Some(Notice::info("Documentation opened in your browser."));
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(format!(
                            "The documentation page could not be opened. {error}"
                        )));
                    }
                }
                Task::none()
            }
            Message::CliPathChanged(value) => {
                self.cli_path_input = value;
                Task::none()
            }
            Message::SaveCliPath => {
                let value = self.cli_path_input.trim().to_string();
                let settings = AppSettings {
                    cli_path: if value.is_empty() {
                        None
                    } else {
                        Some(value.clone())
                    },
                    ..self.app_settings.clone()
                };
                if !value.is_empty() {
                    let path = PathBuf::from(&value);
                    if !path.is_file() {
                        self.notifications.push_event(
                            NotificationLevel::Danger,
                            "Error",
                            format!("`{value}` is not a file. Choose the unity executable."),
                        );
                        return Task::none();
                    }
                }
                match settings::save(&settings) {
                    Ok(()) => {
                        self.app_settings = settings;
                        self.cli.set_override_path(
                            self.app_settings.cli_path.as_ref().map(PathBuf::from),
                        );
                        self.notifications.push_event(
                            NotificationLevel::Success,
                            "Notification",
                            "CLI path saved.",
                        );
                        self.refresh_status()
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                        Task::none()
                    }
                }
            }
            Message::ClearCliPath => {
                self.cli_path_input.clear();
                self.app_settings.cli_path = None;
                match settings::save(&self.app_settings) {
                    Ok(()) => {
                        self.cli.set_override_path(None);
                        self.notifications.push_event(
                            NotificationLevel::Success,
                            "Notification",
                            "CLI path override cleared.",
                        );
                        self.refresh_status()
                    }
                    Err(error) => {
                        self.notifications.push_error(&error);
                        Task::none()
                    }
                }
            }

            // -----------------------------------------------------------------
            // Templates
            Message::TemplatesEditorChanged(version) => {
                self.template_editor = Some(version);
                self.template_selected = None;
                self.template_detail = Loadable::default();
                self.load_templates(true)
            }
            Message::TemplateFilterChanged(filter) => {
                self.template_filter = filter;
                self.load_templates(true)
            }
            Message::TemplateInstalledOnlyToggled(value) => {
                self.template_installed_only = value;
                self.load_templates(true)
            }
            Message::OpenTemplateDetail(name) => {
                let Some(editor) = self.template_editor.clone() else {
                    return Task::none();
                };
                self.template_selected = Some(name.clone());
                self.template_detail.begin();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.template_info(&editor, &name)
                    }),
                    Message::TemplateInfoLoaded,
                )
            }
            Message::TemplateInfoLoaded(result) => {
                match result {
                    Ok(info) => self.template_detail.succeeded(info),
                    Err(error) => self.template_detail.failed(error),
                }
                Task::none()
            }
            Message::TemplatesLoaded(result) => {
                match result {
                    Ok(templates) => self.templates.succeeded(templates),
                    Err(error) => self.templates.failed(error),
                }
                Task::none()
            }
            Message::OpenTemplateAuthoring(mode) => {
                let mut state = TemplateAuthoringState::new(mode);
                state.source = self
                    .projects
                    .data
                    .as_ref()
                    .and_then(|projects| projects.first())
                    .map(|project| project.path.clone())
                    .unwrap_or_default();
                self.modal = Modal::TemplateAuthoring(Box::new(state));
                self.refresh_authoring_issues();
                if self.projects.data.is_none() && !self.projects.loading {
                    self.projects.begin();
                    load_projects(self.cli.clone())
                } else {
                    Task::none()
                }
            }
            Message::TemplateAuthoringProjectChanged(path) => {
                if let Modal::TemplateAuthoring(state) = &mut self.modal {
                    state.source = path;
                    state.result = None;
                }
                self.refresh_authoring_issues();
                Task::none()
            }
            Message::TemplateFieldChanged(field, value) => {
                if let Modal::TemplateAuthoring(state) = &mut self.modal {
                    match field {
                        TemplateField::Name => state.form.name = value,
                        TemplateField::DisplayName => state.form.display_name = value,
                        TemplateField::Description => state.form.description = value,
                        TemplateField::Version => state.form.version = value,
                        TemplateField::Output => state.form.output = value,
                    }
                    state.overwrite_confirmed = false;
                    state.result = None;
                }
                self.refresh_authoring_issues();
                Task::none()
            }
            Message::TemplateOverwriteToggled(value) => {
                if let Modal::TemplateAuthoring(state) = &mut self.modal {
                    state.overwrite_confirmed = value;
                }
                self.refresh_authoring_issues();
                Task::none()
            }
            Message::StartTemplateAuthoring => {
                let Modal::TemplateAuthoring(state) = &self.modal else {
                    return Task::none();
                };
                if !state.issues.is_empty() {
                    return Task::none();
                }
                let mode = state.mode;
                let options = state.create_options();
                let pack_options = state.pack_options();
                let (kind, spec) = if mode.is_pack() {
                    ("template-pack", self.cli.templates_pack_spec(&pack_options))
                } else {
                    ("template-create", self.cli.templates_create_spec(&options))
                };
                match spec {
                    Ok(spec) => {
                        if mode.is_pack() {
                            if let Some(parent) =
                                std::path::Path::new(&pack_options.output).parent()
                            {
                                if !parent.as_os_str().is_empty() {
                                    if let Err(error) = std::fs::create_dir_all(parent) {
                                        if let Modal::TemplateAuthoring(state) = &mut self.modal {
                                            state.issues.push(format!(
                                                "Could not create the archive folder: {error}"
                                            ));
                                        }
                                        return Task::none();
                                    }
                                }
                            }
                        }
                        if self.start_job(kind, spec) {
                            if let Modal::TemplateAuthoring(state) = &mut self.modal {
                                state.result = None;
                            }
                        }
                    }
                    Err(error) => {
                        if let Modal::TemplateAuthoring(state) = &mut self.modal {
                            state
                                .issues
                                .push(format!("{}: {}", error.code, error.message));
                        }
                    }
                }
                Task::none()
            }

            // -----------------------------------------------------------------
            // New Project wizard
            Message::OpenNewProject => {
                let mut state = NewProjectState::new();
                state.form.parent_dir = dirs::home_dir()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default();
                self.modal = Modal::NewProject(Box::new(state));
                if let Some(id) = self.notifications.toast().map(|toast| toast.id) {
                    self.notifications.toast_expired(id);
                }
                Task::batch([
                    load_new_project_editors(self.cli.clone()),
                    load_new_project_license(self.cli.clone()),
                ])
            }
            Message::NewProjectLicenseLoaded(result) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    match result {
                        Ok(license) => state.license.succeeded(license),
                        Err(error) => state.license.failed(error),
                    }
                }
                Task::none()
            }
            Message::NewProjectEditorsLoaded(result) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    match result {
                        Ok(editors) => {
                            state.editors.succeeded(editors);
                            if state.form.editor_version.is_empty() {
                                if let Some(editor) = state
                                    .editors
                                    .data
                                    .as_ref()
                                    .and_then(|editors| default_editor(editors))
                                {
                                    state.form.editor_version = editor.version.clone();
                                    state.form.architecture = editor.architecture.clone();
                                }
                            }
                        }
                        Err(error) => state.editors.failed(error),
                    }
                }
                Task::none()
            }
            Message::NewProjectTemplatesLoaded(result) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    match result {
                        Ok(templates) => state.templates.succeeded(templates),
                        Err(error) => state.templates.failed(error),
                    }
                }
                Task::none()
            }
            Message::NewProjectTemplateInfoLoaded(result) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    match result {
                        Ok(info) => state.selected_template = Some(info),
                        Err(error) => {
                            state.selected_template = None;
                            let mut text = error_text(&error);
                            if let Some(offered) = template_offers(&error) {
                                text.push_str(&format!(
                                    "\n\nTemplates offered by the CLI: {}",
                                    offered.join(", ")
                                ));
                            }
                            state.issues.push(text);
                        }
                    }
                }
                Task::none()
            }
            Message::NewProjectFieldChanged(field, value) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    match field {
                        NewProjectField::Name => state.form.name = value,
                        NewProjectField::ParentDir => state.form.parent_dir = value,
                    }
                    state.issues.clear();
                    state.result = None;
                }
                Task::none()
            }
            Message::NewProjectBrowseParent => {
                Task::perform(pick_project_folder(), Message::NewProjectFolderPicked)
            }
            Message::NewProjectFolderPicked(Some(path)) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    state.form.parent_dir = path.display().to_string();
                    state.issues.clear();
                }
                Task::none()
            }
            Message::NewProjectFolderPicked(None) => Task::none(),
            Message::NewProjectEditorChanged(version) => {
                let mut templates_task = Task::none();
                if let Modal::NewProject(state) = &mut self.modal {
                    state.form.editor_version = version.clone();
                    state.form.template.clear();
                    state.selected_template = None;
                    state.issues.clear();
                    if let Some(editor) =
                        state.editors.data.as_ref().and_then(|editors| {
                            editors.iter().find(|editor| editor.version == version)
                        })
                    {
                        state.form.architecture = editor.architecture.clone();
                    }
                }
                if let Some(state) = self.new_project_state() {
                    let editor = state.form.editor_version.clone();
                    let filter = TemplateFilter::Core;
                    let installed = false;
                    if let Modal::NewProject(state) = &mut self.modal {
                        state.templates.begin();
                    }
                    templates_task = Task::perform(
                        run_blocking({
                            let cli = self.cli.clone();
                            move || cli.templates(&editor, filter, installed)
                        }),
                        Message::NewProjectTemplatesLoaded,
                    );
                }
                templates_task
            }
            Message::NewProjectArchitectureChanged(architecture) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    state.form.architecture = architecture;
                    state.issues.clear();
                }
                Task::none()
            }
            Message::NewProjectTemplateChanged(name) => {
                let editor = self
                    .new_project_state()
                    .map(|state| state.form.editor_version.clone())
                    .unwrap_or_default();
                if let Modal::NewProject(state) = &mut self.modal {
                    state.form.template = name.clone();
                    state.selected_template = None;
                    state.issues.clear();
                }
                if editor.is_empty() || name.is_empty() {
                    return Task::none();
                }
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.template_info(&editor, &name)
                    }),
                    Message::NewProjectTemplateInfoLoaded,
                )
            }
            Message::NewProjectBuiltinToggled(value) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    state.form.show_builtin_templates = value;
                }
                Task::none()
            }
            Message::NewProjectOpenToggled(value) => {
                if let Modal::NewProject(state) = &mut self.modal {
                    state.form.open_after_create = value;
                }
                Task::none()
            }
            Message::NewProjectBack => {
                if let Modal::NewProject(state) = &mut self.modal {
                    state.step = match state.step {
                        NewProjectStep::Editor => NewProjectStep::Details,
                        NewProjectStep::Template => NewProjectStep::Editor,
                        NewProjectStep::Review => NewProjectStep::Template,
                        NewProjectStep::Result | NewProjectStep::Details => NewProjectStep::Details,
                    };
                    state.issues.clear();
                }
                Task::none()
            }
            Message::NewProjectNext => {
                let Some(state) = self.new_project_state() else {
                    return Task::none();
                };
                let issues = state.validation_issues();
                let mut task = Task::none();
                if !issues.is_empty() {
                    if let Modal::NewProject(state) = &mut self.modal {
                        state.issues = issues;
                    }
                    return Task::none();
                }

                match state.step {
                    NewProjectStep::Details => {
                        if let Modal::NewProject(state) = &mut self.modal {
                            state.step = NewProjectStep::Editor;
                            state.issues.clear();
                        }
                    }
                    NewProjectStep::Editor => {
                        let editor = state.form.editor_version.clone();
                        if let Modal::NewProject(state) = &mut self.modal {
                            state.step = NewProjectStep::Template;
                            state.templates.begin();
                            state.issues.clear();
                        }
                        task = Task::perform(
                            run_blocking({
                                let cli = self.cli.clone();
                                move || cli.templates(&editor, TemplateFilter::Core, false)
                            }),
                            Message::NewProjectTemplatesLoaded,
                        );
                    }
                    NewProjectStep::Template | NewProjectStep::Result => {
                        if let Modal::NewProject(state) = &mut self.modal {
                            state.step = NewProjectStep::Review;
                            state.issues.clear();
                        }
                    }
                    NewProjectStep::Review => {}
                }
                task
            }
            Message::StartNewProject => {
                let Some(state) = self.new_project_state() else {
                    return Task::none();
                };
                let mut issues = state.validation_issues();
                match state.license.data.as_ref() {
                    Some(license) if !license.active => issues.push(
                        "Unity reports no usable license. Sign in and activate a license before creating a project."
                            .to_string(),
                    ),
                    Some(_) => {}
                    None => issues.push(
                        "License status is still loading. Wait a moment and try again."
                            .to_string(),
                    ),
                }
                if !issues.is_empty() {
                    if let Modal::NewProject(state) = &mut self.modal {
                        state.issues = issues;
                    }
                    return Task::none();
                }

                let options = CreateProjectOptions {
                    name: state.form.name.trim().to_string(),
                    output_dir: state.form.parent_dir.trim().to_string(),
                    editor_version: state.form.editor_version.trim().to_string(),
                    template: state.form.template.trim().to_string(),
                    architecture: state.form.architecture.trim().to_string(),
                    open: state.form.open_after_create,
                    bare: false,
                };
                match self.cli.create_project_spec(&options) {
                    Ok(spec) => {
                        if self.start_job("project-create", spec) {
                            if let Modal::NewProject(state) = &mut self.modal {
                                state.step = NewProjectStep::Result;
                                state.result = None;
                                state.issues.clear();
                            }
                        }
                    }
                    Err(error) => {
                        if let Modal::NewProject(state) = &mut self.modal {
                            state.issues.push(error_text(&error));
                        }
                    }
                }
                Task::none()
            }

            // -----------------------------------------------------------------
            // Processes
            Message::RefreshProcesses => self.load_processes(true),
            Message::ProcessTick => {
                if self.page == Page::Processes {
                    self.load_processes(false)
                } else {
                    Task::none()
                }
            }
            Message::ProcessesLoaded(result) => {
                match result {
                    Ok(running) => self.processes.succeeded(running),
                    Err(error) => self.processes.failed(error),
                }
                self.sample_processes();
                self.process_last_refreshed =
                    Some(chrono::Local::now().format("%H:%M:%S").to_string());
                Task::none()
            }
            Message::ProcessMetricsToggled(value) => {
                self.app_settings.process_metrics = value;
                if !value {
                    self.process_metrics.clear();
                    self.process_sampler.reset();
                } else {
                    self.sample_processes();
                }
                if let Err(error) = settings::save(&self.app_settings) {
                    self.notifications.push_error(&error);
                }
                Task::none()
            }
            Message::CopyText(text) => {
                self.notifications.push_event(
                    NotificationLevel::Success,
                    "Notification",
                    "Copied to the clipboard.",
                );
                iced::clipboard::write(text)
            }
            Message::OpenFolder(path) => {
                if let Err(error) = open_folder(&path) {
                    self.notifications.push_error(&error);
                }
                Task::none()
            }
        }
    }

    fn new_project_state(&self) -> Option<&NewProjectState> {
        match &self.modal {
            Modal::NewProject(state) => Some(state),
            _ => None,
        }
    }

    /// Recomputes the authoring form's issues without running the CLI.
    fn refresh_authoring_issues(&mut self) {
        let Modal::TemplateAuthoring(state) = &mut self.modal else {
            return;
        };
        let mut issues = Vec::new();
        if state.source.trim().is_empty() {
            issues.push("Choose a registered Unity project.".to_string());
        } else if validate_project_path(&state.source).is_err() {
            issues.push(format!(
                "The source project does not exist: {}",
                state.source
            ));
        }

        if state.mode.is_pack() {
            match validate_archive_path(&state.form.output) {
                Err(error) => issues.push(error.message),
                Ok(output) => {
                    if let Ok(project) = validate_project_path(&state.source) {
                        if path_is_inside(&output, &project) {
                            issues.push(
                                "The archive destination cannot be inside the project being packed."
                                    .to_string(),
                            );
                        }
                    }
                    if output.exists() && !state.overwrite_confirmed {
                        issues.push(
                            "The archive already exists. Confirm overwrite to replace it."
                                .to_string(),
                        );
                    }
                }
            }
        } else if let Err(error) = validate_output_dir(&state.form.output) {
            issues.push(error.message);
        }

        if !state.form.name.trim().is_empty() {
            if let Err(error) = validate_package_id(&state.form.name) {
                issues.push(error.message);
            }
        } else {
            issues.push("Enter a template package id.".to_string());
        }
        if state.form.display_name.trim().is_empty() {
            issues.push("Enter a template display name.".to_string());
        }
        if !state.form.version.trim().is_empty() {
            if let Err(error) = validate_template_version(&state.form.version) {
                issues.push(error.message);
            }
        }

        state.issues = issues;
    }

    /// Loads the template catalog for the selected Editor, starting with the
    /// installed-Editor list when it is missing.
    fn load_templates(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if self.editors.data.is_none() && !self.editors.loading {
            self.editors.begin();
            tasks.push(load_editors(self.cli.clone()));
        }

        if let Some(editors) = &self.editors.data {
            let selected = self
                .template_editor
                .clone()
                .filter(|selected| editors.iter().any(|editor| &editor.version == selected))
                .or_else(|| default_editor(editors).map(|editor| editor.version.clone()));
            let changed = selected != self.template_editor;
            self.template_editor = selected;
            if changed {
                self.template_selected = None;
                self.template_detail = Loadable::default();
            }
        }

        if let Some(editor) = self.template_editor.clone() {
            if force || (self.templates.data.is_none() && !self.templates.loading) {
                self.templates.begin();
                let filter = template_filter(self.template_filter);
                let installed = self.template_installed_only;
                tasks.push(Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.templates(&editor, filter, installed)
                    }),
                    Message::TemplatesLoaded,
                ));
            }
        }

        Task::batch(tasks)
    }

    fn load_processes(&mut self, force: bool) -> Task<Message> {
        if self.processes.loading {
            return Task::none();
        }
        if force || self.processes.data.is_none() {
            self.processes.begin();
            return Task::perform(
                run_blocking({
                    let cli = self.cli.clone();
                    move || cli.editors_running()
                }),
                Message::ProcessesLoaded,
            );
        }
        Task::none()
    }

    fn sample_processes(&mut self) {
        if !self.app_settings.process_metrics {
            return;
        }
        let Some(running) = &self.processes.data else {
            return;
        };
        let pids: Vec<u32> = running
            .instances
            .iter()
            .filter_map(|editor| editor.pid)
            .collect();
        self.process_metrics = self.process_sampler.sample(&pids);
    }

    /// Records that a simple read has a retry target. Parameterised reads
    /// insert their request when they start.
    fn note_read_failed(&mut self, kind: ReadKind) {
        self.retry_targets
            .entry(kind.id().to_string())
            .or_insert(RetryRequest::Read(kind));
    }

    fn clear_retry(&mut self, kind: ReadKind) {
        self.retry_targets.remove(kind.id());
    }

    /// Stores one `unity diagnose` answer in the `Loadable` its section owns.
    fn note_diagnose_result(&mut self, kind: ReadKind, result: Result<String, UnityError>) {
        let loadable = match kind {
            ReadKind::DiagnoseProxy => &mut self.diagnose_proxy,
            ReadKind::DiagnoseAccelerator => &mut self.diagnose_accelerator,
            ReadKind::DiagnoseUpdate => &mut self.diagnose_update,
            _ => return,
        };

        match result {
            Ok(text) => {
                loadable.succeeded(text);
                self.clear_retry(kind);
            }
            Err(error) => {
                loadable.failed(error);
                self.note_read_failed(kind);
            }
        }
    }

    fn load_state<T, U>(
        &self,
        kind: ReadKind,
        loadable: &Loadable<T>,
        map: impl Fn(&T) -> U,
    ) -> LoadState<U> {
        if loadable.loading {
            return LoadState::Loading {
                previous: loadable.data.as_ref().map(&map),
            };
        }
        if let Some(error) = &loadable.error {
            return LoadState::Failed {
                error: self.ui_error(error, ErrorContext::Read(kind)),
                previous: loadable.data.as_ref().map(&map),
            };
        }
        match &loadable.data {
            Some(data) => LoadState::Ready(map(data)),
            None => LoadState::Idle,
        }
    }

    /// Turns an adapter error into the display shape, including the
    /// code-specific primary action and its guidance.
    fn ui_error(&self, error: &UnityError, context: ErrorContext) -> UiError {
        let proxy = configured_proxy();
        let mut detail = error
            .detail
            .clone()
            .filter(|detail| !detail.trim().is_empty());
        let mut action = None;

        if let Some(remediation) = remediation_for(error, proxy.as_deref()) {
            if let Some(hint) = remediation.hint.as_ref() {
                detail = Some(match detail {
                    Some(existing) => format!("{}\n\n{hint}", existing.trim_end()),
                    None => hint.clone(),
                });
            }
            action = self.error_action(&remediation, &context);
        }

        UiError {
            code: error.code.clone(),
            message: error.message.clone(),
            detail,
            action,
        }
    }

    fn error_action(
        &self,
        remediation: &unity_core::errors::ErrorRemediation,
        context: &ErrorContext,
    ) -> Option<ErrorActionVm> {
        let kind = if remediation.kind.is_retry() {
            match context {
                ErrorContext::Read(kind) => {
                    // No controller target means no Retry button: a button that
                    // cannot name its read would be a lie.
                    if !self.retry_targets.contains_key(kind.id()) {
                        return None;
                    }
                    ErrorActionKind::Retry(kind.id().to_string())
                }
                ErrorContext::Job { kind } => {
                    if !job_allows_retry(kind) {
                        return None;
                    }
                    ErrorActionKind::Retry(format!("job:{kind}"))
                }
            }
        } else {
            match remediation.kind {
                RemediationKind::SignIn => ErrorActionKind::SignIn,
                RemediationKind::OpenSettings => ErrorActionKind::OpenSettings,
                RemediationKind::PickVersion => ErrorActionKind::PickVersion,
                RemediationKind::OpenEditor => ErrorActionKind::OpenEditor,
                RemediationKind::EnterToken => ErrorActionKind::EnterToken,
                RemediationKind::InstallGit => ErrorActionKind::InstallGit,
                RemediationKind::ShowCommand => ErrorActionKind::ShowCommand,
                RemediationKind::Confirm => ErrorActionKind::Confirm,
                RemediationKind::ShowBusy => ErrorActionKind::ShowBusy,
                RemediationKind::AnswerDialog => ErrorActionKind::AnswerDialog,
                RemediationKind::FixPermissions => ErrorActionKind::FixPermissions,
                RemediationKind::Retry => return None,
            }
        };

        Some(ErrorActionVm {
            label: remediation.label.to_string(),
            command: remediation.command.clone(),
            kind,
        })
    }

    fn run_error_action(&mut self, action: ErrorActionVm) -> Task<Message> {
        match action.kind {
            ErrorActionKind::Retry(target) => self.retry(&target),
            ErrorActionKind::SignIn => self.handle(Message::SignIn),
            ErrorActionKind::OpenSettings => self.handle(Message::Navigate(Page::Settings)),
            ErrorActionKind::PickVersion => self.handle(Message::Navigate(Page::Editors)),
            ErrorActionKind::OpenEditor => self.handle(Message::Navigate(Page::Projects)),
            ErrorActionKind::EnterToken => {
                self.notice = Some(Notice::info(
                    "This build has no token field. Set UNITY_GITHUB_TOKEN or UNITY_GITLAB_TOKEN, or pass --git-token-from-stdin where the command supports it.",
                ));
                self.handle(Message::Navigate(Page::Settings))
            }
            ErrorActionKind::InstallGit => {
                self.notice = Some(Notice::info(
                    "Install Git for your platform and make sure `git` is on PATH, then retry. The app never installs system packages on its own.",
                ));
                Task::none()
            }
            ErrorActionKind::ShowCommand => match action.command {
                Some(command) => {
                    self.notice = Some(Notice::success("Command copied to the clipboard."));
                    iced::clipboard::write(command)
                }
                None => {
                    self.notice = Some(Notice::error(
                        "The CLI rejected the arguments. Check the CLI path in Settings and the command's help before retrying.",
                    ));
                    Task::none()
                }
            },
            ErrorActionKind::Confirm => {
                self.notice = Some(Notice::info(
                    "This remote deletion needs an explicit confirmation. Review the command and confirm it yourself; the app never adds `--yes` for you here.",
                ));
                Task::none()
            }
            ErrorActionKind::ShowBusy => self.handle(Message::Navigate(Page::Overview)),
            ErrorActionKind::AnswerDialog => self.handle(Message::Navigate(Page::Overview)),
            ErrorActionKind::FixPermissions => {
                let dir = self
                    .config_dir
                    .clone()
                    .unwrap_or_else(|| "the CLI's configuration directory".to_string());
                self.notice = Some(Notice::info(format!(
                    "Fix write permissions on {dir}, then retry."
                )));
                Task::none()
            }
        }
    }

    /// Re-runs the exact read a failed error came from. A missing target means
    /// the action is stale (the read already succeeded), so it does nothing.
    fn retry(&mut self, target: &str) -> Task<Message> {
        let Some(request) = self.retry_targets.get(target).cloned() else {
            self.notice = Some(Notice::info(
                "That read already succeeded; refresh to try again.",
            ));
            return Task::none();
        };

        match request {
            RetryRequest::Read(ReadKind::Status) => self.refresh_status(),
            RetryRequest::Read(ReadKind::Update) => {
                self.update.begin();
                load_update(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::Auth) => {
                self.auth.begin();
                load_auth(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::License) => {
                self.license.begin();
                load_license(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::Running) => {
                self.running.begin();
                load_running(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::Editors) => {
                self.editors.begin();
                load_editors(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::Releases) => {
                self.releases.begin();
                load_releases(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::Projects) => {
                self.projects.begin();
                load_projects(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::Changelog) => {
                self.changelog.begin();
                Task::perform(
                    load_changelog_future(self.cli.clone(), self.changelog_target.clone()),
                    Message::ChangelogLoaded,
                )
            }
            RetryRequest::Read(ReadKind::RawChangelog) => {
                self.raw_changelog.begin();
                let cli = self.cli.clone();
                let target = self.changelog_target.clone();
                Task::perform(
                    run_blocking(move || cli.changelog(target.as_deref()).map(|raw| raw.changelog)),
                    Message::RawChangelogLoaded,
                )
            }
            RetryRequest::Read(ReadKind::Doctor) => {
                self.doctor.begin();
                load_doctor(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::Environment) => {
                self.environment.begin();
                load_environment(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::DiagnoseProxy) => {
                self.diagnose_proxy.begin();
                load_diagnose(self.cli.clone(), CliDiagnose::Proxy)
            }
            RetryRequest::Read(ReadKind::DiagnoseAccelerator) => {
                self.diagnose_accelerator.begin();
                load_diagnose(self.cli.clone(), CliDiagnose::Accelerator)
            }
            RetryRequest::Read(ReadKind::DiagnoseUpdate) => {
                self.diagnose_update.begin();
                load_diagnose(self.cli.clone(), CliDiagnose::Update)
            }
            RetryRequest::Read(ReadKind::SkillClients) => {
                self.skill_clients.begin();
                load_skill_clients(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::SkillInstalls) => {
                self.skill_installs.begin();
                load_skill_installs(self.cli.clone())
            }
            RetryRequest::Read(ReadKind::Prune) => {
                self.prune.begin();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.editors_prune()
                    }),
                    Message::PruneLoaded,
                )
            }
            RetryRequest::Read(ReadKind::EditorVerify) => {
                // Parameterised verifies use the variant below, which carries
                // the build the user saw fail.
                Task::none()
            }
            RetryRequest::EditorVerify {
                version,
                architecture,
            } => self.load_verify(version, architecture),
            RetryRequest::CliChangelog { target, channel } => {
                self.load_cli_changelog_with(target, channel)
            }
            RetryRequest::CliDryRun { target, channel } => {
                self.preview_cli_update_with(target, channel)
            }
            RetryRequest::Read(ReadKind::Modules)
            | RetryRequest::Read(ReadKind::DialogModules)
            | RetryRequest::Read(ReadKind::CliChangelog)
            | RetryRequest::Read(ReadKind::CliDryRun)
            | RetryRequest::Read(ReadKind::PreviewInstall) => {
                // Parameterised reads always use the variants below, which
                // carry the request they were started with.
                Task::none()
            }
            RetryRequest::Modules {
                version,
                architecture,
            } => {
                self.modules.begin();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version, architecture.as_deref())
                    }),
                    Message::ModulesLoaded,
                )
            }
            RetryRequest::DialogModules {
                version,
                architecture,
            } => {
                self.dialog_modules.begin();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version, Some(&architecture))
                    }),
                    Message::DialogModulesLoaded,
                )
            }
            RetryRequest::PreviewInstall {
                version,
                architecture,
                modules,
                headless,
            } => {
                self.preview_loading = true;
                self.preview_error = None;
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || {
                            cli.dry_run_install(
                                &version,
                                architecture.as_deref(),
                                &modules,
                                headless,
                            )
                        }
                    }),
                    Message::PreviewLoaded,
                )
            }
        }
    }

    fn job_running(&self) -> bool {
        self.active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false)
    }

    fn start_job(&mut self, kind: &str, spec: CommandSpec) -> bool {
        if self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false)
        {
            self.notifications.push_event(
                NotificationLevel::Danger,
                "Job",
                "Another job is already running.",
            );
            return false;
        }

        match self.jobs.start(kind, spec) {
            Ok(started) => {
                self.notifications.push_event(
                    NotificationLevel::Info,
                    "Job",
                    format!("Started: {}", started.command),
                );
                self.active_job = Some(ActiveJob::new(started));
                true
            }
            Err(error) => {
                self.notifications.push_error(&error);
                false
            }
        }
    }

    fn poll_jobs(&mut self) -> Task<Message> {
        let events = self.jobs.poll();
        let mut finished: Option<(String, JobState, Option<UnityError>, Option<Value>)> = None;

        for event in events {
            let Some(job) = self.active_job.as_mut() else {
                continue;
            };
            if job.id != event.job_id {
                continue;
            }

            // Progress-only NDJSON lines are already reflected in the progress
            // bar. Keeping them in the scrollback multiplies widget and string
            // churn during installs for no new information.
            let progress_only =
                event.progress.is_some() && event.message.is_none() && event.result.is_none();

            if let Some(line) = event.line {
                if !progress_only {
                    let text = if line.chars().count() > MAX_LOG_CHARS {
                        line.chars().take(MAX_LOG_CHARS).collect::<String>() + "…"
                    } else {
                        line
                    };
                    job.push_log(event.stream.unwrap_or(StreamKind::Stdout), text);
                }
            }
            if let Some(progress) =
                aggregate_progress(&mut job.downloads, &event.downloads, event.progress)
            {
                // 1.1.0 downloads several files at once, so a single frame's
                // scalar is not the whole install. Keep the high-water mark so
                // the bar never moves backwards between files.
                job.progress = job.progress.max(progress);
            }
            if let Some(message) = event.message {
                job.message = Some(message);
            }
            if event.state != JobState::Running {
                job.state = event.state;
                job.error = event.error.clone();
                job.result = event.result.clone();
                if event.state == JobState::Succeeded {
                    job.progress = 100.0;
                }
                finished = Some((job.kind.clone(), event.state, event.error, event.result));
            }
        }

        let Some((kind, state, error, result)) = finished else {
            return Task::none();
        };

        self.handle_finished_job(kind, state, error, result)
    }

    /// Applies the outcome of one finished streamed job.
    ///
    /// Kept separate from polling so tests can drive the same state changes
    /// without spawning a child process.
    fn handle_finished_job(
        &mut self,
        kind: String,
        state: JobState,
        error: Option<UnityError>,
        result: Option<Value>,
    ) -> Task<Message> {
        match state {
            JobState::Succeeded => {
                self.notifications.push_event(
                    NotificationLevel::Success,
                    "Job",
                    "Job finished successfully.",
                );
            }
            JobState::Failed => match &error {
                Some(error) => {
                    self.notifications.push_error(error);
                }
                None => {
                    self.notifications.push_event(
                        NotificationLevel::Danger,
                        "Job",
                        "The job failed.",
                    );
                }
            },
            JobState::Cancelled => {
                self.notifications
                    .push_event(NotificationLevel::Info, "Job", "Job cancelled.");
            }
            JobState::Running => {}
        }

        if kind == "cli-rollback" {
            if state == JobState::Succeeded {
                self.modal = Modal::None;
                self.rollback_confirmed = false;
                self.rollback_error = None;
            } else if let Some(error) = &error {
                // `BAD_ARGS` is a normal, actionable failure here: the dialog
                // stays open and the shared error rendering names the code.
                self.rollback_error = Some(error.clone());
                self.rollback_confirmed = false;
            }
        }

        if kind == "editor-install" {
            self.note_editor_install_outcome(&kind, state, error.as_ref(), result.as_ref());
        }

        if kind == "skill-install" || kind == "skill-refresh" {
            self.note_skill_outcome(&kind, state, result.as_ref());
        }

        let task = match kind.as_str() {
            "cli-install" | "cli-update" | "cli-rollback" => {
                Task::batch(vec![self.refresh_status(), self.load_overview_data(true)])
            }
            "editor-install" | "module-install" | "editor-default" => self.load_editors_page(true),
            "skill-install" | "skill-refresh" => self.load_skills_page(true),
            "editor-prune" | "editor-prune-missing" => {
                if let Some(error) = &error {
                    self.prune_error = Some(error.clone());
                    if kind == "editor-prune" && prune_force_available_for(error) {
                        // The running-Editor check could not complete. Offer
                        // `--force` only now, and start from an off toggle.
                        self.prune_force_available = true;
                        self.prune_force = false;
                    }
                }
                if state == JobState::Succeeded {
                    self.modal = Modal::None;
                }
                Task::batch(vec![self.load_editors_page(true), self.scan_prune()])
            }
            "auth-login" => {
                let mut tasks = Vec::new();
                if !self.auth.loading {
                    self.auth.begin();
                    tasks.push(load_auth(self.cli.clone()));
                }
                if !self.license.loading {
                    self.license.begin();
                    tasks.push(load_license(self.cli.clone()));
                }
                Task::batch(tasks)
            }
            "project-upgrade" => self.load_projects_page(true),
            "template-create" | "template-pack" => {
                let result = self.active_job.as_ref().and_then(|job| job.result.clone());
                let path = result.as_ref().and_then(result_path);
                if let Modal::TemplateAuthoring(state) = &mut self.modal {
                    state.result = Some(match path {
                        Some(path) => path,
                        None if state.mode.is_pack() => state.form.output.clone(),
                        None => "Completed. The template archive was written.".to_string(),
                    });
                }
                if kind == "template-create" {
                    self.load_templates(true)
                } else {
                    Task::none()
                }
            }
            "project-create" => {
                let result = self.active_job.as_ref().and_then(|job| job.result.clone());
                let created = result
                    .as_ref()
                    .and_then(result_payload::<CreateProjectResult>);
                if let Modal::NewProject(state) = &mut self.modal {
                    state.result = created.clone();
                    state.step = NewProjectStep::Result;
                    if let Some(error) = &error {
                        if let Some(offered) = template_offers(error) {
                            state.issues.push(format!(
                                "Templates offered by the CLI: {}",
                                offered.join(", ")
                            ));
                        }
                    }
                }
                let failed = created
                    .as_ref()
                    .map(|created| created.failed_steps().len())
                    .unwrap_or(0);
                if failed == 0 {
                    self.load_projects_page(true)
                } else {
                    Task::none()
                }
            }
            _ => Task::none(),
        };

        // The helper passes non-remediation work through unchanged.
        self.apply_job_terminal(&kind, state, task)
    }

    /// Applies a finished job to a running remediation notification.
    ///
    /// A successful remediation returns the entry to Idle and refreshes the
    /// page the command may have changed; a failed or cancelled one stays
    /// visible as Failed. Other job kinds pass their follow-up work through
    /// untouched.
    fn apply_job_terminal(
        &mut self,
        kind: &str,
        state: JobState,
        task: Task<Message>,
    ) -> Task<Message> {
        if kind != "remediation" {
            return task;
        }
        let Some(id) = self.remediating.take() else {
            return task;
        };

        match state {
            JobState::Succeeded => {
                self.notifications.set_state(id, ActionState::Idle);
                Task::batch(vec![task, self.refresh_page()])
            }
            JobState::Failed | JobState::Cancelled => {
                self.notifications.set_state(id, ActionState::Failed);
                task
            }
            JobState::Running => task,
        }
    }

    /// Starts the stored remediation command after the user's confirmation.
    /// Nothing here retries or runs on its own.
    fn confirm_notification(&mut self, id: u64) -> Task<Message> {
        let Some(notification) = self.notifications.get(id) else {
            return Task::none();
        };
        if notification.state != ActionState::Confirming {
            return Task::none();
        }
        let Some(command) = notification.command.clone() else {
            return Task::none();
        };
        let Some(remaining) = remediation_args(&command) else {
            self.notifications.set_state(id, ActionState::Failed);
            return Task::none();
        };

        if self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false)
        {
            self.notifications.set_state(id, ActionState::Idle);
            self.notifications.push_event(
                NotificationLevel::Danger,
                "Job",
                "Another job is already running.",
            );
            return Task::none();
        }

        match self.cli.spec(&remaining, "ndjson") {
            Ok(spec) => {
                if self.start_job("remediation", spec) {
                    self.remediating = Some(id);
                    self.notifications.set_state(id, ActionState::Running);
                } else {
                    self.notifications.set_state(id, ActionState::Failed);
                }
            }
            Err(error) => {
                self.notifications.push_error(&error);
                self.notifications.set_state(id, ActionState::Failed);
            }
        }

        Task::none()
    }

    /// Starts missing Overview reads. `force` reloads data that is already
    /// present, but never starts a second copy of a read that is in flight.
    /// A read that failed waits for Retry or Refresh.
    fn load_overview_data(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if self.update.should_start(force) {
            self.update.begin();
            tasks.push(load_update(self.cli.clone()));
        }
        if self.auth.should_start(force) {
            self.auth.begin();
            tasks.push(load_auth(self.cli.clone()));
        }
        if self.license.should_start(force) {
            self.license.begin();
            tasks.push(load_license(self.cli.clone()));
        }
        if self.running.should_start(force) {
            self.running.begin();
            tasks.push(load_running(self.cli.clone()));
        }
        // The summary row counts installed Editors and registered projects.
        if self.editors.should_start(force) {
            self.editors.begin();
            tasks.push(load_editors(self.cli.clone()));
        }
        if self.projects.should_start(force) {
            self.projects.begin();
            tasks.push(load_projects(self.cli.clone()));
        }

        Task::batch(tasks)
    }

    fn load_editors_page(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if self.editors.should_start(force) {
            self.editors.begin();
            tasks.push(load_editors(self.cli.clone()));
        }
        if self.releases.should_start(force) {
            self.releases.begin();
            tasks.push(load_releases(self.cli.clone()));
        }

        Task::batch(tasks)
    }

    fn load_projects_page(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if self.projects.should_start(force) {
            self.projects.begin();
            tasks.push(load_projects(self.cli.clone()));
        }
        if self.editors.should_start(force) {
            self.editors.begin();
            tasks.push(load_editors(self.cli.clone()));
        }
        if self.running.should_start(force) {
            self.running.begin();
            tasks.push(load_running(self.cli.clone()));
        }

        Task::batch(tasks)
    }

    fn load_changelog_page(&mut self, force: bool) -> Task<Message> {
        if !force && (self.changelog.loading || self.changelog_loaded) {
            return Task::none();
        }

        self.changelog_loaded = true;
        self.changelog.begin();
        Task::perform(
            load_changelog_future(self.cli.clone(), self.changelog_target.clone()),
            Message::ChangelogLoaded,
        )
    }

    /// Starts the automatic Diagnostics reads. `unity doctor` is deliberately
    /// excluded: its credential probe can trigger a polkit password prompt, so
    /// it runs only through the explicit `LoadDoctor` action or a Retry.
    fn load_diagnostics_page(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if self.environment.should_start(force) {
            self.environment.begin();
            tasks.push(load_environment(self.cli.clone()));
        }
        if self.diagnose_proxy.should_start(force) {
            self.diagnose_proxy.begin();
            tasks.push(load_diagnose(self.cli.clone(), CliDiagnose::Proxy));
        }
        if self.diagnose_accelerator.should_start(force) {
            self.diagnose_accelerator.begin();
            tasks.push(load_diagnose(self.cli.clone(), CliDiagnose::Accelerator));
        }
        if self.diagnose_update.should_start(force) {
            self.diagnose_update.begin();
            tasks.push(load_diagnose(self.cli.clone(), CliDiagnose::Update));
        }

        Task::batch(tasks)
    }

    /// Runs `unity doctor` only on an explicit request. Its credential probe
    /// can trigger polkit, so a page navigation or Refresh must not start it.
    fn load_doctor_now(&mut self) -> Task<Message> {
        if self.doctor.loading {
            return Task::none();
        }
        self.doctor.begin();
        load_doctor(self.cli.clone())
    }

    /// Starts the two Skills reads. They are independent so a failed client
    /// list never hides the tracked installs (and the reverse).
    fn load_skills_page(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if self.skill_clients.should_start(force) {
            self.skill_clients.begin();
            tasks.push(load_skill_clients(self.cli.clone()));
        }
        if self.skill_installs.should_start(force) {
            self.skill_installs.begin();
            tasks.push(load_skill_installs(self.cli.clone()));
        }

        Task::batch(tasks)
    }

    /// Runs `unity skill install` as a streamed job.
    ///
    /// `--local` is checked again here: the controller never trusts a form flag
    /// that has no project context, even if the UI emits it.
    fn start_skill_install(&mut self, client: Option<String>, all: bool) -> Task<Message> {
        if self.job_running() {
            self.notice = Some(Notice::error("Another job is already running."));
            return Task::none();
        }
        if self.skill_install_local && self.project_context.is_none() {
            self.notice = Some(Notice::error(
                "`--local` needs a Unity project context. Launch the Hub from a project folder, then try again.",
            ));
            return Task::none();
        }

        let options = SkillInstallOptions {
            client: client.clone(),
            all,
            local: self.skill_install_local,
            skill: trimmed_option(&self.skill_install_skill),
            yes: self.skill_install_overwrite,
            dry_run: self.skill_install_dry_run,
            force: false,
        };
        let spec = match self.cli.skill_install_spec(&options) {
            Ok(spec) => spec,
            Err(error) => {
                self.notice = Some(Notice::error(error_text(&error)));
                return Task::none();
            }
        };

        self.skill_last_run = Some(SkillRun {
            scope: SkillScope::Install {
                client,
                all,
                local: options.local,
                skill: options.skill.clone(),
                yes: options.yes,
            },
            dry_run: options.dry_run,
            force: false,
            project_context: self.project_context.clone(),
        });
        self.skill_result = None;
        let _ = self.start_job("skill-install", spec);
        Task::none()
    }

    /// Runs `unity skill refresh` as a streamed job.
    ///
    /// `--yes` is always present: the click is the confirmation and a GUI child
    /// has no terminal. `--dry-run` is the caller's decision; the page's first
    /// refresh click always sets it.
    fn start_skill_refresh(&mut self, dry_run: bool) -> Task<Message> {
        if self.job_running() {
            self.notice = Some(Notice::error("Another job is already running."));
            return Task::none();
        }

        let options = SkillRefreshOptions {
            yes: true,
            dry_run,
            force: false,
        };
        let spec = match self.cli.skill_refresh_spec(&options) {
            Ok(spec) => spec,
            Err(error) => {
                self.notice = Some(Notice::error(error_text(&error)));
                return Task::none();
            }
        };

        self.skill_last_run = Some(SkillRun {
            scope: SkillScope::Refresh { yes: true },
            dry_run,
            force: false,
            project_context: self.project_context.clone(),
        });
        self.skill_result = None;
        let _ = self.start_job("skill-refresh", spec);
        Task::none()
    }

    /// `--force` is offered only for a run whose current result has a
    /// `conflict` row. Both the view model and the action handler use this
    /// predicate, so a stale or replayed action cannot start an unexplained
    /// overwrite of a copy the CLI did not write.
    fn skill_force_available(&self) -> bool {
        self.skill_result
            .as_ref()
            .is_some_and(|result| result.rows.iter().any(|row| row.outcome().is_conflict()))
    }

    /// Repeats the last skill run with `--force`. Only reachable from the
    /// `conflict` offer, which carries the explanation first; the handler also
    /// refuses when the current result no longer has a conflict row.
    fn force_skill_last_run(&mut self) -> Task<Message> {
        let Some(run) = self.skill_last_run.clone() else {
            self.notice = Some(Notice::info(
                "Run an install or refresh first; there is nothing to force.",
            ));
            return Task::none();
        };
        if !self.skill_force_available() {
            self.notice = Some(Notice::error(
                "`--force` is only offered when the latest skill run reported a `conflict` row. Run an install or refresh again and review the rows first.",
            ));
            return Task::none();
        }
        if self.job_running() {
            self.notice = Some(Notice::error("Another job is already running."));
            return Task::none();
        }
        if run.scope_local() && run.project_context.is_none() {
            self.notice = Some(Notice::error(
                "`--local` needs a Unity project context. Launch the Hub from a project folder, then try again.",
            ));
            return Task::none();
        }

        let (kind, spec, scope) = match &run.scope {
            SkillScope::Install {
                client,
                all,
                local,
                skill,
                yes,
            } => {
                let options = SkillInstallOptions {
                    client: client.clone(),
                    all: *all,
                    local: *local,
                    skill: skill.clone(),
                    yes: *yes,
                    dry_run: false,
                    force: true,
                };
                (
                    "skill-install",
                    self.cli.skill_install_spec(&options),
                    run.scope.clone(),
                )
            }
            SkillScope::Refresh { yes } => (
                "skill-refresh",
                self.cli.skill_refresh_spec(&SkillRefreshOptions {
                    yes: *yes,
                    dry_run: false,
                    force: true,
                }),
                run.scope.clone(),
            ),
        };
        let spec = match spec {
            Ok(spec) => spec,
            Err(error) => {
                self.notice = Some(Notice::error(error_text(&error)));
                return Task::none();
            }
        };

        self.skill_last_run = Some(SkillRun {
            scope,
            dry_run: false,
            force: true,
            project_context: run.project_context,
        });
        self.skill_result = None;
        let _ = self.start_job(kind, spec);
        Task::none()
    }

    /// Stores a finished skill job's terminal rows.
    ///
    /// The starters clear the previous rows before they run, so a failed job
    /// leaves no outcome card to mistake for the current run; the job card owns
    /// the error. A successful job stores exactly the rows the CLI returned.
    fn note_skill_outcome(&mut self, kind: &str, state: JobState, result: Option<&Value>) {
        if state != JobState::Succeeded {
            return;
        }
        let dry_run = self
            .skill_last_run
            .as_ref()
            .map(|run| run.dry_run)
            .unwrap_or(false);
        let rows = result
            .map(|value| skill_rows_from_result(value, self.skill_last_run.as_ref()))
            .unwrap_or_default();
        self.skill_result = Some(SkillResult { rows, dry_run });
        if kind == "skill-refresh" && dry_run {
            // The preview landed: the next refresh click applies for real.
            self.skill_refresh_previewed = true;
        }
    }

    fn skills_vm(&self) -> SkillsVm {
        let job_running = self.job_running();
        let local_available = self.project_context.is_some();
        let local_disabled_reason = (!local_available).then(|| {
            "`--local` writes into the current project. The Hub was not launched from a Unity project, so it has no project context to write into.".to_string()
        });
        let skill_input_error =
            unity_core::versions::validate_skill_name(&self.skill_install_skill)
                .err()
                .map(|error| error.message);
        let all_options = self.skill_install_options(None, true);
        let all_command = self.command_preview(
            self.cli.skill_install_spec(&all_options),
            "unity skill install --all",
        );

        let next_dry_run = self.next_skill_refresh_dry_run();
        let refresh_command = self.command_preview(
            self.cli.skill_refresh_spec(&SkillRefreshOptions {
                yes: true,
                dry_run: next_dry_run,
                force: false,
            }),
            "unity skill refresh",
        );

        SkillsVm {
            clients: self.load_state(ReadKind::SkillClients, &self.skill_clients, |clients| {
                clients.iter().map(skill_client_vm).collect()
            }),
            installs: self.load_state(ReadKind::SkillInstalls, &self.skill_installs, |installs| {
                installs
                    .iter()
                    .map(|install| SkillInstallVm {
                        path: install.path.clone(),
                        client: install.client.clone(),
                        skill: install.skill.clone(),
                        status: install.status.clone(),
                    })
                    .collect()
            }),
            install: SkillInstallOptionsVm {
                local: self.skill_install_local && local_available,
                overwrite: self.skill_install_overwrite,
                dry_run: self.skill_install_dry_run,
                skill_input: self.skill_install_skill.clone(),
                skill_input_error,
                all_command,
            },
            result: self
                .skill_result
                .as_ref()
                .map(|result| self.skill_run_vm(result)),
            refresh_label: if next_dry_run {
                "Preview refresh (dry run)".to_string()
            } else {
                "Refresh now".to_string()
            },
            refresh_previewed: self.skill_refresh_previewed,
            refresh_hint: if next_dry_run {
                "The first refresh is a dry run: it changes nothing and labels every row would-*."
                    .to_string()
            } else {
                "The next refresh applies for real and reports one row per tracked install."
                    .to_string()
            },
            refresh_command,
            project_context: self.project_context.clone(),
            local_available,
            local_disabled_reason,
            job_running,
        }
    }

    /// The first refresh click after opening the page is a dry run; once a
    /// preview has landed, the next click applies.
    fn next_skill_refresh_dry_run(&self) -> bool {
        !self.skill_refresh_previewed
    }

    fn skill_install_options(&self, client: Option<String>, all: bool) -> SkillInstallOptions {
        SkillInstallOptions {
            client,
            all,
            local: self.skill_install_local,
            skill: trimmed_option(&self.skill_install_skill),
            yes: self.skill_install_overwrite,
            dry_run: self.skill_install_dry_run,
            force: false,
        }
    }

    fn skill_run_vm(&self, result: &SkillResult) -> SkillRunVm {
        let rows: Vec<SkillOutcomeRowVm> = result.rows.iter().map(skill_outcome_row_vm).collect();
        let force = self.skill_force_available().then(|| self.skill_force_vm());
        SkillRunVm {
            dry_run: result.dry_run,
            rows,
            force,
        }
    }

    /// The `--force` offer, built from the exact request the last job ran.
    ///
    /// The explanation is part of the offer: the button never appears without
    /// saying that the CLI did not write the conflicting copy.
    fn skill_force_vm(&self) -> SkillForceVm {
        let Some(run) = &self.skill_last_run else {
            return SkillForceVm {
                explanation: "The Unity CLI did not write that copy, so it left it alone. `--force` takes ownership of it and replaces it with the CLI's own. Only run it if you know who wrote it.".to_string(),
                command: String::new(),
                scope: "conflict row".to_string(),
            };
        };

        let (scope, command) = match &run.scope {
            SkillScope::Install {
                client,
                all,
                local,
                skill,
                yes,
            } => {
                let options = SkillInstallOptions {
                    client: client.clone(),
                    all: *all,
                    local: *local,
                    skill: skill.clone(),
                    yes: *yes,
                    dry_run: false,
                    force: true,
                };
                let scope = match client {
                    Some(client) => format!("`{client}`"),
                    None if *all => "`--all` install".to_string(),
                    None => "install".to_string(),
                };
                let command = self.command_preview(
                    self.cli.skill_install_spec(&options),
                    "unity skill install --force",
                );
                (scope, command)
            }
            SkillScope::Refresh { yes } => {
                let command = self.command_preview(
                    self.cli.skill_refresh_spec(&SkillRefreshOptions {
                        yes: *yes,
                        dry_run: false,
                        force: true,
                    }),
                    "unity skill refresh --force",
                );
                ("refresh".to_string(), command)
            }
        };

        SkillForceVm {
            explanation: format!(
                "The Unity CLI did not write the {scope} copy, so it left it alone rather than overwrite it. `--force` takes ownership of that copy and replaces it with the CLI's own. Only run it if you know who wrote it."
            ),
            command,
            scope,
        }
    }

    /// Refreshes the running-Editor list unless a read is already in flight.
    fn refresh_running_editors(&mut self) -> Task<Message> {
        if self.running.loading {
            return Task::none();
        }
        self.running.begin();
        load_running(self.cli.clone())
    }

    fn refresh_status(&mut self) -> Task<Message> {
        if self.cli_status.loading {
            return Task::none();
        }
        self.cli_status.begin();
        load_status(self.cli.clone())
    }

    fn ensure_page(&mut self, page: Page) -> Task<Message> {
        match page {
            Page::Overview => self.load_overview_data(false),
            Page::Editors => self.load_editors_page(false),
            Page::Projects => self.load_projects_page(false),
            Page::Templates => self.load_templates(false),
            Page::Processes => self.load_processes(false),
            Page::Skills => {
                // Entering the page starts a fresh preview cycle, so the
                // documented first-click dry run holds after navigation.
                self.skill_refresh_previewed = false;
                self.load_skills_page(false)
            }
            Page::Diagnostics => self.load_diagnostics_page(false),
            Page::Changelog => self.load_changelog_page(false),
            _ => Task::none(),
        }
    }

    fn refresh_page(&mut self) -> Task<Message> {
        match self.page {
            Page::Overview => {
                Task::batch(vec![self.refresh_status(), self.load_overview_data(true)])
            }
            Page::Editors => self.load_editors_page(true),
            Page::Projects => self.load_projects_page(true),
            Page::Templates => self.load_templates(true),
            Page::Processes => self.load_processes(true),
            Page::Skills => self.load_skills_page(true),
            Page::Diagnostics => self.load_diagnostics_page(true),
            Page::Changelog => self.load_changelog_page(true),
            Page::Settings => self.refresh_status(),
        }
    }

    /// Builds the presentation snapshot consumed by the UI crate. The UI never
    /// sees controller state, CLI types, or errors.
    pub fn view_model(&self) -> ViewModel {
        ViewModel {
            shell: self.shell_vm(),
            page: match self.page {
                Page::Overview => PageVm::Overview(self.overview_vm()),
                Page::Editors => PageVm::Editors(self.editors_vm()),
                Page::Projects => PageVm::Projects(self.projects_vm()),
                Page::Templates => PageVm::Templates(self.templates_vm()),
                Page::Processes => PageVm::Processes(self.processes_vm()),
                Page::Skills => PageVm::Skills(self.skills_vm()),
                Page::Diagnostics => PageVm::Diagnostics(self.diagnostics_vm()),
                Page::Changelog => PageVm::Changelog(self.changelog_vm()),
                Page::Settings => PageVm::Settings(self.settings_vm()),
            },
            modal: self.modal_vm(),
            text_scale: self.text_scale(),
        }
    }

    fn shell_vm(&self) -> ShellVm {
        let cli = match &self.cli_status.data {
            Some(status) if status.installed => CliConnectionVm::Connected {
                version: status
                    .version
                    .as_ref()
                    .map(|version| version.version.clone()),
            },
            Some(_) => CliConnectionVm::Missing,
            None if self.cli_status.loading => CliConnectionVm::Checking,
            None => CliConnectionVm::Missing,
        };

        ShellVm {
            page: self.page,
            cli,
            job: self.active_job.as_ref().map(|job| self.job_vm(job)),
            notifications: self.notifications.snapshot(),
            unread: self.notifications.unread(),
            rail_open: self.notifications.rail_open(),
            // A notice is a one-shot event (a copy confirmation, a settings
            // error) that never enters the rail; it borrows the toast slot
            // with id 0, which no stored entry can ever carry.
            toast: self
                .notifications
                .toast()
                .map(|toast| ToastVm {
                    id: toast.id,
                    tone: notification_tone(toast.level),
                    message: toast.message.clone(),
                })
                .or_else(|| {
                    self.notice.as_ref().map(|notice| ToastVm {
                        id: 0,
                        tone: notice_tone(notice.kind),
                        message: notice.text.clone(),
                    })
                }),
        }
    }

    /// The CLI version dismissals are scoped to. Before the status read lands
    /// the key is `unknown`; the status read is the first boot read, so no
    /// advisory can be dismissed against the placeholder in practice.
    fn cli_version_key(&self) -> String {
        self.cli_status
            .data
            .as_ref()
            .and_then(|status| status.version.as_ref())
            .map(|version| version.version.clone())
            .unwrap_or_else(|| UNKNOWN_CLI_VERSION.to_string())
    }

    /// Advisories describe the CLI that reported them: a version change
    /// forgets what this session already announced, so a new CLI shows its
    /// advisories once more. The rail keeps its history; the entries of the
    /// old CLI stay dismissable there.
    fn on_cli_version_resolved(&mut self, previous_version: &str) {
        let version = self.cli_version_key();
        if version == previous_version {
            return;
        }
        self.notifications.forget_advisories();
    }

    /// Merges every advisory a read or job reported since the last update.
    /// `CliAdvisory::identity` collapses repeated frames into one entry, and
    /// the notification store deduplicates on it, so a refresh that repeats
    /// the same warning never raises the unread badge or toast again.
    fn absorb_advisories(&mut self) {
        let mut incoming = self.cli.take_advisories();
        incoming.append(&mut self.jobs.take_advisories());

        for advisory in &incoming {
            self.notifications.push_advisory(advisory);
        }
    }

    /// Moves the update flow to a channel and invalidates everything derived
    /// from the old channel: the target release notes and the dry-run preview.
    fn apply_update_channel(&mut self, channel: String) {
        self.update_channel = channel;
        self.update_changelog = Loadable::default();
        self.update_dry_run = Loadable::default();
    }

    fn cli_update_flow_vm(&self) -> CliUpdateFlowVm {
        let target = trimmed_option(&self.update_target);
        let channel = trimmed_option(&self.update_channel);
        let changelog = self.load_state(ReadKind::CliChangelog, &self.update_changelog, |raw| {
            CliChangelogVm {
                version: raw.version.clone(),
                text: raw.changelog.trim().to_string(),
            }
        });
        let dry_run = self.load_state(ReadKind::CliDryRun, &self.update_dry_run, |text| {
            text.clone()
        });
        let can_apply = self.cli_update_can_apply();
        let apply_blocked_reason = if can_apply {
            None
        } else if self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false)
        {
            Some("A job is already running.".to_string())
        } else if target.is_some()
            && unity_core::versions::validate_cli_version(target.as_deref().unwrap_or_default())
                .is_err()
        {
            Some("Enter `latest` or a version such as 1.0.0-beta.18.".to_string())
        } else {
            Some("Load the target release notes before applying the update.".to_string())
        };

        CliUpdateFlowVm {
            channel: self.update_channel.clone(),
            channel_options: vec!["stable".to_string(), "beta".to_string()],
            target_input: self.update_target.clone(),
            changelog,
            dry_run,
            changelog_command: self.command_preview(
                self.cli
                    .self_update_changelog_spec(target.as_deref(), channel.as_deref()),
                "unity self-update --changelog",
            ),
            apply_command: self.command_preview(
                self.cli
                    .self_update_apply_spec(target.as_deref(), channel.as_deref()),
                "unity self-update --yes",
            ),
            rollback_command: self.command_preview(
                self.cli
                    .self_update_rollback_spec(trimmed_option(&self.rollback_target).as_deref()),
                "unity self-update --rollback",
            ),
            can_apply,
            apply_blocked_reason,
        }
    }

    /// Applying an update is gated on the target release notes: the app never
    /// replaces the CLI before the user has seen what that target changes.
    fn cli_update_can_apply(&self) -> bool {
        if self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false)
        {
            return false;
        }
        if let Some(target) = trimmed_option(&self.update_target) {
            if unity_core::versions::validate_cli_version(&target).is_err() {
                return false;
            }
        }
        !self.update_changelog.loading
            && self.update_changelog.error.is_none()
            && self.update_changelog.data.is_some()
    }

    fn load_update_changelog(&mut self) -> Task<Message> {
        let target = trimmed_option(&self.update_target);
        let channel = trimmed_option(&self.update_channel);
        self.load_cli_changelog_with(target, channel)
    }

    fn load_cli_changelog_with(
        &mut self,
        target: Option<String>,
        channel: Option<String>,
    ) -> Task<Message> {
        self.update_changelog.begin();
        self.retry_targets.insert(
            ReadKind::CliChangelog.id().to_string(),
            RetryRequest::CliChangelog {
                target: target.clone(),
                channel: channel.clone(),
            },
        );
        Task::perform(
            run_blocking({
                let cli = self.cli.clone();
                move || cli.self_update_changelog(target.as_deref(), channel.as_deref())
            }),
            Message::CliChangelogLoaded,
        )
    }

    fn preview_cli_update(&mut self) -> Task<Message> {
        let target = trimmed_option(&self.update_target);
        let channel = trimmed_option(&self.update_channel);
        self.preview_cli_update_with(target, channel)
    }

    fn preview_cli_update_with(
        &mut self,
        target: Option<String>,
        channel: Option<String>,
    ) -> Task<Message> {
        self.update_dry_run.begin();
        self.retry_targets.insert(
            ReadKind::CliDryRun.id().to_string(),
            RetryRequest::CliDryRun {
                target: target.clone(),
                channel: channel.clone(),
            },
        );
        Task::perform(
            run_blocking({
                let cli = self.cli.clone();
                move || cli.self_update_dry_run(target.as_deref(), channel.as_deref())
            }),
            Message::CliDryRunLoaded,
        )
    }

    fn rollback_cli_vm(&self) -> RollbackCliVm {
        let version = trimmed_option(&self.rollback_target);
        let restore_label = match &version {
            Some(version) => format!("Unity CLI {version}"),
            None => "the most recent kept CLI version".to_string(),
        };
        let any_job_running = self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false);
        let command = self.command_preview(
            self.cli.self_update_rollback_spec(version.as_deref()),
            "unity self-update --rollback",
        );

        RollbackCliVm {
            version_input: self.rollback_target.clone(),
            restore_label,
            confirmed: self.rollback_confirmed,
            can_rollback: self.rollback_confirmed && !any_job_running,
            command,
            error: self.rollback_error.as_ref().map(|error| {
                self.ui_error(
                    error,
                    ErrorContext::Job {
                        kind: "cli-rollback".to_string(),
                    },
                )
            }),
            job_running: any_job_running,
        }
    }

    fn overview_vm(&self) -> OverviewVm {
        OverviewVm {
            cli: self.cli_card_vm(),
            update: self.load_state(ReadKind::Update, &self.update, |update| UpdateVm {
                available: update.update_available,
                current_version: update.current_version.clone(),
                latest_version: update.latest_version.clone(),
            }),
            update_flow: self.cli_update_flow_vm(),
            account: AccountCardVm {
                auth: self.auth_vm(),
                license: self.license_vm(),
            },
            running: self.load_state(ReadKind::Running, &self.running, |running| {
                running
                    .instances
                    .iter()
                    .map(|instance| RunningEditorVm {
                        project_path: instance
                            .project_path
                            .clone()
                            .unwrap_or_else(|| "Unity Editor".to_string()),
                        editor_version: instance
                            .editor_version
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        pid: instance
                            .pid
                            .map(|pid| pid.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                        phase: instance.phase.clone(),
                        pipeline_version: instance.pipeline_version.clone(),
                        update_available: instance.update_available,
                        safe_mode: instance.in_safe_mode(),
                        dialog: instance.dialog_text(),
                    })
                    .collect()
            }),
            editor_count: self.editors.data.as_ref().map(Vec::len),
            project_count: self.projects.data.as_ref().map(Vec::len),
        }
    }

    fn cli_card_vm(&self) -> CliCardVm {
        if self.cli_status.loading && self.cli_status.data.is_none() {
            return CliCardVm::Loading;
        }

        if let Some(status) = &self.cli_status.data {
            if !status.installed {
                return CliCardVm::Missing;
            }

            let version = status.version.as_ref();
            return CliCardVm::Ready {
                details: CliDetailsVm {
                    version: version
                        .map(|version| version.version.clone())
                        .unwrap_or_else(|| "unknown".to_string()),
                    channel: version
                        .map(|version| version.channel.clone())
                        .unwrap_or_else(|| "unknown".to_string()),
                    platform: format!(
                        "{} {}",
                        version
                            .map(|version| version.platform.as_str())
                            .unwrap_or("—"),
                        version.map(|version| version.arch.as_str()).unwrap_or("")
                    ),
                    runtime: version
                        .and_then(|version| version.runtime.clone())
                        .unwrap_or_else(|| "—".to_string()),
                    executable: status.path.clone().unwrap_or_else(|| "—".to_string()),
                    resolved_via: status
                        .path_source
                        .clone()
                        .unwrap_or_else(|| "—".to_string()),
                    install_path: status
                        .install_path
                        .clone()
                        .unwrap_or_else(|| "—".to_string()),
                    build_date: format_iso(
                        version.and_then(|version| version.build_date.as_deref()),
                    ),
                },
                error: self
                    .cli_status
                    .error
                    .as_ref()
                    .map(|error| self.ui_error(error, ErrorContext::Read(ReadKind::Status))),
            };
        }

        match &self.cli_status.error {
            Some(error) => {
                CliCardVm::Failed(self.ui_error(error, ErrorContext::Read(ReadKind::Status)))
            }
            None => CliCardVm::Idle,
        }
    }

    fn auth_vm(&self) -> AuthVm {
        if self.auth.loading && self.auth.data.is_none() {
            return AuthVm::Loading;
        }

        if let Some(auth) = &self.auth.data {
            if auth.logged_in {
                return AuthVm::SignedIn {
                    name: auth
                        .user
                        .as_ref()
                        .map(|user| user.name.clone())
                        .unwrap_or_else(|| "Signed in".to_string()),
                    email: auth
                        .user
                        .as_ref()
                        .map(|user| user.email.clone())
                        .unwrap_or_default(),
                    stale: auth.session_state.as_deref() == Some("stale"),
                };
            }
            return AuthVm::SignedOut {
                expired: auth.session_state.as_deref() == Some(error_code::SESSION_EXPIRED),
            };
        }

        match &self.auth.error {
            Some(error) => AuthVm::Failed(self.ui_error(error, ErrorContext::Read(ReadKind::Auth))),
            None => AuthVm::Idle,
        }
    }

    fn license_vm(&self) -> LicenseVm {
        if let Some(license) = &self.license.data {
            return LicenseVm::Ready {
                active: license.active,
                licenses: license
                    .licenses
                    .iter()
                    .take(4)
                    .map(|license| LicenseRowVm {
                        product: license.product.clone(),
                        kind: license.kind.clone(),
                    })
                    .collect(),
            };
        }

        match &self.license.error {
            Some(error) => {
                LicenseVm::Failed(self.ui_error(error, ErrorContext::Read(ReadKind::License)))
            }
            None => LicenseVm::Idle,
        }
    }

    fn editors_vm(&self) -> EditorsVm {
        let installed = self.installed_builds();

        EditorsVm {
            installed: self.load_state(ReadKind::Editors, &self.editors, |editors| {
                editors.iter().map(editor_row_vm).collect()
            }),
            install: self.install_editor_vm(),
            releases: self.load_state(ReadKind::Releases, &self.releases, |releases| {
                releases
                    .iter()
                    .map(|release| ReleaseRowVm {
                        version: release.version.clone(),
                        architecture: release.architecture.clone(),
                        stream: release.stream.clone(),
                        stream_tone: stream_tone(&release.stream),
                        installed: release.installed
                            || installed_build_matches(
                                &installed,
                                &release.version,
                                &release.architecture,
                            ),
                    })
                    .collect()
            }),
            prune: self.load_state(ReadKind::Prune, &self.prune, prune_report_vm),
        }
    }

    fn install_editor_vm(&self) -> InstallEditorVm {
        let modules = self.load_state(ReadKind::Modules, &self.modules, |modules| {
            modules
                .iter()
                .filter(|module| !module.is_installed)
                .map(|module| ModuleOptionVm {
                    id: module.id.clone(),
                    label: format!(
                        "{}  ·  {}",
                        module.name,
                        format_bytes(module.download_bytes)
                    ),
                    selected: self.selected_modules.contains(&module.id),
                })
                .collect()
        });

        let available_count = self
            .modules
            .data
            .as_ref()
            .map(|modules| modules.iter().filter(|module| !module.is_installed).count())
            .unwrap_or(0);

        let preview = self.preview.as_ref().map(|preview| InstallPreviewVm {
            editor_version: preview
                .editor
                .as_ref()
                .map(|editor| editor.version.clone())
                .unwrap_or_else(|| self.install_version.clone()),
            // The CLI resolves the host default when no build was requested.
            // An older CLI may leave the field empty; then show the requested
            // build, and let the UI label `None` as the host default.
            architecture: preview
                .editor
                .as_ref()
                .map(|editor| editor.architecture.trim().to_string())
                .filter(|architecture| !architecture.is_empty())
                .or_else(|| self.install_architecture.clone()),
            download_size: format_bytes(
                preview.total_download_size.max(
                    preview
                        .editor
                        .as_ref()
                        .map(|editor| editor.download_size)
                        .unwrap_or(0),
                ),
            ),
            module_count: preview.modules.len().max(self.selected_modules.len()),
        });

        let timeout_error = unity_core::versions::validate_timeout_seconds(&self.install_timeout)
            .err()
            .map(|error| error.message);
        let retries_error = unity_core::versions::validate_retry_count(&self.install_retries)
            .err()
            .map(|error| error.message);
        let advanced_valid = timeout_error.is_none() && retries_error.is_none();
        let base_can_install = self.selected_modules.is_empty() || self.accept_eula;

        InstallEditorVm {
            version_input: self.install_version.clone(),
            architecture: self.install_architecture.clone(),
            architecture_options: architecture_options(self.install_architecture.as_deref()),
            modules,
            available_count,
            headless: self.install_headless && unity_core::runner::headless_editor_supported(),
            headless_available: unity_core::runner::headless_editor_supported(),
            accept_eula: self.accept_eula,
            can_install: base_can_install && advanced_valid,
            second_build_warning: self.second_build_warning(),
            preview,
            preview_note: self
                .preview_raw
                .as_ref()
                .map(|raw| format!("Dry-run returned: {raw}")),
            preview_error: self.preview_error.clone(),
            preview_loading: self.preview_loading,
            advanced: InstallAdvancedVm {
                expanded: self.install_advanced_expanded,
                resume: self.install_resume,
                download_only: self.install_download_only,
                no_wait: self.install_no_wait,
                timeout_input: self.install_timeout.clone(),
                set_default: self.install_set_default,
                retries_input: self.install_retries.clone(),
                conflict_note: self.install_advanced_note.clone(),
                timeout_error,
                retries_error,
                can_install: advanced_valid,
            },
            command_preview: self.command_preview(
                self.install_editor_spec(),
                &format!("unity install {}", self.install_version.trim()),
            ),
            resume_offer: self.resume_offer.as_ref().map(|offer| ResumeInstallVm {
                version: offer.request.version.clone(),
                architecture: offer
                    .request
                    .architecture
                    .clone()
                    .unwrap_or_else(|| "host default".to_string()),
                reason: offer.reason.clone(),
                can_resume: self
                    .active_job
                    .as_ref()
                    .map(ActiveJob::is_running)
                    .map(|running| !running)
                    .unwrap_or(true),
            }),
            download_report: self.download_report.clone(),
        }
    }

    /// Explains that adding a build to a version that already has one keeps
    /// both on disk. Single-build users who leave the host default selected
    /// never see this: their command and card stay unchanged.
    fn second_build_warning(&self) -> Option<String> {
        let requested = self.install_architecture.as_deref()?;
        let version = self.install_version.trim();
        let builds: Vec<&str> = self
            .editors
            .data
            .as_ref()?
            .iter()
            .filter(|editor| editor.version == version)
            .map(|editor| editor.architecture.as_str())
            .filter(|architecture| !architecture.trim().is_empty())
            .collect();
        if builds.is_empty() || builds.contains(&requested) {
            return None;
        }
        Some(format!(
            "Unity {version} is already installed for {}. Installing the {requested} build keeps both on disk, and projects pinned to the existing build keep using it.",
            builds.join(" and ")
        ))
    }

    fn installed_builds_for(&self, version: &str) -> Vec<String> {
        self.editors
            .data
            .as_ref()
            .map(|editors| {
                editors
                    .iter()
                    .filter(|editor| editor.version == version)
                    .map(|editor| {
                        if editor.architecture.trim().is_empty() {
                            "host default".to_string()
                        } else {
                            editor.architecture.clone()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Builds the install command from the card's current fields. Kept as one
    /// method so the handler and its tests always agree on the flags.
    fn install_editor_spec(&self) -> Result<CommandSpec, UnityError> {
        let modules: Vec<String> = self.selected_modules.iter().cloned().collect();
        let options = self.install_options()?;
        self.cli.install_editor_spec(
            &self.install_version,
            self.install_architecture.as_deref(),
            &modules,
            self.install_headless,
            self.accept_eula,
            &options,
        )
    }

    /// Validates the Advanced fields and maps them to the adapter options.
    fn install_options(&self) -> Result<InstallOptions, UnityError> {
        Ok(InstallOptions {
            resume: self.install_resume,
            download_only: self.install_download_only,
            no_wait: self.install_no_wait,
            timeout_secs: unity_core::versions::validate_timeout_seconds(&self.install_timeout)?,
            set_default: self.install_set_default,
            retries: unity_core::versions::validate_retry_count(&self.install_retries)?,
        })
    }

    fn current_install_request(&self) -> InstallRequest {
        InstallRequest {
            version: self.install_version.trim().to_string(),
            architecture: self.install_architecture.clone(),
            modules: self.selected_modules.iter().cloned().collect(),
            headless: self.install_headless,
            accept_eula: self.accept_eula,
            options: self.install_options().unwrap_or_default(),
        }
    }

    fn dialog_module_options(&self) -> Result<ModuleInstallOptions, UnityError> {
        Ok(ModuleInstallOptions {
            retries: unity_core::versions::validate_retry_count(&self.dialog_retries)?,
            download_directory: unity_core::versions::validate_download_directory(
                &self.dialog_download_directory,
            )?,
        })
    }

    /// Remembers what a finished Editor install means for the card: a
    /// download-only success names its folder, and an interrupted/failed
    /// download offers Resume with the exact request that started it.
    fn note_editor_install_outcome(
        &mut self,
        kind: &str,
        state: JobState,
        error: Option<&UnityError>,
        result: Option<&Value>,
    ) {
        if kind != "editor-install" {
            return;
        }

        match state {
            JobState::Succeeded => {
                self.resume_offer = None;
                if self
                    .last_install
                    .as_ref()
                    .is_some_and(|request| request.options.download_only)
                {
                    match download_path(result) {
                        Some(path) => {
                            self.download_report = Some(format!(
                                "Files downloaded to {path}. Nothing was installed or registered, so no Editor was added."
                            ));
                            self.notice = Some(Notice::success(format!(
                                "Download-only finished. Files are in {path}; no Editor was installed."
                            )));
                        }
                        None => {
                            self.download_report = Some(
                                "Download-only finished, but the CLI reported no download folder."
                                    .to_string(),
                            );
                        }
                    }
                }
            }
            JobState::Failed | JobState::Cancelled => {
                if let Some(request) = self.last_install.clone() {
                    // A download-only run promised to install nothing; resuming
                    // it as a normal install would break that promise.
                    if request.options.download_only {
                        return;
                    }
                    if let Some(reason) = resume_reason(error, result) {
                        self.resume_offer = Some(ResumeOffer { request, reason });
                    }
                }
            }
            JobState::Running => {}
        }
    }

    /// Builds the command that stores a default Editor.
    ///
    /// A version with two installed builds cannot be stored by version alone:
    /// the CLI would have to pick one. The app explains the choice instead of
    /// sending a command that silently targets either build.
    fn editor_default_spec(
        &self,
        version: &str,
        architecture: &str,
    ) -> Result<CommandSpec, UnityError> {
        let architecture = architecture.trim();
        if architecture.is_empty() {
            let builds = self.installed_builds_for(version);
            if builds.len() > 1 {
                return Err(UnityError::invalid_input(format!(
                    "Unity {version} has {} installed builds ({}). Choose Set default on the build row you want; the app will not guess which one to store.",
                    builds.len(),
                    builds.join(", ")
                )));
            }
        }
        let architecture = (!architecture.is_empty()).then_some(architecture);
        self.cli.editor_default_spec(version, architecture)
    }

    /// Opens the prune dialog and starts the report-only scan.
    fn open_prune(&mut self) -> Task<Message> {
        self.modal = Modal::PruneEditors;
        self.prune_confirmed = false;
        self.prune_missing_confirmed = false;
        self.prune_force = false;
        self.prune_force_available = false;
        self.prune_error = None;
        self.scan_prune()
    }

    /// Re-runs the report-only scan. It never carries a removal flag.
    fn scan_prune(&mut self) -> Task<Message> {
        if self.prune.loading {
            return Task::none();
        }
        self.prune.begin();
        self.prune_confirmed = false;
        self.prune_missing_confirmed = false;
        Task::perform(
            run_blocking({
                let cli = self.cli.clone();
                move || cli.editors_prune()
            }),
            Message::PruneLoaded,
        )
    }

    /// True only for a successful, settled scan. The dialog and both removal
    /// guards use this instead of `Loadable::data`, which keeps the previous
    /// report after a failure.
    fn prune_report_is_ready(&self) -> bool {
        !self.prune.loading && self.prune.error.is_none() && self.prune.data.is_some()
    }

    /// Opens the verify dialog for exactly the build the row named.
    ///
    /// A stale action whose build is no longer installed does nothing rather
    /// than verifying a different build of the same version.
    fn open_verify(&mut self, version: &str, architecture: &str) -> Task<Message> {
        let architecture = architecture.trim().to_string();
        let known = self.editors.data.as_ref().is_some_and(|editors| {
            editors
                .iter()
                .any(|editor| editor.version == version && editor.architecture == architecture)
        });
        if !known {
            return Task::none();
        }

        self.modal = Modal::VerifyEditor {
            version: version.to_string(),
            architecture: architecture.clone(),
        };
        self.verify_version = version.to_string();
        self.verify_architecture = architecture.clone();
        self.verify_show_raw = false;
        // Never show the previous build's result under this build's header.
        self.verify = Loadable::default();
        self.load_verify(version.to_string(), architecture)
    }

    fn load_verify(&mut self, version: String, architecture: String) -> Task<Message> {
        self.verify.begin();
        self.retry_targets.insert(
            ReadKind::EditorVerify.id().to_string(),
            RetryRequest::EditorVerify {
                version: version.clone(),
                architecture: architecture.clone(),
            },
        );
        Task::perform(
            run_blocking({
                let cli = self.cli.clone();
                move || {
                    cli.editors_verify(
                        &version,
                        Some(architecture.as_str()).filter(|value| !value.trim().is_empty()),
                    )
                }
            }),
            Message::VerifyLoaded,
        )
    }

    fn projects_vm(&self) -> ProjectsVm {
        let filtered = self.filtered_projects();
        let total_count = self.projects.data.as_ref().map(Vec::len).unwrap_or(0);
        let visible_count = filtered.len();
        let installed = self.installed_builds();
        let rows = || {
            filtered
                .iter()
                .map(|(_, project)| project_row_vm(project, &installed))
                .collect::<Vec<_>>()
        };

        let projects = if self.projects.loading {
            LoadState::Loading {
                previous: self.projects.data.as_ref().map(|_| rows()),
            }
        } else if let Some(error) = &self.projects.error {
            LoadState::Failed {
                error: self.ui_error(error, ErrorContext::Read(ReadKind::Projects)),
                previous: self.projects.data.as_ref().map(|_| rows()),
            }
        } else if self.projects.data.is_some() {
            LoadState::Ready(rows())
        } else {
            LoadState::Idle
        };

        ProjectsVm {
            search: self.project_search.clone(),
            total_count,
            visible_count,
            projects,
        }
    }

    fn templates_vm(&self) -> TemplatesVm {
        TemplatesVm {
            editors: load_state(&self.editors, |editors| {
                editors
                    .iter()
                    .map(|editor| editor.version.clone())
                    .collect()
            }),
            selected_editor: self.template_editor.clone(),
            filter: self.template_filter,
            installed_only: self.template_installed_only,
            list: load_state(&self.templates, |templates| {
                templates.iter().map(template_row_vm).collect()
            }),
            detail: self.template_detail.data.as_ref().map(template_detail_vm),
            detail_loading: self.template_detail.loading,
            detail_error: self.template_detail.error.as_ref().map(ui_error),
        }
    }

    fn processes_vm(&self) -> ProcessesVm {
        let pipeline_available = self
            .processes
            .data
            .as_ref()
            .map(|running| {
                running
                    .instances
                    .iter()
                    .any(|editor| editor.phase.is_some() || editor.pipeline_version.is_some())
            })
            .unwrap_or(false);

        ProcessesVm {
            rows: load_state(&self.processes, |running| {
                running
                    .instances
                    .iter()
                    .map(|editor| process_row_vm(editor, &self.process_metrics))
                    .collect()
            }),
            metrics_enabled: self.app_settings.process_metrics,
            pipeline_available,
            last_refreshed: self.process_last_refreshed.clone(),
        }
    }

    fn changelog_vm(&self) -> ChangelogVm {
        ChangelogVm {
            loaded: self.changelog_loaded,
            versions: self.changelog_versions(),
            selected_target: self.changelog_target.clone(),
            show_raw: self.show_raw_changelog,
            app_impact_only: self.changelog_app_impact_only,
            docs_scope: self.changelog_docs_scope,
            docs_package: self.changelog_docs_package.clone(),
            docs_scope_note: self.docs_scope_note(),
            state: self.load_state(ReadKind::Changelog, &self.changelog, |view| {
                ChangelogDataVm {
                    version: view.version.clone(),
                    source: view.source.clone(),
                    source_detail: view.source_detail.clone(),
                    entry_count: view.entry_count,
                    app_impact_count: view
                        .sections
                        .iter()
                        .flat_map(|section| section.entries.iter())
                        .filter(|entry| entry.app_impact)
                        .count(),
                    breaking_changes: view
                        .breaking_changes
                        .iter()
                        .map(|change| BreakingChangeVm {
                            text: change.text.clone(),
                            section: change.section.clone(),
                            reason: change.reason.clone(),
                        })
                        .collect(),
                    sections: view
                        .sections
                        .iter()
                        .map(|section| ChangelogSectionVm {
                            title: section.title.clone(),
                            kind: section_kind(&section.title),
                            entries: section
                                .entries
                                .iter()
                                .map(|entry| ChangelogEntryVm {
                                    text: entry.text.clone(),
                                    app_impact: entry.app_impact,
                                    docs: self.entry_docs_vm(entry, &view.version),
                                })
                                .collect(),
                        })
                        .collect(),
                }
            }),
            raw: self.load_state(ReadKind::RawChangelog, &self.raw_changelog, |markdown| {
                markdown.clone()
            }),
        }
    }

    fn docs_scope_note(&self) -> Option<String> {
        if self.changelog_docs_scope == DocsScopeVm::Package
            && self.changelog_docs_package.trim().is_empty()
        {
            return Some(
                "Enter a package name (for example `com.unity.textmeshpro`) before looking up package docs."
                    .to_string(),
            );
        }
        None
    }

    /// Resolves the page scope for one entry. `Automatic` follows the entry's
    /// own candidate: command sequences open CLI docs, class-like identifiers
    /// open the script reference.
    fn docs_scope_for(&self, kind: DocsTopicKind) -> DocsScopeVm {
        match self.changelog_docs_scope {
            DocsScopeVm::Automatic => match kind {
                DocsTopicKind::Cli => DocsScopeVm::Cli,
                DocsTopicKind::ScriptReference => DocsScopeVm::ScriptReference,
            },
            scope => scope,
        }
    }

    fn resolve_docs_scope(&self, scope: DocsScopeVm) -> Option<DocsScope> {
        match scope {
            DocsScopeVm::Automatic => Some(DocsScope::ScriptReference),
            DocsScopeVm::ScriptReference => Some(DocsScope::ScriptReference),
            DocsScopeVm::Manual => Some(DocsScope::Manual),
            DocsScopeVm::Cli => Some(DocsScope::Cli),
            DocsScopeVm::Hub => Some(DocsScope::Hub),
            DocsScopeVm::Cloud => Some(DocsScope::Cloud),
            DocsScopeVm::Package => {
                let package = self.changelog_docs_package.trim();
                if package.is_empty() {
                    None
                } else {
                    Some(DocsScope::Package(package.to_string()))
                }
            }
        }
    }

    /// The per-entry docs state. A cache hit or a failure is rendered directly;
    /// nothing here starts work, so building a frame never blocks rendering.
    fn entry_docs_vm(&self, entry: &ChangelogEntry, version: &str) -> EntryDocsVm {
        let Some(topic) = entry.docs_topic.as_deref() else {
            return EntryDocsVm::Unavailable;
        };
        let scope_vm = self.docs_scope_for(entry.docs_kind);
        let Some(scope) = self.resolve_docs_scope(scope_vm) else {
            return EntryDocsVm::Unavailable;
        };
        let key = DocsKey {
            topic: topic.to_string(),
            version: version.to_string(),
            scope,
        };

        if let Some(page) = self.docs_cache.get(&key) {
            return EntryDocsVm::Ready {
                topic: page.topic.clone(),
                url: page.url.clone(),
                search: page.search,
            };
        }
        if let Some(error) = self.docs_errors.get(&key) {
            if error.code == DOCS_NOT_FOUND {
                return EntryDocsVm::Missing {
                    topic: topic.to_string(),
                };
            }
            return EntryDocsVm::Failed {
                topic: topic.to_string(),
                message: error.message.clone(),
            };
        }
        if self.docs_loading.contains(&key) {
            return EntryDocsVm::Loading {
                topic: topic.to_string(),
            };
        }
        EntryDocsVm::Idle {
            topic: topic.to_string(),
            scope: scope_vm,
        }
    }

    fn docs_key(&self, topic: &str, scope: DocsScopeVm) -> Option<DocsKey> {
        let scope = self.resolve_docs_scope(scope)?;
        Some(DocsKey {
            topic: topic.to_string(),
            version: self.docs_version(),
            scope,
        })
    }

    fn docs_version(&self) -> String {
        self.changelog
            .data
            .as_ref()
            .map(|view| view.version.clone())
            .or_else(|| self.changelog_target.clone())
            .unwrap_or_else(|| self.cli_version_key())
    }

    /// Starts one session-cached `unity docs --url` read on the serialized
    /// reader worker. The entry stays readable while it runs; the result is
    /// keyed by topic + version + scope so a cache hit never runs the CLI.
    fn lookup_changelog_docs(&mut self, topic: String, scope: DocsScopeVm) -> Task<Message> {
        let Some(key) = self.docs_key(&topic, scope) else {
            self.notice = Some(Notice::info(
                "Enter a package name before looking up package documentation.",
            ));
            return Task::none();
        };
        if self.docs_cache.contains_key(&key) || self.docs_loading.contains(&key) {
            return Task::none();
        }
        self.docs_loading.insert(key.clone());

        let lookup = DocsLookup {
            topic: topic.clone(),
            scope: key.scope.clone(),
        };
        let cli = self.cli.clone();
        let version = key.version.clone();
        let resolved_scope = key.scope.clone();
        Task::perform(run_blocking(move || cli.docs(&lookup)), move |result| {
            Message::DocsLoaded {
                topic,
                version,
                scope: resolved_scope,
                result,
            }
        })
    }

    fn diagnostics_vm(&self) -> DiagnosticsVm {
        DiagnosticsVm {
            doctor: self.load_state(ReadKind::Doctor, &self.doctor, doctor_vm),
            environment: self.load_state(ReadKind::Environment, &self.environment, environment_vm),
            proxy_report: self.load_state(ReadKind::DiagnoseProxy, &self.diagnose_proxy, |text| {
                diagnose_report_vm(DiagnoseTopic::Proxy, text)
            }),
            accelerator_report: self.load_state(
                ReadKind::DiagnoseAccelerator,
                &self.diagnose_accelerator,
                |text| diagnose_report_vm(DiagnoseTopic::Accelerator, text),
            ),
            update_report: self.load_state(
                ReadKind::DiagnoseUpdate,
                &self.diagnose_update,
                |text| diagnose_report_vm(DiagnoseTopic::Update, text),
            ),
            can_copy: self.doctor.data.is_some()
                || self.environment.data.is_some()
                || self.diagnose_proxy.data.is_some(),
        }
    }

    /// The combined plain-text report for the Copy button. It contains only
    /// values the CLI reported; a section that failed is named as unavailable
    /// instead of being omitted silently.
    fn diagnostics_report_text(&self) -> String {
        let has_content = self.doctor.data.is_some()
            || self.environment.data.is_some()
            || self.diagnose_proxy.data.is_some()
            || self.doctor.error.is_some()
            || self.environment.error.is_some()
            || self.diagnose_proxy.error.is_some();
        if !has_content {
            return String::new();
        }

        let mut out = String::from("Unity Rust Hub — diagnostics\n");

        if let Some(read) = &self.doctor.data {
            out.push_str("\n== Unity doctor ==\n");
            push_doctor_report(&mut out, read);
        } else if let Some(error) = &self.doctor.error {
            out.push_str("\n== Unity doctor ==\n");
            push_unavailable(&mut out, error);
        } else {
            out.push_str(
                "\n== Unity doctor ==\nNot run. Use Run doctor on the Diagnostics page; it can ask for polkit authorization.\n",
            );
        }

        if let Some(environment) = &self.environment.data {
            out.push_str("\n== Environment ==\n");
            push_environment_report(&mut out, environment);
        } else if let Some(error) = &self.environment.error {
            out.push_str("\n== Environment ==\n");
            push_unavailable(&mut out, error);
        }

        if let Some(report) = &self.diagnose_proxy.data {
            out.push_str("\n== Diagnose proxy ==\n");
            out.push_str(report.trim_end());
            out.push('\n');
        } else if let Some(error) = &self.diagnose_proxy.error {
            out.push_str("\n== Diagnose proxy ==\n");
            push_unavailable(&mut out, error);
        }

        out
    }

    fn diagnose_text(&self, topic: DiagnoseTopic) -> Option<String> {
        let report = match topic {
            DiagnoseTopic::Proxy => &self.diagnose_proxy,
            DiagnoseTopic::Accelerator => &self.diagnose_accelerator,
            DiagnoseTopic::Update => &self.diagnose_update,
        };
        report
            .data
            .as_ref()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    }

    /// Writes a report to the clipboard and reports the backend's real result.
    /// A failed write never produces a success notice, and an empty report is
    /// rejected before the clipboard is touched.
    fn copy_report(&mut self, text: String, success: impl Into<String>) -> Task<Message> {
        if text.trim().is_empty() {
            self.notice = Some(Notice::error("There is no report to copy yet."));
            return Task::none();
        }

        match self.clipboard.write(&text) {
            Ok(()) => self.notice = Some(Notice::success(success.into())),
            Err(reason) => {
                self.notice = Some(Notice::error(format!(
                    "Could not copy the report to the clipboard: {reason}"
                )));
            }
        }
        Task::none()
    }

    fn settings_vm(&self) -> SettingsVm {
        let resolved_executable = self
            .cli_status
            .data
            .as_ref()
            .and_then(|status| status.path.clone())
            .unwrap_or_else(|| "Not found".to_string());
        let resolved_via = self
            .cli_status
            .data
            .as_ref()
            .and_then(|status| status.path_source.clone())
            .unwrap_or_else(|| "—".to_string());
        let config_dir = self.config_dir.clone().unwrap_or_else(|| "—".to_string());
        let settings_file = if config_dir == "—" {
            "—".to_string()
        } else {
            format!("{config_dir}/settings.json")
        };

        let font_family = self.app_settings.font_family.clone();
        let font_options = settings::font_families();

        SettingsVm {
            resolved_executable,
            resolved_via,
            cli_path_input: self.cli_path_input.clone(),
            config_dir,
            settings_file,
            text_scale_options: TextSize::ALL
                .iter()
                .map(|size| TextScaleOptionVm {
                    percent: size.percent(),
                    label: size.to_string(),
                })
                .collect(),
            text_scale_percent: self.app_settings.text_size().percent(),
            font_options,
            font_family,
            engine_info: self.engine_info(),
        }
    }

    fn modal_vm(&self) -> Option<ModalVm> {
        match &self.modal {
            Modal::None => None,
            Modal::InstallCli => Some(ModalVm::InstallCli(InstallCliVm {
                channels: vec![
                    "stable".to_string(),
                    "beta".to_string(),
                    "alpha".to_string(),
                ],
                selected_channel: self.cli_channel.clone(),
                consent: self.cli_consent,
                command_preview: if cfg!(target_os = "windows") {
                    "irm https://unity.com/install.ps1 | iex"
                } else {
                    "curl -fsSL https://unity.com/install.sh | bash"
                }
                .to_string(),
                can_install: self.cli_consent,
            })),
            Modal::EditorModules(editor) => {
                let modules =
                    self.load_state(ReadKind::DialogModules, &self.dialog_modules, |modules| {
                        modules
                            .iter()
                            .filter(|module| !module.is_installed)
                            .map(|module| ModuleOptionVm {
                                id: module.id.clone(),
                                label: format!(
                                    "{}  ·  {}",
                                    module.name,
                                    format_bytes(module.download_bytes)
                                ),
                                selected: self.dialog_selected.contains(&module.id),
                            })
                            .collect()
                    });

                let retries_error =
                    unity_core::versions::validate_retry_count(&self.dialog_retries)
                        .err()
                        .map(|error| error.message);
                let directory_error = unity_core::versions::validate_download_directory(
                    &self.dialog_download_directory,
                )
                .err()
                .map(|error| error.message);
                let command_preview = {
                    let modules: Vec<String> = self.dialog_selected.iter().cloned().collect();
                    let fallback_modules = if modules.is_empty() {
                        String::new()
                    } else {
                        modules
                            .iter()
                            .map(|module| format!(" -m {module}"))
                            .collect::<String>()
                    };
                    let fallback = format!(
                        "unity install-modules --editor-version {} --yes{fallback_modules}",
                        editor.version
                    );
                    let spec = if modules.is_empty() {
                        Err(UnityError::invalid_input("Select at least one module."))
                    } else {
                        self.cli.install_modules_spec(
                            &editor.version,
                            Some(&editor.architecture),
                            &modules,
                            self.dialog_eula,
                            &ModuleInstallOptions {
                                retries: unity_core::versions::validate_retry_count(
                                    &self.dialog_retries,
                                )
                                .unwrap_or_default(),
                                download_directory:
                                    unity_core::versions::validate_download_directory(
                                        &self.dialog_download_directory,
                                    )
                                    .unwrap_or_default(),
                            },
                        )
                    };
                    self.command_preview(spec, &fallback)
                };

                Some(ModalVm::EditorModules(EditorModulesVm {
                    editor_version: editor.version.clone(),
                    architecture: editor.architecture.clone(),
                    installed_modules: editor.module_names.clone(),
                    modules,
                    selected_count: self.dialog_selected.len(),
                    accept_eula: self.dialog_eula,
                    can_install: !self.dialog_selected.is_empty()
                        && self.dialog_eula
                        && retries_error.is_none()
                        && directory_error.is_none(),
                    retries_input: self.dialog_retries.clone(),
                    download_directory: self.dialog_download_directory.clone(),
                    retries_error: retries_error.or(directory_error),
                    command_preview,
                }))
            }
            Modal::UpgradeProject(project) => {
                let mut target_options: Vec<String> = Vec::new();
                if let Some(editors) = self.editors.data.as_ref() {
                    for editor in editors
                        .iter()
                        .filter(|editor| editor.version != project.version)
                    {
                        if !target_options.contains(&editor.version) {
                            target_options.push(editor.version.clone());
                        }
                    }
                }
                let target_note = self.upgrade_target_note(&target_options);

                Some(ModalVm::UpgradeProject(UpgradeProjectVm {
                    project_title: project.title.clone(),
                    project_path_short: short_path(&project.path, 60).into_owned(),
                    current_version: project.version.clone(),
                    current_architecture: project.architecture.clone(),
                    target_options,
                    target_note,
                    selected_target: self.upgrade_target.clone(),
                    confirmed: self.upgrade_confirmed,
                    can_upgrade: self.upgrade_confirmed && self.upgrade_target.is_some(),
                }))
            }
            Modal::RemoveProject(project) => Some(ModalVm::RemoveProject(RemoveProjectVm {
                project_title: project.title.clone(),
                project_path_short: short_path(&project.path, 60).into_owned(),
            })),
            Modal::NewProject(state) => {
                Some(ModalVm::NewProject(Box::new(self.new_project_vm(state))))
            }
            Modal::TemplateAuthoring(state) => Some(ModalVm::TemplateAuthoring(Box::new(
                self.authoring_vm(state),
            ))),
            Modal::PruneEditors => Some(ModalVm::PruneEditors(self.prune_editors_vm())),
            Modal::RollbackCli => Some(ModalVm::RollbackCli(self.rollback_cli_vm())),
            Modal::VerifyEditor {
                version,
                architecture,
            } => {
                let build = Some(architecture.as_str()).filter(|value| !value.trim().is_empty());
                let fallback = if architecture.trim().is_empty() {
                    format!("unity editors verify {version}")
                } else {
                    format!("unity editors verify {version} --architecture {architecture}")
                };
                Some(ModalVm::VerifyEditor(VerifyEditorVm {
                    version: version.clone(),
                    architecture: architecture.clone(),
                    command: self.command_preview(
                        self.cli.editors_verify_spec(version, build),
                        fallback.as_str(),
                    ),
                    state: self.load_state(ReadKind::EditorVerify, &self.verify, verify_result_vm),
                    show_raw: self.verify_show_raw,
                }))
            }
        }
    }

    fn new_project_vm(&self, state: &NewProjectState) -> NewProjectVm {
        let preflight = if state.license.loading && state.license.data.is_none() {
            NewProjectPreflightVm {
                state: NewProjectPreflightState::Checking,
                license_ok: false,
                message: Some("Checking the Unity license…".to_string()),
                can_continue: false,
            }
        } else if let Some(license) = &state.license.data {
            if license.active {
                NewProjectPreflightVm {
                    state: NewProjectPreflightState::Ready,
                    license_ok: true,
                    message: None,
                    can_continue: true,
                }
            } else {
                NewProjectPreflightVm {
                    state: NewProjectPreflightState::Blocked,
                    license_ok: false,
                    message: Some(
                        "Unity reports no active license. Sign in and activate a license before creating a project."
                            .to_string(),
                    ),
                    can_continue: false,
                }
            }
        } else if let Some(error) = &state.license.error {
            NewProjectPreflightVm {
                state: NewProjectPreflightState::Blocked,
                license_ok: false,
                message: Some(error_text(error)),
                can_continue: false,
            }
        } else {
            NewProjectPreflightVm {
                state: NewProjectPreflightState::Idle,
                license_ok: false,
                message: None,
                can_continue: true,
            }
        };

        let templates = load_state(&state.templates, |templates| {
            templates
                .iter()
                .filter(|template| {
                    state.form.show_builtin_templates || !is_builtin_template(&template.name)
                })
                .map(template_row_vm)
                .collect()
        });

        let form = state.form.clone();
        let options = CreateProjectOptions {
            name: form.name.trim().to_string(),
            output_dir: form.parent_dir.trim().to_string(),
            editor_version: form.editor_version.trim().to_string(),
            template: form.template.trim().to_string(),
            architecture: form.architecture.trim().to_string(),
            open: form.open_after_create,
            bare: false,
        };
        let command_preview = self
            .cli
            .create_project_spec(&options)
            .map(|spec| spec.display())
            .unwrap_or_default();

        NewProjectVm {
            step: state.step,
            form,
            editors: load_state(&state.editors, |editors| {
                editors
                    .iter()
                    .map(|editor| NewProjectEditorVm {
                        version: editor.version.clone(),
                        architecture: editor.architecture.clone(),
                        is_default: editor.default,
                    })
                    .collect()
            }),
            templates,
            selected_template: state.selected_template.as_ref().map(template_detail_vm),
            preflight,
            issues: state.issues.clone(),
            command_preview,
            result: state.result.as_ref().map(new_project_result_vm),
            job: self
                .active_job
                .as_ref()
                .filter(|job| job.kind == "project-create")
                .map(job_vm),
        }
    }

    fn authoring_vm(&self, state: &TemplateAuthoringState) -> TemplateAuthoringVm {
        let projects = self
            .projects
            .data
            .as_ref()
            .map(|projects| {
                projects
                    .iter()
                    .map(|project| project.path.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let options = state.create_options();
        let pack_options = state.pack_options();
        let command_preview = if state.mode.is_pack() {
            self.cli
                .templates_pack_spec(&pack_options)
                .map(|spec| spec.display())
                .unwrap_or_default()
        } else {
            self.cli
                .templates_create_spec(&options)
                .map(|spec| spec.display())
                .unwrap_or_default()
        };
        let job = self
            .active_job
            .as_ref()
            .filter(|job| matches!(job.kind.as_str(), "template-create" | "template-pack"))
            .map(job_vm);

        TemplateAuthoringVm {
            mode: state.mode,
            projects,
            selected_project: (!state.source.trim().is_empty()).then(|| state.source.clone()),
            form: TemplateFormVm {
                name: state.form.name.clone(),
                display_name: state.form.display_name.clone(),
                description: state.form.description.clone(),
                version: state.form.version.clone(),
                output: state.form.output.clone(),
            },
            overwrite: state.overwrite_confirmed,
            overwrite_confirmed: state.overwrite_confirmed,
            can_submit: state.issues.is_empty()
                && !state.source.trim().is_empty()
                && !state.form.name.trim().is_empty()
                && !state.form.display_name.trim().is_empty()
                && !state.form.output.trim().is_empty()
                && job.is_none(),
            issues: state.issues.clone(),
            command_preview,
            result: state.result.clone(),
            job,
        }
    }

    /// Builds the prune dialog. Every command preview comes from the adapter,
    /// so the dialog shows exactly what a button would run.
    fn prune_editors_vm(&self) -> PruneEditorsVm {
        let report = self.load_state(ReadKind::Prune, &self.prune, prune_report_vm);
        // Counts, labels, and confirmations must come from a successful Ready
        // report only. `LoadState::data()` also returns `previous` for a failed
        // or loading read, which would let a stale candidate list arm a command
        // that removes whatever the CLI considers unused at run time.
        let ready = match &report {
            LoadState::Ready(report) => Some(report),
            _ => None,
        };
        let (unused_count, missing_count, reclaimable, unused_names, missing_names) = match ready {
            Some(report) => (
                report.unused.len(),
                report.missing.len(),
                report.reclaimable.clone(),
                candidate_names(&report.unused),
                candidate_names(&report.missing),
            ),
            None => (0, 0, String::new(), String::new(), String::new()),
        };
        let any_job_running = self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false);
        // Only a streamed prune job gets the "Removing…" callout; another job
        // still blocks both buttons, but it is not this dialog's work.
        let job_running = self.active_job.as_ref().is_some_and(|job| {
            job.is_running() && matches!(job.kind.as_str(), "editor-prune" | "editor-prune-missing")
        });
        let busy = any_job_running || self.prune.loading || self.prune.error.is_some();

        let remove_label = if unused_count == 0 {
            "Remove unused Editors".to_string()
        } else {
            format!(
                "Remove {unused_count} unused Editor{} ({reclaimable})",
                plural(unused_count)
            )
        };
        let missing_label = if missing_count == 0 {
            "Remove missing registry entries (no files deleted)".to_string()
        } else {
            format!(
                "Remove {missing_count} missing registry entr{} (no files deleted)",
                if missing_count == 1 { "y" } else { "ies" }
            )
        };
        let confirm_label = if unused_count == 0 {
            "No unused Editors are selected.".to_string()
        } else {
            format!(
                "I understand these Editors and their files will be uninstalled: {unused_names}."
            )
        };
        let missing_confirm_label = if missing_count == 0 {
            "No missing registry entries are selected.".to_string()
        } else {
            format!(
                "I understand only these registry entries are dropped; no files are deleted: {missing_names}."
            )
        };

        PruneEditorsVm {
            report,
            confirmed: self.prune_confirmed,
            missing_confirmed: self.prune_missing_confirmed,
            force: self.prune_force,
            force_available: self.prune_force_available,
            scan_command: self.command_preview(
                self.cli.editors_prune_report_spec(),
                "unity editors prune --remove-missing",
            ),
            remove_command: self.command_preview(
                self.cli
                    .editors_prune_remove_spec(self.prune_force && self.prune_force_available),
                "unity editors prune --remove --yes",
            ),
            missing_command: self.command_preview(
                self.cli.editors_prune_remove_missing_spec(),
                "unity editors prune --remove-missing --yes",
            ),
            remove_label,
            missing_label,
            confirm_label,
            missing_confirm_label,
            can_remove: unused_count > 0 && self.prune_confirmed && !busy,
            can_remove_missing: missing_count > 0 && self.prune_missing_confirmed && !busy,
            error: self.prune_error.as_ref().map(|error| {
                self.ui_error(
                    error,
                    ErrorContext::Job {
                        kind: "editor-prune".to_string(),
                    },
                )
            }),
            job_running,
        }
    }

    fn command_preview(&self, spec: Result<CommandSpec, UnityError>, fallback: &str) -> String {
        spec.map(|spec| spec.display())
            .unwrap_or_else(|_| fallback.to_string())
    }

    fn first_other_editor_version(&self, current: &str) -> Option<String> {
        self.editors
            .data
            .as_ref()?
            .iter()
            .map(|editor| editor.version.clone())
            .find(|version| version != current)
    }

    /// Names target versions with more than one installed build.
    ///
    /// `unity projects upgrade --to <version>` cannot pick a build, so the
    /// dialog offers versions only and says which ones the CLI resolves. That
    /// avoids silently pretending the clicked build is the one that opens.
    fn upgrade_target_note(&self, targets: &[String]) -> Option<String> {
        let editors = self.editors.data.as_ref()?;
        let mut ambiguous = Vec::new();
        for target in targets {
            let mut architectures: Vec<&str> = editors
                .iter()
                .filter(|editor| &editor.version == target)
                .map(|editor| editor.architecture.as_str())
                .filter(|architecture| !architecture.trim().is_empty())
                .collect();
            architectures.sort_unstable();
            architectures.dedup();
            if architectures.len() > 1 {
                ambiguous.push(format!(
                    "{target} has {} builds installed",
                    architectures.join(" and ")
                ));
            }
        }
        if ambiguous.is_empty() {
            return None;
        }
        Some(format!(
            "{}. `unity projects upgrade` selects by version, not by build, so the CLI chooses which installed build opens the project; this dialog does not silently pick one. Review the builds on the Editors page before upgrading.",
            ambiguous.join("; ")
        ))
    }

    fn running_editor_for_project(&self, path: &str) -> Option<&RunningEditor> {
        let running = self.running.data.as_ref()?;
        running.instances.iter().find(|editor| {
            editor
                .project_path
                .as_deref()
                .is_some_and(|candidate| same_project_path(candidate, path))
        })
    }

    /// Fills the target when Editors finish loading after the dialog opened.
    fn ensure_upgrade_target(&mut self) {
        if self.upgrade_target.is_some() {
            return;
        }
        let Modal::UpgradeProject(project) = &self.modal else {
            return;
        };
        self.upgrade_target = self.first_other_editor_version(&project.version);
    }

    pub fn filtered_projects(&self) -> Vec<(usize, &Project)> {
        let Some(projects) = self.projects.data.as_ref() else {
            return Vec::new();
        };
        let needle = self.project_search.trim().to_lowercase();

        if needle.is_empty() {
            return projects.iter().enumerate().collect();
        }

        projects
            .iter()
            .enumerate()
            .filter(|(index, project)| {
                self.project_search_index
                    .get(*index)
                    .map(|key| key.matches(&needle))
                    .unwrap_or_else(|| project_matches_fallback(project, &needle))
            })
            .collect()
    }

    /// Every installed build, keyed by `(version, architecture)`.
    ///
    /// This is the identity used by rows, project badges, and upgrade choices;
    /// a version key alone cannot tell an `arm64` build from an `x86_64` one.
    pub fn installed_builds(&self) -> HashSet<(&str, &str)> {
        self.editors
            .data
            .as_ref()
            .map(|editors| {
                editors
                    .iter()
                    .map(|editor| (editor.version.as_str(), editor.architecture.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn changelog_versions(&self) -> Vec<String> {
        let mut versions = Vec::new();
        if let Some(status) = &self.cli_status.data {
            if let Some(version) = &status.version {
                versions.push(version.version.clone());
            }
        }
        if let Some(update) = &self.update.data {
            if update.update_available {
                versions.push(update.latest_version.clone());
            }
        }
        versions.dedup();
        versions
    }

    pub fn engine_info(&self) -> String {
        let status = self.cli_status.data.as_ref();
        match status.and_then(|status| status.version.as_ref()) {
            Some(version) => format!(
                "{} · {} · {}/{}",
                version.version, version.channel, version.platform, version.arch
            ),
            None => "Unity CLI not detected".to_string(),
        }
    }

    fn job_vm(&self, job: &ActiveJob) -> JobVm {
        JobVm {
            kind: job.kind.clone(),
            command: job.command.clone(),
            phase: match job.state {
                JobState::Running => JobPhase::Running,
                JobState::Succeeded => JobPhase::Succeeded,
                JobState::Failed => JobPhase::Failed,
                JobState::Cancelled => JobPhase::Cancelled,
            },
            progress: job.progress,
            message: job.message.clone(),
            error: job.error.as_ref().map(|error| {
                self.ui_error(
                    error,
                    ErrorContext::Job {
                        kind: job.kind.clone(),
                    },
                )
            }),
            logs: job.log_text.clone(),
        }
    }
}

fn default_editor(editors: &[Editor]) -> Option<&Editor> {
    editors.iter().find(|editor| editor.default).or_else(|| {
        editors
            .iter()
            .max_by(|left, right| left.version.cmp(&right.version))
    })
}

fn template_filter(filter: TemplateKindFilter) -> TemplateFilter {
    match filter {
        TemplateKindFilter::All => TemplateFilter::All,
        TemplateKindFilter::Core => TemplateFilter::Core,
        TemplateKindFilter::Custom => TemplateFilter::Custom,
    }
}

fn is_builtin_template(name: &str) -> bool {
    matches!(name, "com.unity.template.3d" | "com.unity.template.2d")
}

fn template_pipeline(name: &str, display_name: &str) -> Option<String> {
    let label = format!("{name} {display_name}").to_lowercase();
    if label.contains("universal-2d") || label.contains("universal 2d") {
        Some("URP · 2D".to_string())
    } else if label.contains("urp") || label.contains("universal 3d") || label.contains("universal")
    {
        Some("URP".to_string())
    } else if label.contains("hdrp") || label.contains("high definition") {
        Some("HDRP".to_string())
    } else if label.contains("built-in") || label.contains("built in") {
        Some("Built-in".to_string())
    } else {
        None
    }
}

fn template_status_tone(status: &str) -> Tone {
    let status = status.to_ascii_lowercase();
    if status.contains("ready") {
        Tone::Success
    } else if status.contains("upgrad") {
        Tone::Warning
    } else if status.contains("download") {
        Tone::Info
    } else {
        Tone::Neutral
    }
}

fn template_row_vm(template: &TemplateBrief) -> TemplateRowVm {
    TemplateRowVm {
        name: template.name.clone(),
        display_name: if template.display_name.trim().is_empty() {
            template.name.clone()
        } else {
            template.display_name.clone()
        },
        kind: template.kind.clone(),
        version: template.version.clone(),
        status: template.status.clone(),
        status_tone: template_status_tone(&template.status),
        installed: template.is_installed(),
        pipeline: template_pipeline(&template.name, &template.display_name),
    }
}

fn template_detail_vm(info: &TemplateInfo) -> TemplateDetailVm {
    TemplateDetailVm {
        name: info.name.clone(),
        display_name: if info.display_name.trim().is_empty() {
            info.name.clone()
        } else {
            info.display_name.clone()
        },
        description: info.description.clone(),
        kind: info.kind.clone(),
        version: info.version.clone(),
        status: info.status.clone(),
        installed: info.is_installed(),
        render_pipeline: info.render_pipeline_label().map(str::to_string),
        packages: info
            .packages
            .iter()
            .map(|package| {
                let name = if package.package_name.trim().is_empty() {
                    package.name.as_str()
                } else {
                    package.package_name.as_str()
                };
                format!("{name} {}", package.version).trim().to_string()
            })
            .collect(),
        previous_versions: info.previous_version_names(),
    }
}

fn project_name_from_path(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn process_state_tone(state: &str) -> Tone {
    let state = state.to_ascii_lowercase();
    if state.contains("ready") {
        Tone::Success
    } else if state.contains("start") || state.contains("load") {
        Tone::Info
    } else if state.contains("unreachable") || state.contains("stale") {
        Tone::Danger
    } else {
        Tone::Neutral
    }
}

fn process_row_vm(editor: &RunningEditor, metrics: &HashMap<u32, ProcessMetrics>) -> ProcessRowVm {
    let project_path = editor
        .project_path
        .clone()
        .unwrap_or_else(|| "Unity Editor".to_string());
    let state = editor
        .state
        .clone()
        .filter(|state| !state.trim().is_empty())
        .or_else(|| editor.phase.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let phase = editor.phase.clone();
    let play_mode = phase
        .as_deref()
        .map(|phase| {
            let phase = phase.to_ascii_lowercase();
            phase.contains("play") && !phase.contains("stop")
        })
        .unwrap_or(false);
    let metrics = editor
        .pid
        .and_then(|pid| metrics.get(&pid))
        .map(|sample| ProcessMetricsVm {
            cpu: sample
                .cpu_percent
                .map(|cpu| format!("{cpu:.1}%"))
                .unwrap_or_else(|| "—".to_string()),
            memory: format_bytes(sample.memory_bytes),
            uptime: format_duration(sample.uptime),
        });

    ProcessRowVm {
        project_path: project_path.clone(),
        project_name: project_name_from_path(&project_path),
        editor_version: editor
            .editor_version
            .clone()
            .filter(|version| !version.trim().is_empty())
            .unwrap_or_else(|| "unknown".to_string()),
        pid: editor.pid.map(|pid| pid.to_string()),
        port: editor.port.map(|port| port.to_string()),
        state_tone: process_state_tone(&state),
        state,
        phase,
        play_mode,
        frame_count: editor.frame_count.map(|count| count.to_string()),
        pipeline_version: editor.pipeline_version.clone(),
        safe_mode: editor.in_safe_mode(),
        dialog: editor.dialog_text(),
        metrics,
    }
}

fn new_project_result_vm(result: &CreateProjectResult) -> NewProjectResultVm {
    let failed: Vec<String> = result
        .failed_steps()
        .into_iter()
        .map(|step| match step.detail() {
            Some(detail) => format!("{}: {detail}", step.id),
            None => step.id.clone(),
        })
        .collect();
    let success = !result.path.trim().is_empty() && failed.is_empty();
    let summary = if success {
        format!("Project created at {}.", result.path)
    } else if !result.path.trim().is_empty() {
        format!(
            "The project was created at {}, but {} setup step(s) failed.",
            result.path,
            failed.len()
        )
    } else {
        "Project creation did not report a path.".to_string()
    };

    NewProjectResultVm {
        success,
        path: (!result.path.trim().is_empty()).then(|| result.path.clone()),
        summary,
        failed_steps: failed,
    }
}

fn result_payload<T: serde::de::DeserializeOwned>(value: &Value) -> Option<T> {
    value
        .get("data")
        .cloned()
        .and_then(|data| serde_json::from_value(data).ok())
        .or_else(|| serde_json::from_value(value.clone()).ok())
}

fn template_offers(error: &UnityError) -> Option<Vec<String>> {
    let offered = error
        .extra
        .get("offered")
        .or_else(|| error.extra.get("data").and_then(|data| data.get("offered")))?;
    let values = offered.as_array()?;
    let names: Vec<String> = values
        .iter()
        .filter_map(|value| {
            value.as_str().map(str::to_string).or_else(|| {
                value
                    .get("name")
                    .or_else(|| value.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
        .collect();
    (!names.is_empty()).then_some(names)
}

fn result_path(value: &Value) -> Option<String> {
    let data = value.get("data").unwrap_or(value);
    for key in [
        "path",
        "archive",
        "output",
        "outputPath",
        "templatePath",
        "target",
    ] {
        if let Some(path) = data.get(key).and_then(Value::as_str) {
            if !path.trim().is_empty() {
                return Some(path.to_string());
            }
        }
    }
    None
}

fn ui_error(error: &UnityError) -> UiError {
    UiError {
        code: error.code.clone(),
        message: error.message.clone(),
        detail: error
            .detail
            .clone()
            .filter(|detail| !detail.trim().is_empty()),
        action: None,
    }
}

/// No job kind in this app is retryable today: every job installs, upgrades,
/// signs in, or updates the CLI itself. Replaying any of those after a partial
/// run is exactly what the ticket forbids. A future read-only job opts in here
/// with an explicit kind; it must never default to retryable.
fn job_allows_retry(_kind: &str) -> bool {
    false
}

/// Folds one install frame's per-file rows into the job's last-seen map and
/// returns the aggregate percentage.
///
/// 1.1.0 downloads up to three files at once and emits one scalar per frame.
/// Treating that scalar as the whole install made the bar jump backwards
/// between files. Each file keeps its own last value (completed = 100, queued =
/// 0, active = its own percentage or the frame's), and the result is their
/// mean. The caller clamps it to the job's high-water mark.
fn aggregate_progress(
    state: &mut BTreeMap<String, f32>,
    downloads: &[DownloadStatus],
    frame_progress: Option<f32>,
) -> Option<f32> {
    if downloads.is_empty() {
        return frame_progress;
    }

    for download in downloads {
        let current = state.get(&download.key).copied().unwrap_or(0.0);
        let next = if let Some(pct) = download.pct {
            pct
        } else if download.failed {
            // A failed transfer must not count as finished; keep what it
            // reached and let the job's error frame explain it.
            current
        } else if download.downloading {
            frame_progress.unwrap_or(current)
        } else if download.queued {
            0.0
        } else {
            100.0
        };
        state.insert(download.key.clone(), current.max(next));
    }

    if state.is_empty() {
        return frame_progress;
    }
    let total: f32 = state.values().sum();
    Some(total / state.len() as f32)
}

fn trimmed_option(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// The folder a download-only result names.
fn download_path(result: Option<&Value>) -> Option<String> {
    let data = result?.get("data")?;
    ["path", "downloadPath", "downloadFolder", "directory"]
        .iter()
        .find_map(|key| data.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
}

/// Whether a finished Editor install can reuse its cache.
///
/// The user can always ask for `--resume` from Advanced. Automatically, Resume
/// is offered only when the CLI's own result says a partial download is
/// resumable, or the failure is one of the interruption/timeout classes where a
/// cache is the only thing left to reuse. A generic permission or disk error
/// does not offer Resume.
fn resume_reason(error: Option<&UnityError>, result: Option<&Value>) -> Option<String> {
    if let Some(data) = result.and_then(|result| result.get("data")) {
        for key in ["resumable", "resumeAvailable", "interrupted"] {
            if data.get(key).and_then(Value::as_bool) == Some(true) {
                return Some(
                    "The CLI reported an interrupted download with a resumable cache.".to_string(),
                );
            }
        }
        for key in ["cachePath", "downloadCachePath", "partialPath"] {
            if let Some(path) = data.get(key).and_then(Value::as_str) {
                if !path.trim().is_empty() {
                    return Some(format!(
                        "The CLI kept the partial download at {}.",
                        path.trim()
                    ));
                }
            }
        }
    }

    let code = error?.code.as_str();
    let reason = match code {
        cli_code::CANCELLED | cli_code::INTERRUPTED => {
            "The install was interrupted; its downloaded files are in the cache."
        }
        error_code::TIMEOUT => {
            "The install stopped waiting and left its downloaded files in the cache."
        }
        error_code::NETWORK_UNREACHABLE => {
            "The download was cut off; its completed files are cached."
        }
        cli_code::DOWNLOAD_FAILED | cli_code::CHECKSUM_MISMATCH => {
            "A download failed mid-transfer; the CLI kept the partial payload in its cache."
        }
        _ => return None,
    };
    Some(reason.to_string())
}

/// `--force` exists only for the prune running-Editor check. Any other failure
/// (permissions, a busy file, an elevated uninstall that needs a terminal)
/// must not be retried with `--force`.
fn prune_force_available_for(error: &UnityError) -> bool {
    matches!(
        error.code.as_str(),
        cli_code::EDITORS_PRUNE_EDITOR_CHECK_FAILED
            | cli_code::EDITORS_PRUNE_EDITOR_CHECK_UNAVAILABLE
    )
}

/// The proxy the CLI would use, as a display string for the network error
/// hint. The app has no proxy setting, so this only reports the environment;
/// the credentials are stripped before the hint reaches a view model.
fn configured_proxy() -> Option<String> {
    const KEYS: [&str; 7] = [
        "UNITY_PROXY",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ];

    KEYS.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| format!("{key}={}", redact_proxy(&value)))
    })
}

/// Best-effort path comparison for the running-Editor guard. The CLI remains
/// authoritative, so this only needs to catch the common same-folder spelling.
fn same_project_path(left: &str, right: &str) -> bool {
    fn trim(path: &str) -> &str {
        path.trim().trim_end_matches(['/', '\\'])
    }

    let left = trim(left);
    let right = trim(right);
    if left == right {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => cfg!(windows) && left.eq_ignore_ascii_case(right),
    }
}

fn load_state<T, U>(loadable: &Loadable<T>, map: impl Fn(&T) -> U) -> LoadState<U> {
    if loadable.loading {
        return LoadState::Loading {
            previous: loadable.data.as_ref().map(&map),
        };
    }
    if let Some(error) = &loadable.error {
        return LoadState::Failed {
            error: ui_error(error),
            previous: loadable.data.as_ref().map(&map),
        };
    }
    match &loadable.data {
        Some(data) => LoadState::Ready(map(data)),
        None => LoadState::Idle,
    }
}

fn notification_tone(level: NotificationLevel) -> Tone {
    match level {
        NotificationLevel::Info => Tone::Info,
        NotificationLevel::Success => Tone::Success,
        NotificationLevel::Warning => Tone::Warning,
        NotificationLevel::Danger => Tone::Danger,
    }
}

fn notice_tone(kind: NoticeKind) -> Tone {
    match kind {
        NoticeKind::Info => Tone::Info,
        NoticeKind::Success => Tone::Success,
        NoticeKind::Error => Tone::Danger,
    }
}

const UNKNOWN_CLI_VERSION: &str = "unknown";

fn job_vm(job: &ActiveJob) -> JobVm {
    JobVm {
        kind: job.kind.clone(),
        command: job.command.clone(),
        phase: match job.state {
            JobState::Running => JobPhase::Running,
            JobState::Succeeded => JobPhase::Succeeded,
            JobState::Failed => JobPhase::Failed,
            JobState::Cancelled => JobPhase::Cancelled,
        },
        progress: job.progress,
        message: job.message.clone(),
        error: job.error.as_ref().map(ui_error),
        logs: job.log_text.clone(),
    }
}
fn doctor_vm(read: &DoctorRead) -> DoctorVm {
    let report = &read.report;

    DoctorVm {
        platform: dash(&report.platform),
        arch: dash(&report.arch),
        runtime_version: option_dash(report.runtime_version.as_deref()),
        node_version: option_dash(report.node_version.as_deref()),
        cli_version: option_dash(report.cli_version.as_deref()),
        install_path: option_dash(report.install_path.as_deref()),
        log_directory: option_dash(report.log_directory.as_deref()),
        checks: report.checks.iter().map(doctor_check_vm).collect(),
        auth: report.auth.as_ref().and_then(redacted_value_summary),
        license: report.license.as_ref().and_then(redacted_value_summary),
        raw: sanitize_doctor_raw(&read.raw),
    }
}

fn doctor_check_vm(check: &DoctorCheck) -> DoctorCheckVm {
    DoctorCheckVm {
        id: check.id.clone(),
        label: check_label(check),
        status: check_status(&check.status),
        detail: params_detail(&check.params),
    }
}

/// Humanizes an id first, then a pre-rendered CLI message, then the
/// localization key. The key is never shown raw unless nothing else exists.
fn check_label(check: &DoctorCheck) -> String {
    let id = check.id.trim();
    if !id.is_empty() {
        return humanize_identifier(id);
    }
    if let Some(message) = check.message.as_deref().map(str::trim) {
        if !message.is_empty() {
            return message.to_string();
        }
    }
    if let Some(key) = check.message_key.as_deref().map(str::trim) {
        if !key.is_empty() {
            return humanize_message_key(key);
        }
    }
    "Check".to_string()
}

fn humanize_message_key(key: &str) -> String {
    let leaf = key.rsplit('.').find(|part| !part.is_empty()).unwrap_or(key);
    humanize_identifier(leaf)
}

/// Turns `path-available`, `git-credential-helper`, or `pathAvailable` into a
/// readable label. Known acronyms keep their uppercase spelling.
fn humanize_identifier(value: &str) -> String {
    const ACRONYMS: [&str; 15] = [
        "cli", "url", "id", "os", "cpu", "gpu", "fex", "vcs", "http", "https", "tls", "ssl", "tpm",
        "api", "sdk",
    ];
    const ACRONYM_IL2CPP: &str = "il2cpp";

    let mut words = Vec::new();
    let mut word = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() {
            let boundary = character.is_uppercase()
                && !word.is_empty()
                && word
                    .chars()
                    .last()
                    .is_some_and(|previous| previous.is_lowercase() || previous.is_numeric());
            if boundary {
                words.push(std::mem::take(&mut word));
            }
            word.push(character);
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    if words.is_empty() {
        return value.trim().to_string();
    }

    words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let lower = word.to_lowercase();
            if lower == ACRONYM_IL2CPP || ACRONYMS.contains(&lower.as_str()) {
                lower.to_uppercase()
            } else if index == 0 {
                let mut characters = lower.chars();
                match characters.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                    None => String::new(),
                }
            } else {
                lower
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn check_status(status: &str) -> CheckStatus {
    match status.trim().to_ascii_lowercase().as_str() {
        "pass" => CheckStatus::Pass,
        "warn" => CheckStatus::Warn,
        "fail" => CheckStatus::Fail,
        "info" => CheckStatus::Info,
        _ => CheckStatus::Unknown,
    }
}

fn check_status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "pass",
        CheckStatus::Warn => "warn",
        CheckStatus::Fail => "fail",
        CheckStatus::Info => "info",
        CheckStatus::Unknown => "unknown",
    }
}

fn params_detail(params: &Map<String, Value>) -> Option<String> {
    let details = params
        .iter()
        .filter_map(|(key, value)| {
            scalar_text(value).map(|value| format!("{}: {value}", humanize_identifier(key)))
        })
        .collect::<Vec<_>>();
    (!details.is_empty()).then(|| details.join(" · "))
}

/// A faithful, one-line rendering of a JSON scalar or small container. Nothing
/// is interpreted: a string stays a string, a number stays a number.
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(items) => {
            let parts = items.iter().filter_map(scalar_text).collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        Value::Object(map) => {
            let text = serde_json::to_string(value).ok()?;
            (!map.is_empty()).then_some(text)
        }
        Value::Null => None,
    }
}

/// Removes the credential-bearing fields from a doctor raw document before it
/// reaches the view model or the combined support report.
///
/// The CLI is not documented as redacting `unity doctor` the way it redacts
/// `unity diagnose`, so known secret keys are replaced and every URL loses its
/// user-info. Ordinary fields, checks, paths, and versions are kept.
fn sanitize_doctor_raw(raw: &str) -> String {
    match serde_json::from_str::<Value>(raw) {
        Ok(mut value) => {
            redact_doctor_value(&mut value, None);
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| redact_doctor_text(raw))
        }
        Err(_) => redact_doctor_text(raw),
    }
}

fn redact_doctor_text(text: &str) -> String {
    redact_url_credentials(text)
}

fn redact_doctor_value(value: &mut Value, key: Option<&str>) {
    if key.is_some_and(is_sensitive_key) {
        *value = Value::String("***".to_string());
        return;
    }

    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                redact_doctor_value(value, Some(key));
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact_doctor_value(item, key);
            }
        }
        Value::String(text) => {
            let redacted = redact_url_credentials(text);
            if redacted != *text {
                *text = redacted;
            }
        }
        _ => {}
    }
}

/// `redact_proxy` trims its input, which is right for a proxy hint but would
/// silently alter unrelated whitespace in the doctor document. Keep the
/// original surrounding whitespace and only mask the URL's user-info.
fn redact_url_credentials(text: &str) -> String {
    let start = text.len() - text.trim_start().len();
    let end = text.trim_end().len();
    if start >= end {
        return text.to_string();
    }

    let value = &text[start..end];
    let redacted = redact_proxy(value);
    if redacted == value {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..start]);
    result.push_str(&redacted);
    result.push_str(&text[end..]);
    result
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();

    key == "key"
        || [
            "token",
            "secret",
            "password",
            "passwd",
            "passphrase",
            "credential",
            "authorization",
            "cookie",
            "session",
            "serial",
            "privatekey",
            "licensekey",
            "apikey",
        ]
        .iter()
        .any(|needle| key.contains(needle))
}

fn redacted_value_summary(value: &Value) -> Option<String> {
    let mut value = value.clone();
    redact_doctor_value(&mut value, None);
    value_summary(&value)
}

/// The CLI's `auth` / `license` sections have no stable schema this app can
/// model yet, so their scalar fields are summarized without interpretation.
/// The value is sanitized by [`redacted_value_summary`] before this runs.
fn value_summary(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            let parts = map
                .iter()
                .filter_map(|(key, value)| {
                    scalar_text(value).map(|value| format!("{}: {value}", humanize_identifier(key)))
                })
                .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join(" · "))
        }
        other => scalar_text(other),
    }
}

fn environment_vm(info: &EnvironmentInfo) -> EnvironmentVm {
    EnvironmentVm {
        user_data_path: dash(&info.user_data_path),
        editor_install_path: dash(&info.editor_install_path),
        download_cache_path: dash(&info.download_cache_path),
        config_path: dash(&info.config_path),
        cli_version: option_dash(info.cli_version.as_deref()),
        hub_version: option_dash(info.hub_version.as_deref()),
        proxy: info.proxy.as_ref().map(proxy_vm),
    }
}

fn proxy_vm(proxy: &EnvironmentProxy) -> ProxyVm {
    let url = proxy
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(redact_proxy)
        .unwrap_or_else(|| "Not configured".to_string());

    ProxyVm {
        url,
        source: option_dash(proxy.source.as_deref()),
        auth_source: option_dash(proxy.auth_source.as_deref()),
        no_proxy: proxy
            .no_proxy
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
    }
}

fn diagnose_report_vm(topic: DiagnoseTopic, text: &str) -> DiagnoseReportVm {
    DiagnoseReportVm {
        topic,
        text: text.trim().to_string(),
    }
}

fn dash(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "—".to_string()
    } else {
        value.to_string()
    }
}

fn option_dash(value: Option<&str>) -> String {
    value.map(dash).unwrap_or_else(|| "—".to_string())
}

fn push_unavailable(out: &mut String, error: &UnityError) {
    out.push_str(&format!(
        "Unavailable: {} — {}\n",
        error.code,
        error.message.trim()
    ));
    if let Some(detail) = error.detail.as_deref().map(str::trim) {
        if !detail.is_empty() {
            out.push_str(detail);
            out.push('\n');
        }
    }
}

fn push_doctor_report(out: &mut String, read: &DoctorRead) {
    let report = &read.report;
    out.push_str(&format!(
        "Platform: {} {}\nRuntime: {}\nNode: {}\nCLI: {}\nInstall path: {}\nLog directory: {}\n",
        dash(&report.platform),
        dash(&report.arch),
        option_dash(report.runtime_version.as_deref()),
        option_dash(report.node_version.as_deref()),
        option_dash(report.cli_version.as_deref()),
        option_dash(report.install_path.as_deref()),
        option_dash(report.log_directory.as_deref()),
    ));

    if !report.checks.is_empty() {
        out.push_str("\nChecks:\n");
        for check in &report.checks {
            out.push_str(&format!(
                "[{}] {}\n",
                check_status_label(check_status(&check.status)),
                check_label(check)
            ));
            if let Some(detail) = params_detail(&check.params) {
                out.push_str(&format!("    {detail}\n"));
            }
        }
    }

    if let Some(auth) = report.auth.as_ref().and_then(redacted_value_summary) {
        out.push_str(&format!("\nAuth: {auth}\n"));
    }
    if let Some(license) = report.license.as_ref().and_then(redacted_value_summary) {
        out.push_str(&format!("License: {license}\n"));
    }

    out.push_str("\nRaw report:\n");
    out.push_str(sanitize_doctor_raw(&read.raw).trim_end());
    out.push('\n');
}

fn push_environment_report(out: &mut String, info: &EnvironmentInfo) {
    out.push_str(&format!(
        "User data: {}\nEditor installs: {}\nDownload cache: {}\nConfig: {}\nCLI: {}\nHub: {}\n",
        dash(&info.user_data_path),
        dash(&info.editor_install_path),
        dash(&info.download_cache_path),
        dash(&info.config_path),
        option_dash(info.cli_version.as_deref()),
        option_dash(info.hub_version.as_deref()),
    ));

    match &info.proxy {
        Some(proxy) => {
            let url = proxy
                .url
                .as_deref()
                .map(redact_proxy)
                .map(|url| dash(&url))
                .unwrap_or_else(|| "—".to_string());
            out.push_str(&format!(
                "Proxy: {url}\nProxy source: {}\nProxy auth source: {}\n",
                option_dash(proxy.source.as_deref()),
                option_dash(proxy.auth_source.as_deref()),
            ));
            let no_proxy = if proxy.no_proxy.is_empty() {
                "none".to_string()
            } else {
                proxy.no_proxy.join(", ")
            };
            out.push_str(&format!("No proxy: {no_proxy}\n"));
        }
        None => out.push_str("Proxy: not reported\n"),
    }
}

/// Maps the CLI's report into the page/dialog shape. Every size and reason
/// comes from the CLI document; only formatting happens here.
fn prune_report_vm(report: &PruneReport) -> PruneReportVm {
    PruneReportVm {
        unused: report
            .editors
            .iter()
            .map(|candidate| {
                prune_candidate_vm(candidate, "Reported unused by `unity editors prune`.")
            })
            .collect(),
        missing: report
            .missing
            .iter()
            .map(|candidate| {
                prune_candidate_vm(
                    candidate,
                    "Reported missing by `unity editors prune --remove-missing`.",
                )
            })
            .collect(),
        reclaimable: format_bytes(report.total),
        removed: report.removed,
    }
}

fn prune_candidate_vm(candidate: &PruneCandidate, fallback_reason: &str) -> PruneCandidateVm {
    let size = candidate
        .size
        .clone()
        .map(|size| size.trim().to_string())
        .filter(|size| !size.is_empty())
        .or_else(|| (candidate.bytes > 0).then(|| format_bytes(candidate.bytes)));
    let reason = candidate
        .reason
        .clone()
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty())
        .or_else(|| {
            candidate
                .status
                .clone()
                .filter(|status| !matches!(status.as_str(), "kept" | "removed"))
        })
        .unwrap_or_else(|| fallback_reason.to_string());

    PruneCandidateVm {
        version: candidate.version.clone(),
        architecture: candidate.architecture.clone(),
        path: candidate.path.clone(),
        size,
        reason,
    }
}

/// Names every candidate in a confirmation, so the checkbox says exactly what
/// the button will do.
fn candidate_names(candidates: &[PruneCandidateVm]) -> String {
    candidates
        .iter()
        .map(|candidate| {
            let build = if candidate.architecture.trim().is_empty() {
                "host build".to_string()
            } else {
                candidate.architecture.clone()
            };
            match &candidate.size {
                Some(size) => format!("Unity {} ({build}, {size})", candidate.version),
                None => format!("Unity {} ({build})", candidate.version),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

fn verify_result_vm(read: &EditorVerifyRead) -> VerifyResultVm {
    let report = &read.report;
    let checks: Vec<VerifyCheckVm> = report.components.iter().map(verify_check_vm).collect();
    let failed_count = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Fail)
        .count();
    let repair_command = report
        .command
        .clone()
        .map(|command| command.trim().to_string())
        .filter(|command| !command.is_empty())
        .or_else(|| checks.iter().find_map(|check| check.repair_command.clone()));

    VerifyResultVm {
        version: report.version.clone(),
        architecture: report.architecture.clone(),
        ok: report.ok,
        checks,
        failed_count,
        repair_command,
        raw: read.raw.clone(),
    }
}

fn verify_check_vm(component: &VerifyComponent) -> VerifyCheckVm {
    let component_name = component.component.trim();
    let label = if component_name.is_empty() {
        if component.kind.trim().is_empty() {
            "Component".to_string()
        } else {
            humanize_identifier(&component.kind)
        }
    } else {
        component_name.to_string()
    };

    VerifyCheckVm {
        component: label,
        kind: humanize_identifier(&component.kind),
        status: verify_status(&component.status),
        path: component
            .path
            .clone()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty()),
        repair_command: component
            .command
            .clone()
            .map(|command| command.trim().to_string())
            .filter(|command| !command.is_empty()),
    }
}

fn verify_status(status: &str) -> CheckStatus {
    match status.trim().to_ascii_lowercase().as_str() {
        "ok" | "pass" => CheckStatus::Pass,
        "missing" | "fail" => CheckStatus::Fail,
        "skipped" | "warn" => CheckStatus::Warn,
        "info" => CheckStatus::Info,
        _ => CheckStatus::Unknown,
    }
}

fn skill_client_vm(client: &SkillClientInfo) -> SkillClientVm {
    SkillClientVm {
        client: client.client.clone(),
        label: skill_client_label(&client.client),
        path: client.path.clone(),
        installed: client.is_installed(),
    }
}

fn skill_client_label(client: &str) -> String {
    match client {
        "claude-code" => "Claude Code".to_string(),
        "claude-desktop" => "Claude Desktop".to_string(),
        "grok" => "Grok".to_string(),
        "cursor" => "Cursor".to_string(),
        "windsurf" => "Windsurf".to_string(),
        "vscode" => "VS Code".to_string(),
        "cline" => "Cline".to_string(),
        "codex" => "Codex".to_string(),
        "opencode" => "OpenCode".to_string(),
        "antigravity" => "Antigravity".to_string(),
        "qwen" => "Qwen".to_string(),
        "" => "Unknown client".to_string(),
        other => humanize_identifier(other),
    }
}

fn skill_outcome_row_vm(row: &SkillRefreshRow) -> SkillOutcomeRowVm {
    SkillOutcomeRowVm {
        path: row.path.clone(),
        client: row.client.clone(),
        skill: row.skill.clone(),
        outcome: skill_outcome_vm(row.outcome()),
        skip_reason: row.reason().map(str::to_string),
    }
}

fn skill_outcome_vm(outcome: unity_core::models::SkillOutcome) -> SkillOutcomeVm {
    use unity_core::models::SkillOutcome as Cli;
    match outcome {
        Cli::Refreshed => SkillOutcomeVm::Refreshed,
        Cli::UpToDate => SkillOutcomeVm::UpToDate,
        Cli::Pruned => SkillOutcomeVm::Pruned,
        Cli::Skipped => SkillOutcomeVm::Skipped,
        Cli::Conflict => SkillOutcomeVm::Conflict,
        Cli::WouldUpdate => SkillOutcomeVm::WouldUpdate,
        Cli::WouldSkip => SkillOutcomeVm::WouldSkip,
        Cli::WouldPrune => SkillOutcomeVm::WouldPrune,
        Cli::Unknown(value) => SkillOutcomeVm::Unknown(value),
    }
}

/// Extracts the outcome rows from a streamed terminal frame.
///
/// The documented shape is `data.items[]`. Older betas answer a single-object
/// frame with `skipped: true` instead, so that flag is mapped to a single
/// `skipped` row using the request's client/skill for identity. Nothing is
/// invented: every field comes from the CLI frame or the request the user
/// clicked.
fn skill_rows_from_result(value: &Value, run: Option<&SkillRun>) -> Vec<SkillRefreshRow> {
    let data = value.get("data").cloned().unwrap_or(Value::Null);
    let parsed: Vec<SkillRefreshRow> =
        unity_core::envelope::parse_items(Some(data.clone()), &["items"]).unwrap_or_default();
    if !parsed.is_empty() {
        return parsed;
    }
    // An explicit empty `items` array is a real answer ("nothing tracked").
    if data.get("items").is_some() {
        return Vec::new();
    }

    let Some(object) = data.as_object() else {
        return Vec::new();
    };

    let skipped = object
        .get("skipped")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = ["status", "outcome", "result"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|status| !status.is_empty())
        .map(str::to_string)
        .or_else(|| skipped.then(|| "skipped".to_string()));
    let Some(status) = status else {
        return Vec::new();
    };

    let (client, skill) = match run.map(|run| &run.scope) {
        Some(SkillScope::Install { client, skill, .. }) => (
            client.clone().unwrap_or_default(),
            skill.clone().unwrap_or_default(),
        ),
        _ => (String::new(), String::new()),
    };
    let skip_reason = ["skipReason", "reason", "message"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string);

    vec![SkillRefreshRow {
        path: object
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        client,
        skill,
        status,
        skip_reason,
        extra: Map::new(),
    }]
}

fn editor_row_vm(editor: &Editor) -> EditorRowVm {
    EditorRowVm {
        version: editor.version.clone(),
        architecture: editor.architecture.clone(),
        is_default: editor.default,
        modules: editor.module_names.clone(),
    }
}

/// The install card's build choices. `None` is deliberately first and is the
/// boot default, so an existing single-build install keeps its old command.
fn architecture_options(selected: Option<&str>) -> Vec<ArchitectureOptionVm> {
    let selected = selected.map(str::trim).filter(|value| !value.is_empty());
    [None, Some("x86_64"), Some("arm64")]
        .into_iter()
        .map(|architecture| ArchitectureOptionVm {
            label: architecture.unwrap_or("Host default").to_string(),
            selected: architecture == selected,
            architecture: architecture.map(str::to_string),
        })
        .collect()
}

fn project_row_vm(project: &Project, installed: &HashSet<(&str, &str)>) -> ProjectRowVm {
    let mut badges = Vec::new();
    if project.cloud_enabled {
        badges.push(BadgeVm {
            label: "Unity Cloud".to_string(),
            tone: Tone::Info,
        });
    }
    if let Some(vcs) = &project.vcs_provider {
        badges.push(BadgeVm {
            label: vcs.clone(),
            tone: Tone::Neutral,
        });
    }
    if !installed.is_empty()
        && !installed_build_matches(installed, &project.version, &project.architecture)
    {
        badges.push(BadgeVm {
            label: "Editor not installed".to_string(),
            tone: Tone::Warning,
        });
    }
    if badges.is_empty() {
        badges.push(BadgeVm {
            label: "local project".to_string(),
            tone: Tone::Neutral,
        });
    }

    ProjectRowVm {
        path: project.path.clone(),
        title: project.title.clone(),
        version: project.version.clone(),
        architecture: project.architecture.clone(),
        pipeline: project.render_pipeline.as_ref().map(|pipeline| BadgeVm {
            label: pipeline.clone(),
            tone: Tone::Highlight,
        }),
        build_target: build_target_label(project.build_target.as_deref()),
        short_path: short_path(&project.path, 30).into_owned(),
        modified: format_epoch_millis(project.last_modified),
        is_favorite: project.is_favorite,
        badges,
    }
}

fn build_target_label(target: Option<&str>) -> String {
    let Some(target) = target.map(str::trim).filter(|target| !target.is_empty()) else {
        return "—".to_string();
    };

    let lower = target.to_ascii_lowercase();
    if lower.contains("windows") {
        "Windows".to_string()
    } else if lower.contains("linux") {
        "Linux".to_string()
    } else if lower.contains("osx") || lower.contains("macos") {
        "macOS".to_string()
    } else if lower.contains("android") {
        "Android".to_string()
    } else if lower == "ios" {
        "iOS".to_string()
    } else if lower.contains("webgl") {
        "WebGL".to_string()
    } else if lower.contains("tvos") {
        "tvOS".to_string()
    } else {
        target.to_string()
    }
}

/// Whether an installed build satisfies a row that names a version and an
/// architecture. A CLI that omits the architecture (older rows) falls back to
/// a version-only match instead of showing a false "not installed" badge.
fn installed_build_matches(
    installed: &HashSet<(&str, &str)>,
    version: &str,
    architecture: &str,
) -> bool {
    let architecture = architecture.trim();
    if architecture.is_empty() {
        installed
            .iter()
            .any(|(installed_version, _)| *installed_version == version)
    } else {
        installed.contains(&(version, architecture))
    }
}

fn stream_tone(stream: &str) -> Tone {
    let stream = stream.to_lowercase();
    if stream.contains("lts") || stream.contains("supported") {
        Tone::Success
    } else if stream.contains("beta") {
        Tone::Warning
    } else if stream.contains("alpha") {
        Tone::Danger
    } else {
        Tone::Neutral
    }
}

fn section_kind(title: &str) -> SectionKind {
    let title = title.to_lowercase();
    if title.contains("removed") {
        SectionKind::Removed
    } else if title.contains("changed") || title.contains("deprecated") {
        SectionKind::Changed
    } else if title.contains("added") {
        SectionKind::Added
    } else if title.contains("security") {
        SectionKind::Security
    } else if title.contains("fixed") {
        SectionKind::Fixed
    } else {
        SectionKind::Other
    }
}

fn load_status(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || Ok(cli.status())),
        Message::StatusLoaded,
    )
}

fn load_update(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.self_update_check()),
        Message::UpdateLoaded,
    )
}

fn load_update_on(cli: UnityCli, channel: Option<String>) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.self_update_check_on(channel.as_deref())),
        Message::UpdateLoaded,
    )
}

fn load_auth(cli: UnityCli) -> Task<Message> {
    Task::perform(run_blocking(move || cli.auth_status()), Message::AuthLoaded)
}

fn load_license(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.license_status()),
        Message::LicenseLoaded,
    )
}

fn load_running(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.editors_running()),
        Message::RunningLoaded,
    )
}

fn load_editors(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.editors_installed()),
        Message::EditorsLoaded,
    )
}

fn load_new_project_editors(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.editors_installed()),
        Message::NewProjectEditorsLoaded,
    )
}

fn load_new_project_license(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.license_status()),
        Message::NewProjectLicenseLoaded,
    )
}

fn load_releases(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.releases(20, 0, None)),
        Message::ReleasesLoaded,
    )
}

fn load_projects(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.projects()),
        Message::ProjectsLoaded,
    )
}

/// Opens the operating system's folder picker. `rfd` hands the work to the
/// platform's native dialog, so the iced event loop stays responsive.
async fn pick_project_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Add an existing Unity project")
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}

fn load_doctor(cli: UnityCli) -> Task<Message> {
    Task::perform(run_blocking(move || cli.doctor()), Message::DoctorLoaded)
}

fn load_environment(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.environment()),
        Message::EnvironmentLoaded,
    )
}

fn load_skill_clients(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.skill_clients()),
        Message::SkillClientsLoaded,
    )
}

fn load_skill_installs(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.skills()),
        Message::SkillInstallsLoaded,
    )
}

fn load_diagnose(cli: UnityCli, topic: CliDiagnose) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.diagnose(topic)),
        move |result| match topic {
            CliDiagnose::Proxy => Message::DiagnoseProxyLoaded(result),
            CliDiagnose::Accelerator => Message::DiagnoseAcceleratorLoaded(result),
            CliDiagnose::Update => Message::DiagnoseUpdateLoaded(result),
        },
    )
}

async fn load_changelog_future(
    cli: UnityCli,
    target: Option<String>,
) -> Result<ChangelogView, UnityError> {
    run_blocking(move || {
        let provider = CliChangelogProvider::new(cli);
        ChangelogView::load(&provider, &ChangelogRequest { target })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use unity_core::changelog::ChangelogSection;
    use unity_core::models::{CliVersion, VerifyReport};

    fn project(title: &str, path: &str, version: &str) -> Project {
        Project {
            title: title.to_string(),
            path: path.to_string(),
            version: version.to_string(),
            ..Project::default()
        }
    }

    #[test]
    fn project_filter_uses_the_precomputed_index_and_returns_original_indices() {
        let (mut app, _) = App::boot();
        let projects = vec![
            project("Alpha Game", "/games/alpha", "6000.1.1f1"),
            project("Beta Tool", "/tools/beta", "6000.2.2f1"),
            project("Gamma", "/games/gamma", "6000.3.3f1"),
        ];
        app.project_search_index = project_search_keys(&projects);
        app.projects.data = Some(projects);
        app.project_search = "beta".to_string();

        let filtered = app.filtered_projects();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, 1);
        assert_eq!(filtered[0].1.title, "Beta Tool");
    }

    #[test]
    fn empty_project_search_borrows_every_project() {
        let (mut app, _) = App::boot();
        let projects = vec![project("Alpha", "/alpha", "6000.1.1f1")];
        app.project_search_index = project_search_keys(&projects);
        app.projects.data = Some(projects);

        let filtered = app.filtered_projects();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, 0);
    }

    #[test]
    fn view_model_maps_cli_status_and_filtered_projects() {
        let (mut app, _) = App::boot();
        app.cli_status.data = Some(CliStatus {
            installed: true,
            path: Some("/usr/bin/unity".to_string()),
            path_source: Some("PATH".to_string()),
            version: Some(CliVersion {
                version: "1.0.0".to_string(),
                channel: "stable".to_string(),
                platform: "linux".to_string(),
                arch: "x86_64".to_string(),
                ..CliVersion::default()
            }),
            ..CliStatus::default()
        });
        app.page = Page::Overview;

        let vm = app.view_model();
        assert!(matches!(
            &vm.shell.cli,
            CliConnectionVm::Connected {
                version: Some(version)
            } if version == "1.0.0"
        ));
        match &vm.page {
            PageVm::Overview(overview) => {
                match &overview.cli {
                    CliCardVm::Ready { details, .. } => {
                        assert_eq!(details.channel, "stable");
                        assert_eq!(details.platform, "linux x86_64");
                    }
                    other => panic!("unexpected CLI card: {other:?}"),
                }
                assert_eq!(overview.editor_count, None);
                assert_eq!(overview.project_count, None);
            }
            other => panic!("unexpected page: {other:?}"),
        }

        let projects = vec![
            project("Alpha Game", "/games/alpha", "6000.1.1f1"),
            project("Beta Tool", "/tools/beta", "6000.2.2f1"),
        ];
        app.project_search_index = project_search_keys(&projects);
        app.projects.data = Some(projects);
        app.project_search = "beta".to_string();
        app.page = Page::Projects;

        let vm = app.view_model();
        match &vm.page {
            PageVm::Projects(projects) => {
                assert_eq!(projects.total_count, 2);
                assert_eq!(projects.visible_count, 1);
                let rows = projects.projects.data().expect("filtered rows");
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].title, "Beta Tool");
                assert_eq!(rows[0].badges[0].label, "local project");
            }
            other => panic!("unexpected page: {other:?}"),
        }
    }

    #[test]
    fn favorite_projects_map_to_the_star_not_a_badge() {
        let mut favorite = project("Alpha", "/games/alpha", "6000.1.1f1");
        favorite.is_favorite = true;
        let row = project_row_vm(&favorite, &HashSet::new());
        assert!(row.is_favorite);
        assert!(!row.badges.iter().any(|badge| badge.label == "favorite"));

        let row = project_row_vm(
            &project("Alpha", "/games/alpha", "6000.1.1f1"),
            &HashSet::new(),
        );
        assert!(!row.is_favorite);
    }

    #[test]
    fn view_model_maps_rich_running_editor_status() {
        let (mut app, _) = App::boot();
        app.page = Page::Overview;
        app.running.data = Some(RunningEditors {
            count: 1,
            instances: vec![RunningEditor {
                project_path: Some("/games/orbital".to_string()),
                editor_version: Some("6000.6.6f1".to_string()),
                pid: Some(4242),
                phase: Some("safe mode".to_string()),
                pipeline_version: Some("0.9.1-exp.1".to_string()),
                update_available: true,
                safe_mode: Some(serde_json::json!({ "detected": true })),
                dialog: Some(serde_json::json!({ "title": "Save changes?" })),
                ..RunningEditor::default()
            }],
        });

        let vm = app.view_model();
        match &vm.page {
            PageVm::Overview(overview) => {
                let editors = overview.running.data().expect("running editors");
                assert_eq!(editors.len(), 1);
                assert_eq!(editors[0].phase.as_deref(), Some("safe mode"));
                assert_eq!(editors[0].pipeline_version.as_deref(), Some("0.9.1-exp.1"));
                assert!(editors[0].update_available);
                assert!(editors[0].safe_mode);
                assert_eq!(editors[0].dialog.as_deref(), Some("Save changes?"));
            }
            other => panic!("unexpected page: {other:?}"),
        }
    }

    #[test]
    fn stale_auth_sessions_are_flagged_in_the_account_card() {
        let (mut app, _) = App::boot();
        app.page = Page::Overview;
        app.auth.data = Some(AuthStatus {
            logged_in: true,
            session_state: Some("stale".to_string()),
            ..AuthStatus::default()
        });

        let vm = app.view_model();
        match &vm.page {
            PageVm::Overview(overview) => match &overview.account.auth {
                AuthVm::SignedIn { stale, .. } => assert!(*stale),
                other => panic!("unexpected auth card: {other:?}"),
            },
            other => panic!("unexpected page: {other:?}"),
        }
    }

    #[test]
    fn upgrade_guard_matches_running_editor_by_project_path() {
        let (mut app, _) = App::boot();
        app.running.data = Some(RunningEditors {
            count: 1,
            instances: vec![RunningEditor {
                project_path: Some("/games/orbital/".to_string()),
                ..RunningEditor::default()
            }],
        });

        assert!(app.running_editor_for_project("/games/orbital").is_some());
        assert!(app.running_editor_for_project("/games/other").is_none());
    }

    #[test]
    fn opening_upgrade_dialog_starts_a_running_editor_refresh() {
        let (mut app, _) = App::boot();
        app.projects.data = Some(vec![project("Alpha", "/games/alpha", "6000.1.1f1")]);
        app.editors.data = Some(vec![Editor {
            version: "6000.2.2f1".to_string(),
            ..Editor::default()
        }]);

        let _task = app.update(Message::OpenUpgrade("/games/alpha".to_string()));

        assert!(app.running.loading);
        assert!(matches!(app.modal, Modal::UpgradeProject(_)));
    }

    #[test]
    fn headless_install_option_follows_the_platform() {
        let (mut app, _) = App::boot();
        app.page = Page::Editors;
        let _ = app.update(Message::HeadlessToggled(true));

        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        let supported = unity_core::runner::headless_editor_supported();
        assert_eq!(editors.install.headless_available, supported);
        assert_eq!(editors.install.headless, supported);
    }

    #[test]
    fn module_dialog_keeps_the_architecture_of_the_opened_build() {
        let (mut app, _) = App::boot();
        app.modal = Modal::EditorModules(Editor {
            version: "6000.6.6f1".to_string(),
            architecture: "arm64".to_string(),
            ..Editor::default()
        });

        let vm = app.view_model();
        let Some(ModalVm::EditorModules(modules)) = vm.modal else {
            panic!("expected the modules dialog");
        };
        assert_eq!(modules.editor_version, "6000.6.6f1");
        assert_eq!(modules.architecture, "arm64");
    }

    fn editor(version: &str, architecture: &str, is_default: bool) -> Editor {
        Editor {
            version: version.to_string(),
            architecture: architecture.to_string(),
            default: is_default,
            ..Editor::default()
        }
    }

    #[test]
    fn two_builds_of_one_version_are_two_rows_and_target_the_clicked_build() {
        let (mut app, _) = App::boot();
        app.page = Page::Editors;
        app.editors.data = Some(vec![
            editor("6000.1.1f1", "arm64", true),
            editor("6000.1.1f1", "x86_64", false),
        ]);

        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        let rows = editors.installed.data().expect("installed rows");
        assert_eq!(rows.len(), 2, "a version must not collapse two builds");
        assert_eq!(rows[0].version, rows[1].version);
        assert_eq!(rows[0].architecture, "arm64");
        assert_eq!(rows[1].architecture, "x86_64");
        assert!(rows[0].is_default);
        assert!(!rows[1].is_default);

        let _task = app.update(Message::OpenModules(
            "6000.1.1f1".to_string(),
            "x86_64".to_string(),
        ));
        let Modal::EditorModules(opened) = &app.modal else {
            panic!("expected the modules dialog");
        };
        assert_eq!(opened.architecture, "x86_64");

        // A stale action that names a build the list no longer has must not
        // fall back to another build of the same version.
        let _task = app.update(Message::OpenModules(
            "6000.1.1f1".to_string(),
            "ppc64".to_string(),
        ));
        let Modal::EditorModules(opened) = &app.modal else {
            panic!("expected the original modules dialog");
        };
        assert_eq!(opened.architecture, "x86_64");
    }

    #[test]
    fn install_architecture_choice_reaches_the_install_command() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(temporary_cli()));
        app.install_version = "6000.1.1f1".to_string();
        let _ = app.update(Message::InstallArchitectureChanged(Some(
            "arm64".to_string(),
        )));

        let spec = app.install_editor_spec().expect("install spec");
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["--architecture", "arm64"]));

        let _ = app.update(Message::InstallArchitectureChanged(None));
        let spec = app
            .install_editor_spec()
            .expect("host-default install spec");
        assert!(
            !spec.args.iter().any(|arg| arg == "--architecture"),
            "host default must keep the command unchanged"
        );
    }

    #[test]
    fn an_ambiguous_default_is_explained_instead_of_guessed() {
        let (mut app, _) = App::boot();
        app.editors.data = Some(vec![
            editor("6000.1.1f1", "arm64", false),
            editor("6000.1.1f1", "x86_64", false),
        ]);

        let _ = app.update(Message::SetDefaultEditor(
            "6000.1.1f1".to_string(),
            String::new(),
        ));

        assert!(app.active_job.is_none(), "no command may be started");
        // A refusal lands as a notice, which borrows the toast slot (id 0).
        let toast = app
            .view_model()
            .shell
            .toast
            .expect("the ambiguity must be explained");
        assert!(toast.message.contains("arm64"), "{}", toast.message);
        assert!(toast.message.contains("x86_64"), "{}", toast.message);

        // The same version with a named build is unambiguous and builds the
        // command the row's action sends.
        app.cli.set_override_path(Some(temporary_cli()));
        let spec = app
            .editor_default_spec("6000.1.1f1", "arm64")
            .expect("the build-specific default");
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["--architecture", "arm64"]));
    }

    // ------------------------------------------------------------------
    // Editor maintenance: prune, verify, default

    /// A harmless executable for tests that must reach `start_job`. Never the
    /// real `unity` binary: no test may run a CLI mutation.
    fn startable_cli() -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from("cmd.exe")
        }
        #[cfg(not(windows))]
        {
            PathBuf::from("/bin/sh")
        }
    }

    fn prune_candidate(version: &str, architecture: &str, bytes: u64) -> PruneCandidate {
        PruneCandidate {
            version: version.to_string(),
            architecture: architecture.to_string(),
            path: format!("/home/ada/Unity/{version}"),
            bytes,
            ..PruneCandidate::default()
        }
    }

    fn prune_report() -> PruneReport {
        PruneReport {
            removed: false,
            editors: vec![
                prune_candidate("6000.3.27f2", "x86_64", 8_688_108_322),
                prune_candidate("2022.3.10f1", "arm64", 5_690_000_000),
            ],
            total: 13_400_000_000,
            missing: vec![PruneCandidate {
                version: "6000.0.0b1".to_string(),
                architecture: "x86_64".to_string(),
                path: "/gone/6000.0.0b1".to_string(),
                ..PruneCandidate::default()
            }],
        }
    }

    fn verify_read(ok: bool) -> EditorVerifyRead {
        EditorVerifyRead {
            report: VerifyReport {
                version: "6000.6.6f1".to_string(),
                architecture: "arm64".to_string(),
                ok,
                components: vec![
                    VerifyComponent {
                        component: "editor".to_string(),
                        kind: "editor".to_string(),
                        status: "ok".to_string(),
                        path: Some("/home/ada/Unity/6000.6.6f1/Editor/Unity".to_string()),
                        ..VerifyComponent::default()
                    },
                    VerifyComponent {
                        component: "windows-mono".to_string(),
                        kind: "module".to_string(),
                        status: "missing".to_string(),
                        path: Some("/home/ada/Unity/6000.6.6f1/WindowsSupport".to_string()),
                        command: Some(
                            "unity install-modules --editor-version 6000.6.6f1 --architecture arm64 -m windows-mono --yes"
                                .to_string(),
                        ),
                        ..VerifyComponent::default()
                    },
                    VerifyComponent {
                        component: "ios".to_string(),
                        kind: "module".to_string(),
                        status: "skipped".to_string(),
                        ..VerifyComponent::default()
                    },
                ],
                ..VerifyReport::default()
            },
            raw: "{\"success\":false,\"data\":{\"ok\":false}}".to_string(),
        }
    }

    #[test]
    fn prune_commands_are_report_only_until_a_removal_is_explicit() {
        let app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(temporary_cli()));

        let report = app.cli.editors_prune_report_spec().expect("report spec");
        assert_eq!(
            &report.args[..3],
            &[
                "editors".to_string(),
                "prune".to_string(),
                "--remove-missing".to_string()
            ]
        );
        assert!(!report.args.iter().any(|arg| arg == "--remove"));
        assert!(!report.args.iter().any(|arg| arg == "--yes"));
        assert!(report
            .args
            .windows(2)
            .any(|pair| pair == ["--format", "json"]));

        let remove = app
            .cli
            .editors_prune_remove_spec(false)
            .expect("remove spec");
        assert_eq!(
            &remove.args[..4],
            &[
                "editors".to_string(),
                "prune".to_string(),
                "--remove".to_string(),
                "--yes".to_string()
            ]
        );
        assert!(remove
            .args
            .windows(2)
            .any(|pair| pair == ["--format", "ndjson"]));

        let forced = app
            .cli
            .editors_prune_remove_spec(true)
            .expect("forced remove spec");
        assert!(forced.args.iter().any(|arg| arg == "--force"));

        let missing = app
            .cli
            .editors_prune_remove_missing_spec()
            .expect("missing spec");
        assert!(missing.args.iter().any(|arg| arg == "--remove-missing"));
        assert!(!missing.args.iter().any(|arg| arg == "--remove"));
        assert!(missing.args.iter().any(|arg| arg == "--yes"));
    }

    #[test]
    fn cancelling_the_prune_review_removes_nothing() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.editors.data = Some(vec![editor("6000.1.1f1", "x86_64", false)]);

        let _ = app.update(Message::OpenPrune);
        assert!(matches!(app.modal, Modal::PruneEditors));
        assert!(app.prune.loading);

        let _ = app.update(Message::CloseModal);

        assert!(matches!(app.modal, Modal::None));
        assert!(
            app.active_job.is_none(),
            "closing the review must not start a removal"
        );
    }

    #[test]
    fn prune_requires_its_own_confirmation_and_names_the_candidates() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.prune.data = Some(prune_report());
        app.modal = Modal::PruneEditors;

        let _ = app.update(Message::StartPruneRemove);
        assert!(
            app.active_job.is_none(),
            "the unconfirmed button must remove nothing"
        );

        let _ = app.update(Message::PruneConfirmed(true));
        let vm = app.view_model();
        let Some(ModalVm::PruneEditors(prune)) = vm.modal else {
            panic!("expected the prune dialog");
        };
        assert!(prune.can_remove);
        assert_eq!(prune.remove_label, "Remove 2 unused Editors (12.5 GB)");
        assert!(
            prune.confirm_label.contains("6000.3.27f2"),
            "{}",
            prune.confirm_label
        );
        assert!(
            prune.confirm_label.contains("2022.3.10f1"),
            "{}",
            prune.confirm_label
        );
        assert!(prune.remove_command.contains("--remove"));
        assert!(!prune.remove_command.contains("--force"));

        let _ = app.update(Message::StartPruneRemove);
        let job = app.active_job.as_ref().expect("confirmed removal starts");
        assert_eq!(job.kind, "editor-prune");
        assert!(job.command.contains("--remove"));
    }

    #[test]
    fn a_failed_or_in_flight_rescan_cannot_arm_or_start_removal() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.prune.data = Some(prune_report());
        app.modal = Modal::PruneEditors;

        // Tick the confirmation while a rescan is in flight: it must not arm.
        app.prune.begin();
        let _ = app.update(Message::PruneConfirmed(true));
        assert!(!app.prune_confirmed);
        let _ = app.update(Message::PruneMissingConfirmed(true));
        assert!(!app.prune_missing_confirmed);

        // A failed rescan keeps the previous report in `Loadable::data`, which
        // must never be treated as the report the user can act on.
        app.prune.failed(UnityError::new(
            "NETWORK_UNREACHABLE",
            "The prune report could not be read.",
        ));
        app.prune_confirmed = true;
        let _ = app.update(Message::StartPruneRemove);
        assert!(
            app.active_job.is_none(),
            "a failed report must not run --remove"
        );
        let _ = app.update(Message::PruneMissingConfirmed(true));
        assert!(
            !app.prune_missing_confirmed,
            "a failed report cannot arm a confirmation"
        );
        let _ = app.update(Message::StartPruneRemoveMissing);
        assert!(app.active_job.is_none());

        let vm = app.view_model();
        let Some(ModalVm::PruneEditors(prune)) = vm.modal else {
            panic!("expected the prune dialog");
        };
        assert!(
            prune.report.data().is_some(),
            "the stale report is still present for display"
        );
        assert!(!prune.can_remove);
        assert!(!prune.can_remove_missing);
    }

    #[test]
    fn a_running_prune_job_is_visible_in_the_dialog_and_blocks_its_buttons() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.prune.data = Some(prune_report());
        app.prune_confirmed = true;
        app.modal = Modal::PruneEditors;
        app.active_job = Some(ActiveJob {
            id: "job-test".to_string(),
            kind: "editor-prune".to_string(),
            command: "unity editors prune --remove --yes".to_string(),
            state: JobState::Running,
            progress: 20.0,
            downloads: BTreeMap::new(),
            message: Some("Removing 6000.3.27f2…".to_string()),
            logs: VecDeque::new(),
            log_text: Arc::from(""),
            error: None,
            result: None,
        });

        let vm = app.view_model();
        let Some(ModalVm::PruneEditors(prune)) = vm.modal else {
            panic!("expected the prune dialog");
        };
        assert!(prune.job_running);
        assert!(!prune.can_remove);
        assert!(!prune.can_remove_missing);
    }

    #[test]
    fn remove_missing_is_a_separate_registry_only_action() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.prune.data = Some(prune_report());
        app.modal = Modal::PruneEditors;

        let _ = app.update(Message::StartPruneRemoveMissing);
        assert!(app.active_job.is_none());

        let _ = app.update(Message::PruneMissingConfirmed(true));
        let vm = app.view_model();
        let Some(ModalVm::PruneEditors(prune)) = vm.modal else {
            panic!("expected the prune dialog");
        };
        assert!(prune.can_remove_missing);
        assert!(
            prune.missing_label.contains("no files deleted"),
            "{}",
            prune.missing_label
        );
        assert!(prune.missing_command.contains("--remove-missing"));
        assert!(!prune.missing_command.contains("--remove "));
        assert!(
            !prune.can_remove,
            "the missing confirmation is not the file one"
        );

        let _ = app.update(Message::StartPruneRemoveMissing);
        let job = app.active_job.as_ref().expect("confirmed registry action");
        assert_eq!(job.kind, "editor-prune-missing");
    }

    #[test]
    fn force_is_only_offered_after_a_failed_running_editor_check() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.prune.data = Some(prune_report());
        app.modal = Modal::PruneEditors;

        let _ = app.update(Message::PruneForceToggled(true));
        assert!(!app.prune_force, "force must not be selectable up front");

        app.prune_force_available = true;
        let _ = app.update(Message::PruneForceToggled(true));
        assert!(app.prune_force);
        let vm = app.view_model();
        let Some(ModalVm::PruneEditors(prune)) = vm.modal else {
            panic!("expected the prune dialog");
        };
        assert!(prune.remove_command.contains("--force"));

        assert!(prune_force_available_for(&UnityError::new(
            "EDITORS_PRUNE_EDITOR_CHECK_FAILED",
            "Could not detect running Editors."
        )));
        assert!(prune_force_available_for(&UnityError::new(
            "EDITORS_PRUNE_EDITOR_CHECK_UNAVAILABLE",
            "Could not detect running Editors."
        )));
        assert!(!prune_force_available_for(&UnityError::new(
            "INSTALL_LOCKED",
            "Another process holds the lock."
        )));
    }

    #[test]
    fn prune_report_maps_every_candidate_from_the_cli_document() {
        let (mut app, _) = App::boot();
        app.page = Page::Editors;
        app.prune.data = Some(prune_report());

        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        let report = editors.prune.data().expect("prune report");
        assert_eq!(report.unused.len(), 2);
        assert_eq!(report.unused[0].version, "6000.3.27f2");
        assert_eq!(report.unused[0].architecture, "x86_64");
        assert_eq!(report.unused[0].size.as_deref(), Some("8.1 GB"));
        assert!(report.unused[0].reason.contains("unused"));
        assert_eq!(report.reclaimable, "12.5 GB");
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].size, None);
        assert!(report.missing[0].reason.contains("missing"));
        assert!(!report.removed);
    }

    #[test]
    fn verifying_two_builds_targets_the_clicked_build_or_nothing() {
        let (mut app, _) = App::boot();
        app.editors.data = Some(vec![
            editor("6000.1.1f1", "arm64", false),
            editor("6000.1.1f1", "x86_64", false),
        ]);

        let _ = app.update(Message::OpenVerify(
            "6000.1.1f1".to_string(),
            "x86_64".to_string(),
        ));
        let Modal::VerifyEditor {
            version,
            architecture,
        } = &app.modal
        else {
            panic!("expected the verify dialog");
        };
        assert_eq!(version, "6000.1.1f1");
        assert_eq!(architecture, "x86_64");
        assert_eq!(
            app.retry_targets.get("read:editor-verify"),
            Some(&RetryRequest::EditorVerify {
                version: "6000.1.1f1".to_string(),
                architecture: "x86_64".to_string(),
            })
        );

        // A stale action naming a build this version never had must not verify
        // the other build.
        let _ = app.update(Message::OpenVerify(
            "6000.1.1f1".to_string(),
            "ppc64".to_string(),
        ));
        let Modal::VerifyEditor { architecture, .. } = &app.modal else {
            panic!("the original dialog stays open");
        };
        assert_eq!(architecture, "x86_64");
    }

    #[test]
    fn verify_results_map_status_path_and_raw_log() {
        let (mut app, _) = App::boot();
        app.verify.data = Some(verify_read(false));
        app.verify_show_raw = true;
        app.modal = Modal::VerifyEditor {
            version: "6000.6.6f1".to_string(),
            architecture: "arm64".to_string(),
        };

        let vm = app.view_model();
        let Some(ModalVm::VerifyEditor(verify)) = vm.modal else {
            panic!("expected the verify dialog");
        };
        assert!(verify.show_raw);
        let result = verify.state.data().expect("verify result");
        assert!(!result.ok);
        assert_eq!(result.failed_count, 1);
        assert_eq!(result.checks[0].status, CheckStatus::Pass);
        assert_eq!(result.checks[1].status, CheckStatus::Fail);
        assert_eq!(result.checks[2].status, CheckStatus::Warn);
        assert_eq!(
            result.checks[1].path.as_deref(),
            Some("/home/ada/Unity/6000.6.6f1/WindowsSupport")
        );
        assert!(result.checks[1].repair_command.is_some());
        assert_eq!(
            result.repair_command.as_deref(),
            result.checks[1].repair_command.as_deref()
        );
        assert!(result.raw.contains("\"ok\":false"));
    }

    #[test]
    fn clearing_the_default_leaves_no_badge_and_set_targets_the_build() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.page = Page::Editors;
        app.editors.data = Some(vec![editor("6000.1.1f1", "arm64", true)]);

        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        assert!(editors.installed.data().unwrap()[0].is_default);

        let _ = app.update(Message::SetDefaultEditor(
            "6000.1.1f1".to_string(),
            "arm64".to_string(),
        ));
        let job = app.active_job.as_ref().expect("set default job");
        assert_eq!(job.kind, "editor-default");
        assert!(job.command.contains("--architecture"));
        assert!(job.command.contains("arm64"));

        app.active_job = None;
        let _ = app.update(Message::ClearDefaultEditor);
        let job = app.active_job.as_ref().expect("clear default job");
        assert_eq!(job.kind, "editor-default");
        assert!(job.command.contains("editors default --unset"));

        // The refreshed list the CLI will return has no default row, so the
        // page shows no badge.
        app.active_job = None;
        app.editors.data = Some(vec![editor("6000.1.1f1", "arm64", false)]);
        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        assert!(!editors.installed.data().unwrap()[0].is_default);
    }

    #[test]
    fn project_badges_and_upgrade_options_match_version_and_architecture() {
        let (mut app, _) = App::boot();
        app.page = Page::Projects;
        app.editors.data = Some(vec![
            editor("6000.1.1f1", "arm64", true),
            editor("6000.3.0f1", "arm64", false),
            editor("6000.3.0f1", "x86_64", false),
        ]);
        let projects = vec![
            Project {
                architecture: "arm64".to_string(),
                ..project("Arm project", "/games/arm", "6000.1.1f1")
            },
            Project {
                architecture: "x86_64".to_string(),
                ..project("Intel project", "/games/intel", "6000.1.1f1")
            },
        ];
        app.project_search_index = project_search_keys(&projects);
        app.projects.data = Some(projects);

        let vm = app.view_model();
        let PageVm::Projects(projects) = &vm.page else {
            panic!("expected the Projects page");
        };
        let rows = projects.projects.data().expect("project rows");
        assert!(
            !rows[0]
                .badges
                .iter()
                .any(|badge| badge.label == "Editor not installed"),
            "the arm64 build satisfies the arm64 project"
        );
        assert!(
            rows[1]
                .badges
                .iter()
                .any(|badge| badge.label == "Editor not installed"),
            "an x86_64 project is not satisfied by the arm64 build"
        );

        let _ = app.update(Message::OpenUpgrade("/games/arm".to_string()));
        let vm = app.view_model();
        let Some(ModalVm::UpgradeProject(upgrade)) = vm.modal else {
            panic!("expected the upgrade dialog");
        };
        assert_eq!(upgrade.target_options, vec!["6000.3.0f1".to_string()]);
        let note = upgrade
            .target_note
            .expect("two target builds are explained");
        assert!(note.contains("arm64"), "{note}");
        assert!(note.contains("x86_64"), "{note}");
    }

    #[test]
    fn projects_force_load_starts_a_running_editor_refresh() {
        let (mut app, _) = App::boot();

        let _task = app.load_projects_page(true);

        assert!(app.running.loading);
    }

    fn active_job() -> ActiveJob {
        ActiveJob {
            id: "job".to_string(),
            kind: "editor-install".to_string(),
            command: "unity install lts".to_string(),
            state: JobState::Running,
            progress: 0.0,
            downloads: BTreeMap::new(),
            message: None,
            logs: VecDeque::new(),
            log_text: Arc::from(""),
            error: None,
            result: None,
        }
    }

    // ------------------------------------------------------------------
    // CLI advisories

    fn connected_app(version: &str) -> App {
        let (mut app, _) = App::boot();
        app.cli_status = Loadable::default();
        app.cli_status.data = Some(CliStatus {
            installed: true,
            version: Some(CliVersion {
                version: version.to_string(),
                ..CliVersion::default()
            }),
            ..CliStatus::default()
        });
        app
    }

    fn absorb(app: &mut App, envelope: Value) {
        let envelope: unity_core::envelope::ValueEnvelope =
            serde_json::from_value(envelope).expect("envelope");
        app.cli
            .advisory_sink()
            .push(unity_core::advisory::advisories_from_envelope(&envelope));
        app.absorb_advisories();
    }

    fn update_advisory() -> Value {
        serde_json::json!({
            "success": true,
            "notifications": [{
                "code": "CLI_UPDATE_AVAILABLE",
                "message": "A new Unity CLI is available.",
                "data": { "current": "1.0.0-beta.16", "latest": "1.0.0-beta.17" },
                "remediation": { "command": "unity self-update", "requiresUserApproval": true }
            }]
        })
    }

    fn temporary_cli() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "unity-rust-hub-advisory-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::write(&path, b"test").expect("temp binary");
        path
    }

    #[test]
    fn duplicate_advisories_from_a_refresh_collapse_into_one_notification() {
        let mut app = connected_app("1.0.0-beta.17");

        absorb(&mut app, update_advisory());
        absorb(&mut app, update_advisory());
        absorb(&mut app, update_advisory());

        let notifications = app.view_model().shell.notifications;
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "A new Unity CLI is available.");
    }

    #[test]
    fn update_advisory_becomes_a_copy_only_notification_entry() {
        let mut app = connected_app("1.0.0-beta.16");

        absorb(&mut app, update_advisory());

        let vm = app.view_model();
        assert_eq!(vm.shell.notifications.len(), 1);
        let entry = &vm.shell.notifications[0];
        // The CLI's code becomes the entry label; the message stays verbatim.
        assert_eq!(entry.label, "CLI_UPDATE_AVAILABLE");
        assert_eq!(entry.message, "A new Unity CLI is available.");
        assert_eq!(entry.command.as_deref(), Some("unity self-update"));
        // The CLI asked for an explicit human decision, so the command stays
        // copy-only: the app never runs it on a single click.
        assert!(!entry.can_run);
    }

    #[test]
    fn unknown_codes_and_object_warnings_render_their_message() {
        let mut app = connected_app("1.0.0-beta.17");

        absorb(
            &mut app,
            serde_json::json!({
                "success": true,
                "notifications": [{ "code": "FUTURE_CODE", "message": "Something new happened." }]
            }),
        );
        absorb(
            &mut app,
            serde_json::json!({
                "success": true,
                "warnings": [{ "message": "The --skip flag is deprecated." }]
            }),
        );

        let notifications = app.view_model().shell.notifications;
        assert_eq!(notifications.len(), 2);
        // The rail is newest-first: the second absorb is on top.
        assert_eq!(notifications[0].label, "Warning");
        assert_eq!(notifications[0].message, "The --skip flag is deprecated.");
        assert_eq!(notifications[1].label, "FUTURE_CODE");
        assert_eq!(notifications[1].message, "Something new happened.");
        assert!(notifications[1].command.is_none());
    }

    #[test]
    fn plugin_beta_warning_from_a_job_frame_reaches_the_shell() {
        let mut app = connected_app("1.0.0-beta.17");
        absorb(
            &mut app,
            serde_json::json!({
                "type": "result",
                "success": true,
                "data": { "plugin": "com.unity.ai" },
                "warnings": ["This plugin is a beta release. Beta plugins can break projects."]
            }),
        );

        let notifications = app.view_model().shell.notifications;
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].label, "Warning");
        assert_eq!(
            notifications[0].message,
            "This plugin is a beta release. Beta plugins can break projects."
        );
        assert!(notifications[0].command.is_none());
    }

    #[test]
    fn failed_read_gets_a_retry_action_with_a_controller_target() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Projects;
        app.projects.error = Some(
            UnityError::new(
                "NETWORK_UNREACHABLE",
                "A Unity service could not be reached.",
            )
            .with_exit_code(Some(7)),
        );
        app.note_read_failed(ReadKind::Projects);

        let vm = app.view_model();
        let PageVm::Projects(projects) = &vm.page else {
            panic!("expected the Projects page");
        };
        let error = projects.projects.error().expect("read error");
        assert_eq!(error.code, "NETWORK_UNREACHABLE");
        let action = error.action.as_ref().expect("retry action");
        assert_eq!(
            action.kind,
            ErrorActionKind::Retry("read:projects".to_string())
        );

        let action = action.clone();
        let _task = app.update(Message::RunErrorAction(action));
        assert!(app.projects.loading, "Retry must re-run the failed read");
    }

    #[test]
    fn navigating_back_does_not_retry_a_failed_read() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Overview;
        app.auth.error = Some(UnityError::new("AUTH_ERROR", "Authorization was denied."));
        app.note_read_failed(ReadKind::Auth);

        // Returning to the page must not silently re-run a read that is
        // waiting for an explicit Retry. A denied authorization would
        // otherwise become a prompt/retry storm.
        let _ = app.update(Message::Navigate(Page::Overview));
        assert!(
            !app.auth.loading,
            "a failed read must wait for Retry or Refresh"
        );
        assert!(app.auth.error.is_some());

        // Refresh is the explicit path that may retry the read.
        let _ = app.update(Message::Refresh);
        assert!(app.auth.loading, "Refresh must re-run the failed read");
    }

    #[test]
    fn retry_is_absent_when_the_controller_has_no_target() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Projects;
        app.projects.error = Some(
            UnityError::new(
                "NETWORK_UNREACHABLE",
                "A Unity service could not be reached.",
            )
            .with_exit_code(Some(7)),
        );
        // No `note_read_failed(ReadKind::Projects)` and no request: the view
        // model must not draw a button that cannot name its read.

        let vm = app.view_model();
        let PageVm::Projects(projects) = &vm.page else {
            panic!("expected the Projects page");
        };
        assert!(projects
            .projects
            .error()
            .expect("read error")
            .action
            .is_none());
    }

    #[test]
    fn unknown_codes_keep_generic_rendering_without_an_action() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Projects;
        app.projects.error = Some(UnityError::new(
            "FUTURE_CLI_CODE",
            "The CLI returned an error this app does not know yet.",
        ));
        app.note_read_failed(ReadKind::Projects);

        let vm = app.view_model();
        let PageVm::Projects(projects) = &vm.page else {
            panic!("expected the Projects page");
        };
        let error = projects.projects.error().expect("read error");
        assert_eq!(error.code, "FUTURE_CLI_CODE");
        assert!(error.action.is_none());
    }

    fn failed_job(kind: &str, error: UnityError) -> ActiveJob {
        ActiveJob {
            id: "job-test".to_string(),
            kind: kind.to_string(),
            command: format!("unity {kind}"),
            state: JobState::Failed,
            progress: 0.0,
            downloads: BTreeMap::new(),
            message: Some("Working…".to_string()),
            logs: VecDeque::new(),
            log_text: Arc::from(""),
            error: Some(error),
            result: None,
        }
    }

    #[test]
    fn job_log_text_prefixes_stderr_and_ends_each_line() {
        let mut job = active_job();
        job.push_log(StreamKind::Stdout, "downloading".to_string());
        job.push_log(StreamKind::Stderr, "checksum mismatch".to_string());

        assert_eq!(&*job.log_text, "downloading\n! checksum mismatch\n");
    }

    #[test]
    fn job_log_text_stays_in_sync_when_the_buffer_rolls_over() {
        let mut job = active_job();

        for index in 0..MAX_LOG_LINES + 5 {
            job.push_log(StreamKind::Stdout, format!("line {index}"));
        }

        let expected: String = (5..MAX_LOG_LINES + 5)
            .map(|index| format!("line {index}\n"))
            .collect();

        assert_eq!(job.logs.len(), MAX_LOG_LINES);
        assert_eq!(&*job.log_text, expected);
    }

    #[test]
    fn template_filters_map_to_cli_values() {
        assert_eq!(
            template_filter(TemplateKindFilter::All),
            TemplateFilter::All
        );
        assert_eq!(
            template_filter(TemplateKindFilter::Core),
            TemplateFilter::Core
        );
        assert_eq!(
            template_filter(TemplateKindFilter::Custom),
            TemplateFilter::Custom
        );
    }

    #[test]
    fn new_project_details_reject_a_missing_parent_and_an_existing_folder() {
        let dir =
            std::env::temp_dir().join(format!("unity-rust-hub-wizard-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Taken")).unwrap();
        std::fs::write(dir.join("Taken").join("ProjectVersion.txt"), b"x").unwrap();

        let mut state = NewProjectState::new();
        state.form.name = "Game".to_string();
        state.form.parent_dir = dir.join("missing").to_string_lossy().into_owned();
        assert!(!state.validation_issues().is_empty());

        state.form.parent_dir = dir.to_string_lossy().into_owned();
        state.form.name = String::new();
        assert!(!state
            .validation_issues()
            .iter()
            .any(|issue| issue.contains("not empty")));
        state.form.name = "Taken".to_string();
        assert!(state
            .validation_issues()
            .iter()
            .any(|issue| issue.contains("not empty")));

        state.form.name = "Free".to_string();
        assert!(state.validation_issues().is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn new_project_editor_step_requires_a_known_architecture() {
        let mut state = NewProjectState::new();
        state.step = NewProjectStep::Editor;
        state.form.editor_version = "6000.3.15f1".to_string();
        state.form.architecture = "ppc".to_string();
        assert!(!state.validation_issues().is_empty());

        state.form.architecture = "x86_64".to_string();
        assert!(state.validation_issues().is_empty());
    }

    #[test]
    fn partial_create_success_keeps_the_path_and_names_the_failed_step() {
        let created = CreateProjectResult {
            path: "/games/Game".to_string(),
            steps: vec![
                unity_core::models::CreateProjectStep {
                    id: "template".to_string(),
                    status: "done".to_string(),
                    ..Default::default()
                },
                unity_core::models::CreateProjectStep {
                    id: "cloudLink".to_string(),
                    status: "failed".to_string(),
                    reason: Some("NO_CLOUD".to_string()),
                    ..Default::default()
                },
            ],
            ..CreateProjectResult::default()
        };

        let vm = new_project_result_vm(&created);
        assert!(!vm.success);
        assert_eq!(vm.path.as_deref(), Some("/games/Game"));
        assert_eq!(vm.failed_steps.len(), 1);
        assert!(vm.failed_steps[0].contains("cloudLink"));
        assert!(vm.summary.contains("1 setup step"));
    }

    #[test]
    fn process_rows_show_play_mode_and_optional_metrics() {
        let mut metrics = HashMap::new();
        metrics.insert(
            42,
            ProcessMetrics {
                cpu_percent: Some(12.5),
                memory_bytes: 1024 * 1024 * 512,
                uptime: Duration::from_secs(90),
            },
        );
        let editor = RunningEditor {
            project_path: Some("/games/Orbital".to_string()),
            editor_version: Some("6000.3.15f1".to_string()),
            pid: Some(42),
            port: Some(7800),
            state: Some("ready".to_string()),
            phase: Some("play mode".to_string()),
            frame_count: Some(120),
            safe_mode: Some(serde_json::json!({ "detected": true })),
            dialog: Some(serde_json::json!({ "title": "Save changes?" })),
            ..RunningEditor::default()
        };

        let row = process_row_vm(&editor, &metrics);

        assert_eq!(row.project_name, "Orbital");
        assert!(row.play_mode);
        assert!(row.safe_mode);
        assert_eq!(row.dialog.as_deref(), Some("Save changes?"));
        assert_eq!(row.pid.as_deref(), Some("42"));
        assert_eq!(row.port.as_deref(), Some("7800"));
        let sampled = row.metrics.expect("metrics should be attached");
        assert_eq!(sampled.cpu, "12.5%");
        assert_eq!(sampled.memory, "512.0 MB");
        assert_eq!(sampled.uptime, "1m");
    }

    #[test]
    fn dialog_blocked_close_and_command_never_offer_retry() {
        // 1.1.0: exit 18 is the dialog contract, not a timeout to retry. The
        // close/command flows are not wired yet, but the policy they must use
        // is fixed here.
        assert!(!job_allows_retry("close"));
        assert!(!job_allows_retry("command"));

        let error =
            UnityError::new("EDITOR_TIMEOUT", "The command timed out.").with_exit_code(Some(18));
        let remediation = remediation_for(&error, None).expect("dialog action");
        assert_eq!(remediation.kind, RemediationKind::AnswerDialog);
        assert!(!remediation.kind.is_retry());
        assert!(error.is_blocked_by_dialog());
    }

    #[test]
    fn process_rows_show_a_dialog_reported_under_blocked_by() {
        let editor = RunningEditor {
            project_path: Some("/games/Orbital".to_string()),
            editor_version: Some("6000.3.15f1".to_string()),
            pid: Some(42),
            // 1.1.0 `unity status` shape: the dialog is under blockedBy, not
            // the Pipeline row's `dialog` field.
            blocked_by: Some(serde_json::json!({
                "dialogs": [{ "title": "Save changes?" }]
            })),
            ..RunningEditor::default()
        };

        let row = process_row_vm(&editor, &HashMap::new());

        assert_eq!(row.dialog.as_deref(), Some("Save changes?"));
    }

    #[test]
    fn template_rows_infer_the_pipeline_and_keep_a_blank_one_optional() {
        let urp = template_row_vm(&TemplateBrief {
            name: "com.unity.template.urp-blank".to_string(),
            display_name: "Universal 3D".to_string(),
            ..TemplateBrief::default()
        });
        assert_eq!(urp.pipeline.as_deref(), Some("URP"));

        let universal_2d = template_row_vm(&TemplateBrief {
            name: "com.unity.template.universal-2d".to_string(),
            display_name: "Universal 2D".to_string(),
            ..TemplateBrief::default()
        });
        assert_eq!(universal_2d.pipeline.as_deref(), Some("URP · 2D"));

        let info: TemplateInfo = serde_json::from_value(serde_json::json!({
            "name": "com.unity.template.universal-2d",
            "displayName": "Universal 2D",
            "status": "READY"
        }))
        .unwrap();
        assert_eq!(template_detail_vm(&info).render_pipeline, None);
    }

    #[test]
    fn result_path_reads_the_cli_data_frame() {
        let value = serde_json::json!({
            "type": "result",
            "success": true,
            "data": { "path": "/games/Game" }
        });
        assert_eq!(result_path(&value).as_deref(), Some("/games/Game"));
    }

    #[test]
    fn built_in_templates_are_hidden_by_default_in_the_wizard() {
        assert!(is_builtin_template("com.unity.template.3d"));
        assert!(is_builtin_template("com.unity.template.2d"));
        assert!(!is_builtin_template("com.unity.template.urp-blank"));
    }

    #[test]
    fn template_not_found_exposes_the_offered_ids() {
        let mut error = UnityError::new("TEMPLATE_NOT_FOUND", "Unknown template");
        error.extra.insert(
            "data".to_string(),
            serde_json::json!({
                "offered": [
                    "com.unity.template.urp-blank",
                    { "name": "com.unity.template.universal-2d" }
                ]
            }),
        );

        let offered = template_offers(&error).expect("offered templates");
        assert_eq!(
            offered,
            vec![
                "com.unity.template.urp-blank".to_string(),
                "com.unity.template.universal-2d".to_string()
            ]
        );
    }

    #[test]
    fn view_model_maps_the_templates_page() {
        let (mut app, _) = App::boot();
        app.page = Page::Templates;
        app.template_editor = Some("6000.3.15f1".to_string());
        app.editors.data = Some(vec![Editor {
            version: "6000.3.15f1".to_string(),
            ..Editor::default()
        }]);
        app.templates.data = Some(vec![TemplateBrief {
            name: "com.unity.template.urp-blank".to_string(),
            display_name: "Universal 3D".to_string(),
            kind: "CORE".to_string(),
            version: "17.1.0".to_string(),
            status: "ready".to_string(),
        }]);
        app.template_detail.data = Some(TemplateInfo {
            name: "com.unity.template.universal-2d".to_string(),
            display_name: "Universal 2D".to_string(),
            status: "READY".to_string(),
            ..TemplateInfo::default()
        });

        let vm = app.view_model();
        match &vm.page {
            PageVm::Templates(templates) => {
                let rows = templates.list.data().expect("template rows");
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].pipeline.as_deref(), Some("URP"));
                let detail = templates.detail.as_ref().expect("detail");
                assert_eq!(detail.render_pipeline, None);
            }
            other => panic!("unexpected page: {other:?}"),
        }
    }

    #[test]
    fn view_model_maps_the_processes_page_with_metrics_off() {
        let (mut app, _) = App::boot();
        app.page = Page::Processes;
        app.processes.data = Some(RunningEditors {
            count: 1,
            instances: vec![RunningEditor {
                project_path: Some("/games/Orbital".to_string()),
                pid: Some(42),
                state: Some("ready".to_string()),
                ..RunningEditor::default()
            }],
        });

        let vm = app.view_model();
        match &vm.page {
            PageVm::Processes(processes) => {
                assert!(!processes.metrics_enabled);
                let rows = processes.rows.data().expect("process rows");
                assert_eq!(rows.len(), 1);
                assert!(rows[0].metrics.is_none());
            }
            other => panic!("unexpected page: {other:?}"),
        }
    }

    #[test]
    fn the_metrics_toggle_is_off_by_default() {
        assert!(!AppSettings::default().process_metrics);
        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(!settings.process_metrics);
    }

    fn store_advisory(
        code: &str,
        message: &str,
        remediation: Option<serde_json::Value>,
    ) -> unity_core::advisory::CliAdvisory {
        let mut value = serde_json::json!({ "code": code, "message": message });
        if let Some(remediation) = remediation {
            value["remediation"] = remediation;
        }
        unity_core::advisory::advisories_from_value(&serde_json::json!({ "warnings": [value] }))
            .into_iter()
            .next()
            .expect("test advisory is valid")
    }

    #[test]
    fn view_model_exposes_notifications_unread_rail_and_toast() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        app.notifications
            .push_event(NotificationLevel::Info, "Notification", "One.");
        app.notifications
            .push_event(NotificationLevel::Warning, "Warning", "Two.");
        app.notifications.set_rail_open(true);

        let vm = app.view_model();

        assert_eq!(vm.shell.notifications.len(), 2);
        assert_eq!(vm.shell.notifications[0].message, "Two.");
        assert_eq!(vm.shell.notifications[0].tone, Tone::Warning);
        assert_eq!(vm.shell.notifications[0].label, "Warning");
        assert_eq!(vm.shell.unread, 0);
        assert!(vm.shell.rail_open);
        let toast = vm.shell.toast.as_ref().expect("newest entry is toasted");
        assert_eq!(toast.message, "Two.");
        assert_eq!(toast.tone, Tone::Warning);
    }

    #[test]
    fn a_hydrated_store_survives_the_view_model_mapping() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        app.notifications.push_advisory(&store_advisory(
            "CLI_COMPATIBILITY",
            "Compatibility mode is on.",
            Some(serde_json::json!({
                "command": "unity status",
                "requiresUserApproval": false
            })),
        ));
        let id = app.notifications.snapshot()[0].id;
        app.notifications.set_state(id, ActionState::Confirming);

        let vm = app.view_model();
        let entry = vm
            .shell
            .notifications
            .first()
            .expect("the hydrated entry is mapped");

        assert_eq!(entry.id, id);
        assert_eq!(entry.label, "CLI_COMPATIBILITY");
        assert_eq!(entry.message, "Compatibility mode is on.");
        assert_eq!(entry.command.as_deref(), Some("unity status"));
        assert!(entry.can_run);
        assert_eq!(
            entry.state,
            unity_hub_model::NotificationActionState::Confirming
        );
        assert_eq!(vm.shell.unread, 1);
    }

    #[test]
    fn the_confirmation_gate_blocks_run_for_approval_required_remediation() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        app.notifications.push_advisory(&store_advisory(
            "CLI_UPDATE_AVAILABLE",
            "A new CLI version is available.",
            Some(serde_json::json!({
                "command": "unity self-update",
                "requiresUserApproval": true
            })),
        ));
        let id = app.notifications.snapshot()[0].id;

        let _ = app.update(Message::RunNotificationCommand(id));
        assert_eq!(
            app.notifications.get(id).map(|entry| entry.state),
            Some(ActionState::Idle),
            "approval-required entries have no Run action"
        );

        let _ = app.update(Message::ConfirmNotificationCommand(id));
        assert_eq!(
            app.notifications.get(id).map(|entry| entry.state),
            Some(ActionState::Idle)
        );
        assert!(app.active_job.is_none());
    }

    #[test]
    fn a_runnable_notification_waits_in_the_confirming_state() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        app.notifications.push_advisory(&store_advisory(
            "CLI_COMPATIBILITY",
            "Compatibility mode is on.",
            Some(serde_json::json!({
                "command": "unity status",
                "requiresUserApproval": false
            })),
        ));
        let id = app.notifications.snapshot()[0].id;

        let _ = app.update(Message::RunNotificationCommand(id));
        assert_eq!(
            app.notifications.get(id).map(|entry| entry.state),
            Some(ActionState::Confirming)
        );

        let _ = app.update(Message::CancelNotificationCommand(id));
        assert_eq!(
            app.notifications.get(id).map(|entry| entry.state),
            Some(ActionState::Idle)
        );
    }

    #[test]
    fn a_job_that_started_a_mutation_never_offers_retry() {
        let mut app = connected_app("1.0.0-beta.17");
        app.active_job = Some(failed_job(
            "editor-install",
            UnityError::new(
                "NETWORK_UNREACHABLE",
                "A Unity service could not be reached.",
            )
            .with_exit_code(Some(7)),
        ));

        let vm = app.view_model();
        let error = vm.shell.job.expect("job").error.expect("job error");
        assert_eq!(error.code, "NETWORK_UNREACHABLE");
        assert!(
            error.action.is_none(),
            "an install that already started must not be replayed"
        );
    }

    #[test]
    fn every_existing_job_kind_is_treated_as_non_idempotent() {
        for kind in [
            "cli-install",
            "cli-update",
            "editor-install",
            "module-install",
            "auth-login",
            "project-upgrade",
        ] {
            assert!(!job_allows_retry(kind), "{kind} must not be retryable");
        }
    }

    #[test]
    fn parallel_install_progress_is_aggregated_and_monotonic() {
        let mut state = BTreeMap::new();

        // a and b start together: a is active at 10%, b is queued.
        let first = aggregate_progress(
            &mut state,
            &[
                download("a", None, true, false),
                download("b", None, false, true),
            ],
            Some(10.0),
        )
        .expect("aggregate");
        assert!((first - 5.0).abs() < 0.01, "{first}");

        // Interleaved frame: a finishes, b is active at 20%. The mean must not
        // fall back to b's own 20%.
        let second = aggregate_progress(
            &mut state,
            &[
                download("a", None, false, false),
                download("b", None, true, false),
            ],
            Some(20.0),
        )
        .expect("aggregate");
        assert!(second >= first, "{second} < {first}");
        assert!((second - 60.0).abs() < 0.01, "{second}");

        // Both finished.
        let third = aggregate_progress(
            &mut state,
            &[
                download("a", None, false, false),
                download("b", None, false, false),
            ],
            None,
        )
        .expect("aggregate");
        assert_eq!(third, 100.0);

        // Jobs without per-file rows keep the scalar path.
        assert_eq!(
            aggregate_progress(&mut BTreeMap::new(), &[], Some(42.0)),
            Some(42.0)
        );
    }

    fn download(key: &str, pct: Option<f32>, downloading: bool, queued: bool) -> DownloadStatus {
        DownloadStatus {
            key: key.to_string(),
            pct,
            downloading,
            queued,
            failed: false,
        }
    }

    #[test]
    fn a_job_error_keeps_its_non_retry_action_and_code() {
        let mut app = connected_app("1.0.0-beta.17");
        app.active_job = Some(failed_job(
            "module-install",
            UnityError::new("VCS_TOKEN_REQUIRED", "A token is required.").with_exit_code(Some(4)),
        ));

        let vm = app.view_model();
        let error = vm.shell.job.expect("job").error.expect("job error");
        assert_eq!(error.code, "VCS_TOKEN_REQUIRED");
        let action = error.action.expect("token action");
        assert_eq!(action.kind, ErrorActionKind::EnterToken);
        assert!(error.detail.expect("detail").contains("UNITY_GITHUB_TOKEN"));
    }

    #[test]
    fn retry_uses_the_request_captured_when_the_read_started() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Editors;
        app.modules.error = Some(
            UnityError::new(
                "NETWORK_UNREACHABLE",
                "A Unity service could not be reached.",
            )
            .with_exit_code(Some(7)),
        );
        app.retry_targets.insert(
            ReadKind::Modules.id().to_string(),
            RetryRequest::Modules {
                version: "6000.1.1f1".to_string(),
                architecture: Some("arm64".to_string()),
            },
        );
        // A later edit to the form must not change the read Retry repeats.
        app.install_version = "6000.9.9f1".to_string();

        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        let action = editors
            .install
            .modules
            .error()
            .expect("module error")
            .action
            .as_ref()
            .expect("retry")
            .clone();
        assert_eq!(
            action.kind,
            ErrorActionKind::Retry("read:modules".to_string())
        );
        assert_eq!(
            app.retry_targets.get(ReadKind::Modules.id()),
            Some(&RetryRequest::Modules {
                version: "6000.1.1f1".to_string(),
                architecture: Some("arm64".to_string()),
            })
        );
    }

    #[test]
    fn preview_failure_has_its_own_retry_action() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Editors;
        app.retry_targets.insert(
            ReadKind::PreviewInstall.id().to_string(),
            RetryRequest::PreviewInstall {
                version: "lts".to_string(),
                architecture: Some("arm64".to_string()),
                modules: vec!["webgl".to_string()],
                headless: false,
            },
        );
        app.preview_error = Some(app.ui_error(
            &UnityError::new("TIMEOUT", "The operation timed out.").with_exit_code(Some(14)),
            ErrorContext::Read(ReadKind::PreviewInstall),
        ));

        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        let action = editors
            .install
            .preview_error
            .as_ref()
            .expect("preview error")
            .action
            .as_ref()
            .expect("retry")
            .clone();
        assert_eq!(
            action.kind,
            ErrorActionKind::Retry("read:preview-install".to_string())
        );

        let _task = app.update(Message::RunErrorAction(action));
        assert!(app.preview_loading);
    }

    // ------------------------------------------------------------------
    // Install resilience and CLI rollback

    fn install_request(version: &str, architecture: Option<&str>) -> InstallRequest {
        InstallRequest {
            version: version.to_string(),
            architecture: architecture.map(str::to_string),
            modules: Vec::new(),
            headless: false,
            accept_eula: false,
            options: InstallOptions::default(),
        }
    }

    #[test]
    fn the_default_install_command_is_unchanged_when_advanced_is_untouched() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(temporary_cli()));
        app.install_version = "6000.6.6f1".to_string();

        let spec = app.install_editor_spec().expect("default install spec");

        assert_eq!(
            &spec.args[..3],
            &[
                "install".to_string(),
                "6000.6.6f1".to_string(),
                "--yes".to_string()
            ]
        );
        for advanced in [
            "--resume",
            "--download-only",
            "--no-wait",
            "--timeout",
            "--set-default",
            "--retries",
        ] {
            assert!(
                !spec.args.iter().any(|arg| arg == advanced),
                "{advanced} must stay out of the default command"
            );
        }
        assert!(spec
            .env
            .iter()
            .all(|(key, _)| key != "UNITY_INSTALL_RETRIES"));
    }

    #[test]
    fn no_wait_and_timeout_clear_each_other_and_explain_the_lock() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(temporary_cli()));
        app.install_version = "6000.6.6f1".to_string();

        let _ = app.update(Message::InstallNoWaitToggled(true));
        assert!(app.install_no_wait);
        let spec = app.install_editor_spec().expect("no-wait spec");
        assert!(spec.args.iter().any(|arg| arg == "--no-wait"));

        let _ = app.update(Message::InstallTimeoutChanged("600".to_string()));
        assert!(!app.install_no_wait, "the timeout must clear no-wait");
        let note = app
            .install_advanced_note
            .clone()
            .expect("the conflict is explained");
        assert!(note.contains("cleared no-wait"), "{note}");

        let spec = app.install_editor_spec().expect("timeout spec");
        assert!(spec.args.iter().any(|arg| arg == "--timeout"));
        assert!(spec.args.contains(&"600".to_string()));
        assert!(!spec.args.iter().any(|arg| arg == "--no-wait"));

        let _ = app.update(Message::InstallNoWaitToggled(true));
        assert!(app.install_timeout.is_empty(), "no-wait clears the timeout");
        let note = app.install_advanced_note.clone().expect("note");
        assert!(note.contains("cleared the timeout"), "{note}");
    }

    #[test]
    fn conflicting_install_flags_are_cleared_before_the_command_is_built() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(temporary_cli()));
        app.install_version = "6000.6.6f1".to_string();

        let _ = app.update(Message::InstallDownloadOnlyToggled(true));
        let _ = app.update(Message::InstallResumeToggled(true));
        assert!(app.install_resume);
        assert!(!app.install_download_only);
        assert!(!app.install_set_default);

        let _ = app.update(Message::InstallSetDefaultToggled(true));
        assert!(app.install_set_default);
        assert!(!app.install_resume);
        assert!(!app.install_download_only);

        let spec = app.install_editor_spec().expect("set-default spec");
        assert!(spec.args.iter().any(|arg| arg == "--set-default"));
        assert!(!spec.args.iter().any(|arg| arg == "--download-only"));
        assert!(!spec.args.iter().any(|arg| arg == "--resume"));
    }

    #[test]
    fn advanced_install_options_reach_the_exact_command() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Editors;
        app.cli.set_override_path(Some(temporary_cli()));
        app.install_version = "6000.6.6f1".to_string();
        app.install_architecture = Some("arm64".to_string());
        app.accept_eula = true;

        let _ = app.update(Message::InstallResumeToggled(true));
        let _ = app.update(Message::InstallTimeoutChanged("600".to_string()));
        let _ = app.update(Message::InstallRetriesChanged("3".to_string()));

        let spec = app.install_editor_spec().expect("advanced install spec");
        assert_eq!(
            &spec.args[..9],
            &[
                "install".to_string(),
                "6000.6.6f1".to_string(),
                "--yes".to_string(),
                "--architecture".to_string(),
                "arm64".to_string(),
                "--accept-eula".to_string(),
                "--resume".to_string(),
                "--timeout".to_string(),
                "600".to_string(),
            ]
        );
        assert!(spec
            .env
            .iter()
            .any(|(key, value)| key == "UNITY_INSTALL_RETRIES" && value == "3"));
        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        assert!(editors.install.command_preview.contains("--resume"));
    }

    #[test]
    fn module_install_advanced_options_name_retries_and_the_directory() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.modal = Modal::EditorModules(Editor {
            version: "6000.6.6f1".to_string(),
            architecture: "x86_64".to_string(),
            ..Editor::default()
        });
        app.dialog_selected.insert("webgl".to_string());
        app.dialog_eula = true;
        app.dialog_retries = "0".to_string();
        app.dialog_download_directory = "./.unity".to_string();

        let vm = app.view_model();
        let Some(ModalVm::EditorModules(modules)) = &vm.modal else {
            panic!("expected the modules dialog");
        };
        assert!(modules.can_install);
        assert_eq!(modules.retries_input, "0");
        assert!(modules.command_preview.contains("--retries 0"));

        let _ = app.update(Message::StartInstallModules);
        let job = app.active_job.as_ref().expect("module install job");
        for expected in [
            "--retries 0",
            "--download-directory ./.unity",
            "--accept-eula",
        ] {
            assert!(job.command.contains(expected), "{}", job.command);
        }
    }

    #[test]
    fn an_interrupted_install_offers_resume_and_reuses_its_cache_request() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Editors;
        app.cli.set_override_path(Some(startable_cli()));
        app.last_install = Some(InstallRequest {
            version: "6000.6.6f1".to_string(),
            architecture: Some("arm64".to_string()),
            modules: vec!["android".to_string()],
            headless: false,
            accept_eula: true,
            options: InstallOptions::default(),
        });

        let _ = app.handle_finished_job(
            "editor-install".to_string(),
            JobState::Failed,
            Some(UnityError::new(
                "INTERRUPTED",
                "The install was interrupted.",
            )),
            None,
        );

        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        let offer = editors.install.resume_offer.as_ref().expect("resume offer");
        assert_eq!(offer.version, "6000.6.6f1");
        assert_eq!(offer.architecture, "arm64");
        assert!(offer.can_resume);

        let _ = app.update(Message::ResumeInstallEditor);
        let job = app.active_job.as_ref().expect("resume job");
        assert!(job.command.contains("--resume"));
        assert!(job.command.contains("6000.6.6f1"));
        assert!(job.command.contains("--architecture arm64"));
        assert!(job.command.contains("-m android"));
        assert!(!job.command.contains("--download-only"));
        assert!(!job.command.contains("--set-default"));
        assert!(app.resume_offer.is_none(), "the offer is consumed");
    }

    #[test]
    fn a_generic_install_failure_does_not_offer_resume() {
        let mut app = connected_app("1.0.0-beta.17");
        app.last_install = Some(install_request("6000.6.6f1", None));

        let _ = app.handle_finished_job(
            "editor-install".to_string(),
            JobState::Failed,
            Some(UnityError::new("LOCAL_STORE_UNWRITABLE", "No permission.")),
            None,
        );

        assert!(app.resume_offer.is_none());
    }

    #[test]
    fn a_cli_reported_cache_offers_resume_even_for_a_generic_failure() {
        let mut app = connected_app("1.0.0-beta.17");
        app.last_install = Some(install_request("6000.6.6f1", None));

        let result = serde_json::json!({
            "type": "result",
            "success": false,
            "data": { "cachePath": "/home/ada/Unity/Cache/6000.6.6f1" }
        });
        let _ = app.handle_finished_job(
            "editor-install".to_string(),
            JobState::Failed,
            Some(UnityError::new("COMMAND_FAILED", "The install failed.")),
            Some(result),
        );

        let offer = app.resume_offer.as_ref().expect("resume offer");
        assert!(
            offer.reason.contains("Cache/6000.6.6f1"),
            "{}",
            offer.reason
        );
    }

    #[test]
    fn download_only_reports_its_folder_and_registers_no_editor() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Editors;
        app.editors.data = Some(vec![editor("6000.1.1f1", "x86_64", false)]);
        app.last_install = Some(InstallRequest {
            options: InstallOptions {
                download_only: true,
                ..InstallOptions::default()
            },
            ..install_request("6000.6.6f1", None)
        });

        let result = serde_json::json!({
            "type": "result",
            "success": true,
            "data": { "path": "/home/ada/Unity/downloads", "completedUids": ["editor"] }
        });
        let _ = app.handle_finished_job(
            "editor-install".to_string(),
            JobState::Succeeded,
            None,
            Some(result),
        );

        let vm = app.view_model();
        let PageVm::Editors(editors) = &vm.page else {
            panic!("expected the Editors page");
        };
        let report = editors
            .install
            .download_report
            .as_ref()
            .expect("download report");
        assert!(report.contains("/home/ada/Unity/downloads"), "{report}");
        assert!(report.contains("Nothing was installed or registered"));
        // The installed list still names only the pre-existing build.
        let rows = editors.installed.data().expect("installed rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].version, "6000.1.1f1");
    }

    #[test]
    fn a_failed_download_only_run_does_not_offer_resume_as_an_install() {
        let mut app = connected_app("1.0.0-beta.17");
        app.last_install = Some(InstallRequest {
            options: InstallOptions {
                download_only: true,
                ..InstallOptions::default()
            },
            ..install_request("6000.6.6f1", None)
        });

        let _ = app.handle_finished_job(
            "editor-install".to_string(),
            JobState::Failed,
            Some(UnityError::new("INTERRUPTED", "The download stopped.")),
            None,
        );

        assert!(
            app.resume_offer.is_none(),
            "resuming a download-only run would install an Editor the user declined"
        );
    }

    #[test]
    fn applying_a_cli_update_requires_the_target_changelog() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Overview;

        let vm = app.view_model();
        let PageVm::Overview(overview) = &vm.page else {
            panic!("expected the Overview page");
        };
        assert!(!overview.update_flow.can_apply);
        assert!(overview
            .update_flow
            .apply_blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("release notes")));

        app.update_target = "1.0.0-beta.18".to_string();
        app.update_changelog.data = Some(unity_core::models::RawChangelog {
            version: "1.0.0-beta.18".to_string(),
            changelog: "## Added\n\n- Rollback.".to_string(),
        });
        let vm = app.view_model();
        let PageVm::Overview(overview) = &vm.page else {
            panic!("expected the Overview page");
        };
        assert!(overview.update_flow.can_apply);
        let notes = overview
            .update_flow
            .changelog
            .data()
            .expect("release notes");
        assert_eq!(notes.version, "1.0.0-beta.18");
        assert!(notes.text.contains("Rollback"));

        // Changing the target invalidates the notes the apply was gated on.
        let _ = app.update(Message::CliUpdateTargetChanged("1.0.0-beta.19".to_string()));
        let vm = app.view_model();
        let PageVm::Overview(overview) = &vm.page else {
            panic!("expected the Overview page");
        };
        assert!(!overview.update_flow.can_apply);
        assert!(overview.update_flow.changelog.data().is_none());
    }

    #[test]
    fn status_refresh_never_moves_a_user_chosen_update_channel_or_reuses_stale_notes() {
        let mut app = connected_app("1.0.0-beta.17");
        app.boot_background_started = true;
        app.page = Page::Overview;

        // The user explicitly picks stable and loads its notes.
        let _ = app.update(Message::CliUpdateChannelChanged("stable".to_string()));
        app.update_changelog.data = Some(unity_core::models::RawChangelog {
            version: "1.0.0-beta.18".to_string(),
            changelog: "stable notes".to_string(),
        });

        // A later status read says the installed CLI tracks beta. It must not
        // move the flow or retain notes that describe another channel.
        let _ = app.update(Message::StatusLoaded(Ok(CliStatus {
            installed: true,
            version: Some(CliVersion {
                version: "1.0.0-beta.17".to_string(),
                channel: "beta".to_string(),
                ..CliVersion::default()
            }),
            ..CliStatus::default()
        })));

        assert_eq!(app.update_channel, "stable");
        assert_eq!(
            app.update_changelog
                .data
                .as_ref()
                .map(|raw| raw.version.as_str()),
            Some("1.0.0-beta.18"),
            "the stable notes still describe the selected channel"
        );

        // Before the user touches the flow, the status read owns the channel.
        // A change there goes through the same invalidation path.
        let mut fresh = connected_app("1.0.0-beta.17");
        fresh.boot_background_started = true;
        fresh.update_channel = "beta".to_string();
        fresh.update_changelog.data = Some(unity_core::models::RawChangelog {
            version: "1.0.0-beta.18".to_string(),
            changelog: "beta notes".to_string(),
        });

        let _ = fresh.update(Message::StatusLoaded(Ok(CliStatus {
            installed: true,
            version: Some(CliVersion {
                version: "1.0.0-beta.17".to_string(),
                channel: "stable".to_string(),
                ..CliVersion::default()
            }),
            ..CliStatus::default()
        })));

        assert_eq!(fresh.update_channel, "stable");
        assert!(
            fresh.update_changelog.data.is_none(),
            "a status-derived channel change invalidates the old notes"
        );
    }

    #[test]
    fn update_apply_names_only_the_chosen_target_and_channel() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));
        app.update_target = "1.0.0-beta.18".to_string();
        app.update_channel = "beta".to_string();
        app.update_changelog.data = Some(unity_core::models::RawChangelog {
            version: "1.0.0-beta.18".to_string(),
            changelog: "notes".to_string(),
        });

        let _ = app.update(Message::StartCliUpdate);

        let job = app.active_job.as_ref().expect("update job");
        assert_eq!(job.kind, "cli-update");
        assert!(job.command.contains("--target 1.0.0-beta.18"));
        assert!(job.command.contains("--channel beta"));
        assert!(!job.command.contains("--rollback"));
        assert!(!job.command.contains("--dry-run"));
    }

    #[test]
    fn rollback_requires_confirmation_and_names_the_restored_version() {
        let mut app = connected_app("1.0.0-beta.17");
        app.cli.set_override_path(Some(startable_cli()));

        let _ = app.update(Message::OpenCliRollback);
        let _ = app.update(Message::CliRollbackVersionChanged(
            "1.0.0-beta.16".to_string(),
        ));

        let vm = app.view_model();
        let Some(ModalVm::RollbackCli(rollback)) = &vm.modal else {
            panic!("expected the rollback dialog");
        };
        assert_eq!(rollback.restore_label, "Unity CLI 1.0.0-beta.16");
        assert!(!rollback.can_rollback);
        assert!(!rollback.command.contains("--target"));
        assert!(!rollback.command.contains("--channel"));
        assert!(!rollback.command.contains("--dry-run"));

        let _ = app.update(Message::StartCliRollback);
        assert!(
            app.active_job.is_none(),
            "an unconfirmed rollback must do nothing"
        );

        let _ = app.update(Message::CliRollbackConfirmed(true));
        let _ = app.update(Message::StartCliRollback);
        let job = app.active_job.as_ref().expect("rollback job");
        assert_eq!(job.kind, "cli-rollback");
        assert!(job.command.contains("--rollback 1.0.0-beta.16"));
        for refused in [
            "--check",
            "--changelog",
            "--target",
            "--channel",
            "--dry-run",
        ] {
            assert!(
                !job.command.contains(refused),
                "{refused} must never join rollback: {}",
                job.command
            );
        }
    }

    #[test]
    fn rollback_bad_args_keeps_the_dialog_and_renders_the_cli_code() {
        let mut app = connected_app("1.0.0-beta.17");
        app.modal = Modal::RollbackCli;
        app.rollback_confirmed = true;

        let _ = app.handle_finished_job(
            "cli-rollback".to_string(),
            JobState::Failed,
            Some(UnityError::new(
                "BAD_ARGS",
                "Rollback cannot be combined with --target.",
            )),
            None,
        );

        assert!(matches!(app.modal, Modal::RollbackCli));
        assert!(!app.rollback_confirmed);
        let vm = app.view_model();
        let Some(ModalVm::RollbackCli(rollback)) = &vm.modal else {
            panic!("expected the rollback dialog");
        };
        let error = rollback.error.as_ref().expect("rollback error");
        assert_eq!(error.code, "BAD_ARGS");
        let action = error.action.as_ref().expect("a copy action");
        assert_eq!(action.kind, ErrorActionKind::ShowCommand);
    }

    fn doctor_read() -> DoctorRead {
        let mut params = Map::new();
        params.insert(
            "directory".to_string(),
            Value::String("/home/ada/.local/bin".to_string()),
        );
        params.insert("expected".to_string(), Value::Bool(true));

        DoctorRead {
            report: unity_core::models::DoctorReport {
                platform: "linux".to_string(),
                arch: "x86_64".to_string(),
                runtime_version: Some("8.0.4".to_string()),
                cli_version: Some("1.0.0-beta.17".to_string()),
                install_path: Some("/home/ada/.unity".to_string()),
                log_directory: Some("/home/ada/.unity/logs".to_string()),
                checks: vec![
                    DoctorCheck {
                        id: "path-available".to_string(),
                        status: "pass".to_string(),
                        message_key: Some("doctor.checks.pathAvailable".to_string()),
                        params,
                        ..DoctorCheck::default()
                    },
                    DoctorCheck {
                        id: String::new(),
                        status: "warn".to_string(),
                        message_key: Some("doctor.checks.longPathSupport".to_string()),
                        ..DoctorCheck::default()
                    },
                    DoctorCheck {
                        id: String::new(),
                        status: "fail".to_string(),
                        message: Some("Git credential helper is not configured.".to_string()),
                        ..DoctorCheck::default()
                    },
                    DoctorCheck {
                        id: "future-check".to_string(),
                        status: "future".to_string(),
                        ..DoctorCheck::default()
                    },
                ],
                auth: Some(serde_json::json!({ "loggedIn": true, "user": "ada" })),
                license: Some(serde_json::json!({ "active": false })),
                ..unity_core::models::DoctorReport::default()
            },
            raw: "{\n  \"platform\": \"linux\"\n}".to_string(),
        }
    }

    fn environment_info() -> EnvironmentInfo {
        EnvironmentInfo {
            user_data_path: "/home/ada/.config/Unity".to_string(),
            editor_install_path: "/home/ada/Unity/Editors".to_string(),
            download_cache_path: "/home/ada/Unity/Cache".to_string(),
            config_path: "/home/ada/.unity".to_string(),
            cli_version: Some("1.0.0-beta.17".to_string()),
            hub_version: Some("0.2.1".to_string()),
            proxy: Some(EnvironmentProxy {
                url: Some("http://proxy.example:8080".to_string()),
                source: Some("env".to_string()),
                auth_source: Some("keyring".to_string()),
                no_proxy: vec!["localhost".to_string(), "*.unity.local".to_string()],
                ..EnvironmentProxy::default()
            }),
            ..EnvironmentInfo::default()
        }
    }

    #[test]
    fn doctor_and_environment_map_to_humanized_diagnostics() {
        let (mut app, _) = App::boot();
        app.page = Page::Diagnostics;
        app.doctor.data = Some(doctor_read());
        app.environment.data = Some(environment_info());

        let vm = app.view_model();
        let PageVm::Diagnostics(diagnostics) = &vm.page else {
            panic!("expected the Diagnostics page");
        };

        let doctor = diagnostics.doctor.data().expect("doctor data");
        assert_eq!(doctor.platform, "linux");
        assert_eq!(doctor.arch, "x86_64");
        assert_eq!(doctor.runtime_version, "8.0.4");
        assert_eq!(doctor.node_version, "—");
        assert_eq!(doctor.cli_version, "1.0.0-beta.17");
        assert_eq!(doctor.checks.len(), 4);

        let path = &doctor.checks[0];
        assert_eq!(path.id, "path-available");
        assert_eq!(path.label, "Path available");
        assert_eq!(path.status, CheckStatus::Pass);
        assert_eq!(
            path.detail.as_deref(),
            Some("Directory: /home/ada/.local/bin · Expected: true")
        );

        assert_eq!(doctor.checks[1].label, "Long path support");
        assert_eq!(doctor.checks[1].status, CheckStatus::Warn);
        assert_eq!(
            doctor.checks[2].label,
            "Git credential helper is not configured."
        );
        assert_eq!(doctor.checks[2].status, CheckStatus::Fail);
        assert_eq!(doctor.checks[3].status, CheckStatus::Unknown);
        assert_eq!(doctor.auth.as_deref(), Some("Logged in: true · User: ada"));
        assert_eq!(doctor.license.as_deref(), Some("Active: false"));
        assert_eq!(doctor.raw, "{\n  \"platform\": \"linux\"\n}");

        let environment = diagnostics.environment.data().expect("environment data");
        assert_eq!(environment.user_data_path, "/home/ada/.config/Unity");
        assert_eq!(environment.editor_install_path, "/home/ada/Unity/Editors");
        assert_eq!(environment.download_cache_path, "/home/ada/Unity/Cache");
        assert_eq!(environment.config_path, "/home/ada/.unity");
        assert_eq!(environment.cli_version, "1.0.0-beta.17");
        assert_eq!(environment.hub_version, "0.2.1");
        let proxy = environment.proxy.as_ref().expect("proxy");
        assert_eq!(proxy.url, "http://proxy.example:8080");
        assert_eq!(proxy.source, "env");
        assert_eq!(proxy.auth_source, "keyring");
        assert_eq!(proxy.no_proxy, vec!["localhost", "*.unity.local"]);
    }

    #[test]
    fn doctor_labels_humanize_ids_and_fall_back_to_the_key() {
        assert_eq!(humanize_identifier("path-available"), "Path available");
        assert_eq!(
            humanize_identifier("git-credential-helper"),
            "Git credential helper"
        );
        assert_eq!(humanize_identifier("fex-emu"), "FEX emu");
        assert_eq!(humanize_identifier("longPathSupport"), "Long path support");
        assert_eq!(
            humanize_message_key("doctor.checks.longPathSupport"),
            "Long path support"
        );

        let key_only = DoctorCheck {
            message_key: Some("doctor.checks.gitCredentialHelper".to_string()),
            ..DoctorCheck::default()
        };
        assert_eq!(check_label(&key_only), "Git credential helper");

        let message_only = DoctorCheck {
            message_key: Some("doctor.checks.gitCredentialHelper".to_string()),
            message: Some("Git is not installed.".to_string()),
            ..DoctorCheck::default()
        };
        assert_eq!(check_label(&message_only), "Git is not installed.");
        assert_eq!(check_label(&DoctorCheck::default()), "Check");
        assert_eq!(check_status("PASS"), CheckStatus::Pass);
        assert_eq!(check_status("nope"), CheckStatus::Unknown);
    }

    #[test]
    fn diagnostics_sections_keep_independent_states_and_retry_one_at_a_time() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Diagnostics;
        app.doctor.data = Some(doctor_read());
        app.environment.error = Some(
            UnityError::new(
                "NETWORK_UNREACHABLE",
                "A Unity service could not be reached.",
            )
            .with_exit_code(Some(7)),
        );
        app.note_read_failed(ReadKind::Environment);
        app.diagnose_proxy.error =
            Some(UnityError::new("TIMEOUT", "The operation timed out.").with_exit_code(Some(14)));
        app.note_read_failed(ReadKind::DiagnoseProxy);
        app.diagnose_accelerator.data = Some("Accelerator: none".to_string());
        app.diagnose_update.loading = true;

        let vm = app.view_model();
        let PageVm::Diagnostics(diagnostics) = &vm.page else {
            panic!("expected the Diagnostics page");
        };

        assert!(diagnostics.doctor.data().is_some());
        assert!(diagnostics.environment.error().is_some());
        assert!(diagnostics.proxy_report.error().is_some());
        assert_eq!(
            diagnostics
                .accelerator_report
                .data()
                .expect("accelerator report")
                .text,
            "Accelerator: none"
        );
        assert!(diagnostics.update_report.is_loading());
        assert_eq!(
            diagnostics
                .environment
                .error()
                .expect("environment error")
                .action
                .as_ref()
                .expect("retry")
                .kind,
            ErrorActionKind::Retry("read:environment".to_string())
        );
        assert_eq!(
            diagnostics
                .proxy_report
                .error()
                .expect("proxy error")
                .action
                .as_ref()
                .expect("retry")
                .kind,
            ErrorActionKind::Retry("read:diagnose-proxy".to_string())
        );

        let retry = diagnostics
            .environment
            .error()
            .expect("environment error")
            .action
            .clone()
            .expect("retry");
        let _task = app.update(Message::RunErrorAction(retry));

        assert!(app.environment.loading);
        assert!(app.doctor.data.is_some() && !app.doctor.loading);
        assert!(!app.diagnose_proxy.loading);
        assert!(!app.diagnose_accelerator.loading);
        // A retry of one section must not disturb a section that is already
        // loading, ready, or failed.
        assert!(app.diagnose_update.loading);
    }

    fn retry_kind<T>(state: &LoadState<T>) -> &ErrorActionKind {
        &state
            .error()
            .expect("section error")
            .action
            .as_ref()
            .expect("retry")
            .kind
    }

    #[test]
    fn every_diagnostics_section_has_its_own_retry_target() {
        let (mut app, _) = App::boot();
        app.page = Page::Diagnostics;

        let sections = [
            (ReadKind::Doctor, "read:doctor"),
            (ReadKind::Environment, "read:environment"),
            (ReadKind::DiagnoseProxy, "read:diagnose-proxy"),
            (ReadKind::DiagnoseAccelerator, "read:diagnose-accelerator"),
            (ReadKind::DiagnoseUpdate, "read:diagnose-update"),
        ];
        for (kind, _) in sections {
            app.note_read_failed(kind);
        }

        app.doctor.error = Some(UnityError::new("TIMEOUT", "The read timed out."));
        app.environment.error = Some(UnityError::new("TIMEOUT", "The read timed out."));
        app.diagnose_proxy.error = Some(UnityError::new("TIMEOUT", "The read timed out."));
        app.diagnose_accelerator.error = Some(UnityError::new("TIMEOUT", "The read timed out."));
        app.diagnose_update.error = Some(UnityError::new("TIMEOUT", "The read timed out."));

        let vm = app.view_model();
        let PageVm::Diagnostics(diagnostics) = &vm.page else {
            panic!("expected the Diagnostics page");
        };
        for (kind, expected) in [
            (retry_kind(&diagnostics.doctor), "read:doctor"),
            (retry_kind(&diagnostics.environment), "read:environment"),
            (retry_kind(&diagnostics.proxy_report), "read:diagnose-proxy"),
            (
                retry_kind(&diagnostics.accelerator_report),
                "read:diagnose-accelerator",
            ),
            (
                retry_kind(&diagnostics.update_report),
                "read:diagnose-update",
            ),
        ] {
            assert_eq!(kind, &ErrorActionKind::Retry(expected.to_string()));
        }
    }

    struct FailingClipboard;

    impl ClipboardBackend for FailingClipboard {
        fn write(&self, _text: &str) -> Result<(), String> {
            Err("backend unavailable".to_string())
        }
    }

    struct RecordingClipboard;

    impl ClipboardBackend for RecordingClipboard {
        fn write(&self, _text: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn non_empty_copy_failure_is_reported_without_a_success_notice() {
        let (mut app, _) = App::boot();
        app.clipboard = Box::new(FailingClipboard);
        app.doctor.data = Some(doctor_read());

        let _task = app.update(Message::CopyDiagnosticsReport);

        let notice = app.notice.as_ref().expect("notice");
        assert_eq!(notice.kind, NoticeKind::Error);
        assert!(notice.text.contains("backend unavailable"));
        assert!(!notice.text.contains("copied to the clipboard"));
    }

    struct CountingClipboard(std::rc::Rc<std::cell::Cell<usize>>);

    impl ClipboardBackend for CountingClipboard {
        fn write(&self, _text: &str) -> Result<(), String> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }

    #[test]
    fn copying_twice_reuses_the_app_owned_clipboard_backend() {
        let writes = std::rc::Rc::new(std::cell::Cell::new(0));
        let (mut app, _) = App::boot();
        app.clipboard = Box::new(CountingClipboard(writes.clone()));
        app.doctor.data = Some(doctor_read());

        let _first = app.update(Message::CopyDiagnosticsReport);
        let _second = app.update(Message::CopyDiagnosticsReport);

        assert_eq!(writes.get(), 2);
    }

    #[test]
    fn doctor_sanitization_preserves_unrelated_whitespace() {
        let raw = r#"{"logDirectory":"  /tmp/logs  ","proxy":{"url":"  http://user:secret@proxy.example:8080  "}}"#;
        let sanitized = sanitize_doctor_raw(raw);

        assert!(sanitized.contains("\"  /tmp/logs  \""));
        assert!(sanitized.contains("\"  http://***@proxy.example:8080  \""));
        assert!(!sanitized.contains("user:secret"));
    }

    #[test]
    fn non_empty_copy_reports_success_only_after_the_backend_accepts_it() {
        let (mut app, _) = App::boot();
        app.clipboard = Box::new(RecordingClipboard);
        app.doctor.data = Some(doctor_read());

        let _task = app.update(Message::CopyDiagnosticsReport);

        let notice = app.notice.as_ref().expect("notice");
        assert_eq!(notice.kind, NoticeKind::Success);
        assert!(notice.text.contains("Diagnostics report copied"));
    }

    #[test]
    fn credential_bearing_doctor_fields_are_masked_before_view_and_copy() {
        let mut read = doctor_read();
        read.report.auth = Some(serde_json::json!({
            "loggedIn": true,
            "accessToken": "token-123"
        }));
        read.report.license = Some(serde_json::json!({
            "active": true,
            "serial": "LIC-123",
            "key": "KEY-456"
        }));
        read.raw = serde_json::to_string_pretty(&serde_json::json!({
            "platform": "linux",
            "proxy": {
                "url": "http://user:secret@proxy.example:8080",
                "source": "env"
            },
            "auth": { "accessToken": "token-123" },
            "license": { "serial": "LIC-123", "key": "KEY-456" }
        }))
        .expect("raw doctor json");

        let (mut app, _) = App::boot();
        app.page = Page::Diagnostics;
        app.doctor.data = Some(read);

        let vm = app.view_model();
        let PageVm::Diagnostics(diagnostics) = &vm.page else {
            panic!("expected the Diagnostics page");
        };
        let doctor = diagnostics.doctor.data().expect("doctor data");

        assert!(!doctor.raw.contains("user:secret"));
        assert!(!doctor.raw.contains("token-123"));
        assert!(!doctor.raw.contains("LIC-123"));
        assert!(!doctor.raw.contains("KEY-456"));
        assert!(doctor.raw.contains("http://***@proxy.example:8080"));
        assert!(doctor.raw.contains("\"accessToken\": \"***\""));
        assert!(doctor.raw.contains("\"serial\": \"***\""));
        assert!(doctor.raw.contains("\"key\": \"***\""));

        let auth = doctor.auth.as_deref().expect("auth summary");
        assert!(auth.contains("Access token: ***"));
        assert!(!auth.contains("token-123"));

        let license = doctor.license.as_deref().expect("license summary");
        assert!(license.contains("Serial: ***"));
        assert!(!license.contains("LIC-123"));
        assert!(!license.contains("KEY-456"));

        let report = app.diagnostics_report_text();
        assert!(!report.contains("user:secret"));
        assert!(!report.contains("token-123"));
        assert!(!report.contains("LIC-123"));
        assert!(!report.contains("KEY-456"));
        assert!(report.contains("http://***@proxy.example:8080"));
    }

    #[test]
    fn proxy_credentials_are_redacted_in_the_view_and_the_report() {
        let (mut app, _) = App::boot();
        app.page = Page::Diagnostics;
        app.environment.data = Some(EnvironmentInfo {
            proxy: Some(EnvironmentProxy {
                url: Some("http://user:secret@proxy.example:8080".to_string()),
                source: Some("env".to_string()),
                ..EnvironmentProxy::default()
            }),
            ..EnvironmentInfo::default()
        });

        let vm = app.view_model();
        let PageVm::Diagnostics(diagnostics) = &vm.page else {
            panic!("expected the Diagnostics page");
        };
        let proxy = diagnostics
            .environment
            .data()
            .expect("environment data")
            .proxy
            .as_ref()
            .expect("proxy");
        assert_eq!(proxy.url, "http://***@proxy.example:8080");

        let report = app.diagnostics_report_text();
        assert!(report.contains("Proxy: http://***@proxy.example:8080"));
        assert!(!report.contains("user:secret"));
    }

    #[test]
    fn combined_report_names_doctor_as_not_run_when_the_opt_in_read_is_idle() {
        let (mut app, _) = App::boot();
        app.environment.data = Some(environment_info());

        let report = app.diagnostics_report_text();

        assert!(report.contains("== Unity doctor =="));
        assert!(report.contains("Not run."));
        assert!(!report.contains("Raw report:"));
    }

    #[test]
    fn combined_report_contains_doctor_environment_and_proxy() {
        let (mut app, _) = App::boot();
        app.doctor.data = Some(doctor_read());
        app.environment.data = Some(environment_info());
        app.diagnose_proxy.data = Some("Proxy: http://proxy.example:8080\nSource: env".to_string());

        let report = app.diagnostics_report_text();
        assert!(report.contains("== Unity doctor =="));
        assert!(report.contains("Platform: linux x86_64"));
        assert!(report.contains("[pass] Path available"));
        assert!(report.contains("Raw report:"));
        assert!(report.contains("== Environment =="));
        assert!(report.contains("Proxy: http://proxy.example:8080"));
        assert!(report.contains("== Diagnose proxy =="));
        assert!(report.contains("Source: env"));
        assert!(report.contains("License: Active: false"));
    }

    #[test]
    fn copying_an_empty_report_reports_a_notice_without_panicking() {
        let (mut app, _) = App::boot();
        let _task = app.update(Message::CopyDiagnosticsReport);

        let notice = app.notice.as_ref().expect("notice");
        assert!(notice.text.contains("no report to copy"));
    }

    #[test]
    fn opening_diagnostics_starts_automatic_sections_but_not_doctor() {
        let (mut app, _) = App::boot();
        let _task = app.update(Message::Navigate(Page::Diagnostics));

        // unity doctor's credential probe can trigger a polkit prompt, so it
        // is opt-in instead of part of the automatic page load.
        assert!(
            !app.doctor.loading,
            "unity doctor must not run automatically"
        );
        assert!(app.environment.loading);
        assert!(app.diagnose_proxy.loading);
        assert!(app.diagnose_accelerator.loading);
        assert!(app.diagnose_update.loading);
    }

    #[test]
    fn load_doctor_action_starts_the_opt_in_read() {
        let (mut app, _) = App::boot();
        app.page = Page::Diagnostics;

        let _task = app.update(Message::from(UiAction::LoadDoctor));

        assert!(app.doctor.loading, "the explicit action must start doctor");
    }

    #[test]
    fn refreshing_diagnostics_does_not_rerun_doctor() {
        let (mut app, _) = App::boot();
        app.page = Page::Diagnostics;
        app.doctor.data = Some(doctor_read());

        let _task = app.update(Message::Refresh);

        assert!(
            !app.doctor.loading,
            "the page refresh must not re-run the credential probe"
        );
        assert!(app.environment.loading);
    }

    #[test]
    fn plugin_remediation_is_copy_only_until_the_plugins_ticket_lands() {
        let mut app = connected_app("1.0.0-beta.17");
        absorb(
            &mut app,
            serde_json::json!({
                "success": true,
                "notifications": [{
                    "code": "PLUGIN_UPDATE_AVAILABLE",
                    "message": "A plugin update is available.",
                    "data": { "plugin": "com.unity.ai", "current": "1.0.0", "latest": "1.1.0" },
                    "remediation": {
                        "command": "unity plugin upgrade com.unity.ai",
                        "requiresUserApproval": true
                    }
                }]
            }),
        );

        let entry = &app.view_model().shell.notifications[0];
        assert_eq!(entry.label, "PLUGIN_UPDATE_AVAILABLE");
        assert_eq!(
            entry.command.as_deref(),
            Some("unity plugin upgrade com.unity.ai")
        );
        // No builder exists for a plugin upgrade, and the CLI asked for an
        // explicit decision, so the command stays copy-only.
        assert!(!entry.can_run);
    }

    fn docs_changelog(version: &str) -> ChangelogView {
        ChangelogView {
            version: version.to_string(),
            sections: vec![ChangelogSection {
                title: "Added".to_string(),
                entries: vec![
                    ChangelogEntry::new("`unity install-modules --architecture` selects a build."),
                    ChangelogEntry::new("`unity editors default` stores the build."),
                    ChangelogEntry::new("`MonoBehaviour.Start` runs first."),
                    ChangelogEntry::new("A regular note without any token."),
                ],
            }],
            breaking_changes: Vec::new(),
            entry_count: 4,
            source: "unity-changelog".to_string(),
            source_detail: "Unity CLI 1.0.0-beta.17 release notes".to_string(),
        }
    }

    fn app_with_changelog(version: &str) -> App {
        let (mut app, _) = App::boot();
        app.page = Page::Changelog;
        app.changelog_loaded = true;
        app.changelog.data = Some(docs_changelog(version));
        app
    }

    #[test]
    fn changelog_filter_count_matches_the_entries_it_keeps() {
        let mut app = app_with_changelog("1.0.0-beta.17");
        app.changelog_app_impact_only = false;

        let vm = app.view_model();
        let PageVm::Changelog(page) = &vm.page else {
            panic!("expected the changelog page");
        };
        let data = page.state.data().expect("changelog data");
        let visible = data
            .sections
            .iter()
            .flat_map(|section| section.entries.iter())
            .filter(|entry| entry.app_impact)
            .count();
        assert_eq!(visible, 2);
        assert_eq!(data.app_impact_count, visible);
        assert!(!page.app_impact_only);

        let _ = app.update(Message::ToggleChangelogImpact(true));
        let vm = app.view_model();
        let PageVm::Changelog(page) = &vm.page else {
            panic!("expected the changelog page");
        };
        let data = page.state.data().expect("changelog data");
        let kept = data
            .sections
            .iter()
            .flat_map(|section| section.entries.iter())
            .filter(|entry| entry.app_impact)
            .count();
        assert!(page.app_impact_only);
        assert_eq!(data.app_impact_count, kept);
        assert_eq!(data.entry_count - kept, 2);
    }

    #[test]
    fn docs_states_map_from_the_session_cache_without_running_the_cli() {
        let mut app = app_with_changelog("1.0.0-beta.17");
        let version = "1.0.0-beta.17".to_string();

        app.docs_cache.insert(
            DocsKey {
                topic: "install-modules".to_string(),
                version: version.clone(),
                scope: DocsScope::Cli,
            },
            DocsPage {
                topic: "install-modules".to_string(),
                url: "https://docs.unity3d.com/Manual/EditorCommandLineArguments.html".to_string(),
                search: false,
            },
        );
        app.docs_errors.insert(
            DocsKey {
                topic: "editors default".to_string(),
                version: version.clone(),
                scope: DocsScope::Cli,
            },
            UnityError::new(DOCS_NOT_FOUND, "No documentation page was found."),
        );
        app.docs_errors.insert(
            DocsKey {
                topic: "MonoBehaviour.Start".to_string(),
                version: version.clone(),
                scope: DocsScope::ScriptReference,
            },
            UnityError::new("UNEXPECTED_OUTPUT", "The docs index could not be read."),
        );

        let vm = app.view_model();
        let PageVm::Changelog(page) = &vm.page else {
            panic!("expected the changelog page");
        };
        let entries = &page.state.data().expect("changelog data").sections[0].entries;

        assert!(matches!(
            &entries[0].docs,
            EntryDocsVm::Ready { url, search: false, .. } if url.contains("EditorCommandLineArguments")
        ));
        assert!(matches!(
            &entries[1].docs,
            EntryDocsVm::Missing { topic } if topic == "editors default"
        ));
        assert!(matches!(
            &entries[2].docs,
            EntryDocsVm::Failed { message, .. } if message.contains("docs index")
        ));
        assert!(matches!(&entries[3].docs, EntryDocsVm::Unavailable));
    }

    #[test]
    fn skills_vm_maps_clients_installs_and_every_outcome_row() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        app.project_context = Some("/home/ada/Games/Orbital".to_string());
        app.skill_clients.data = Some(vec![
            SkillClientInfo {
                client: "claude-code".to_string(),
                path: "/home/ada/.claude/skills".to_string(),
                installed: Some(true),
                ..SkillClientInfo::default()
            },
            SkillClientInfo {
                client: "cursor".to_string(),
                path: "/home/ada/.cursor/skills".to_string(),
                status: "absent".to_string(),
                ..SkillClientInfo::default()
            },
        ]);
        app.skill_installs.data = Some(vec![TrackedSkillInstall {
            path: "/home/ada/.claude/skills/unity-cli/SKILL.md".to_string(),
            client: "claude-code".to_string(),
            skill: "unity-cli".to_string(),
            status: "installed".to_string(),
            ..TrackedSkillInstall::default()
        }]);
        let statuses = [
            "refreshed",
            "up-to-date",
            "pruned",
            "skipped",
            "conflict",
            "would-update",
            "would-skip",
            "would-prune",
        ];
        app.skill_result = Some(SkillResult {
            dry_run: true,
            rows: statuses
                .iter()
                .enumerate()
                .map(|(index, status)| skill_row(status, (index == 3).then_some("Outdated copy.")))
                .collect(),
        });

        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };

        let clients = skills.clients.data().expect("clients");
        assert_eq!(clients[0].label, "Claude Code");
        assert_eq!(clients[0].client, "claude-code");
        assert_eq!(clients[0].path, "/home/ada/.claude/skills");
        assert!(clients[0].installed);
        assert!(!clients[1].installed);

        let installs = skills.installs.data().expect("installs");
        assert_eq!(installs[0].skill, "unity-cli");
        assert_eq!(installs[0].status, "installed");

        assert!(skills.local_available);
        let result = skills.result.as_ref().expect("result");
        assert!(result.dry_run);
        assert_eq!(
            result
                .rows
                .iter()
                .map(|row| row.outcome.clone())
                .collect::<Vec<_>>(),
            vec![
                SkillOutcomeVm::Refreshed,
                SkillOutcomeVm::UpToDate,
                SkillOutcomeVm::Pruned,
                SkillOutcomeVm::Skipped,
                SkillOutcomeVm::Conflict,
                SkillOutcomeVm::WouldUpdate,
                SkillOutcomeVm::WouldSkip,
                SkillOutcomeVm::WouldPrune,
            ]
        );
        assert_eq!(
            result.rows[3].skip_reason.as_deref(),
            Some("Outdated copy.")
        );
        // A dry-run row is labelled with the CLI's own would-* word.
        assert_eq!(result.rows[5].outcome.label(), "would-update");
        assert!(result.force.is_some(), "a conflict must offer --force");
        assert_eq!(skills.refresh_label, "Preview refresh (dry run)");
    }

    #[test]
    fn local_option_is_gated_on_a_known_project_context() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        app.project_context = None;
        app.skill_install_local = false;

        let _ = app.update(Message::SkillLocalToggled(true));
        assert!(!app.skill_install_local);
        assert!(app.notice.is_some());

        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };
        assert!(!skills.local_available);
        assert!(skills
            .local_disabled_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("project")));

        app.notice = None;
        app.project_context = Some("/home/ada/Games/Orbital".to_string());
        let _ = app.update(Message::SkillLocalToggled(true));
        assert!(app.skill_install_local);
        assert!(app.notice.is_none());

        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };
        assert!(skills.local_available);
        assert!(skills.install.local);
        assert_eq!(
            skills.project_context.as_deref(),
            Some("/home/ada/Games/Orbital")
        );
    }

    #[test]
    fn the_first_refresh_click_is_a_dry_run_that_labels_would_rows() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        assert!(app.next_skill_refresh_dry_run());

        app.skill_last_run = Some(SkillRun {
            scope: SkillScope::Refresh { yes: true },
            dry_run: true,
            force: false,
            project_context: None,
        });
        let frame = serde_json::json!({
            "type": "result",
            "success": true,
            "data": {
                "items": [
                    { "path": "/a", "client": "cursor", "skill": "unity-cli", "status": "would-update" },
                    { "path": "/b", "client": "codex", "skill": "unity-cli", "status": "would-skip", "skipReason": "Outdated copy." }
                ]
            }
        });
        app.note_skill_outcome("skill-refresh", JobState::Succeeded, Some(&frame));

        assert!(!app.next_skill_refresh_dry_run());
        let result = app.skill_result.as_ref().expect("result");
        assert!(result.dry_run);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            result.rows[0].outcome(),
            unity_core::models::SkillOutcome::WouldUpdate
        );
        assert_eq!(
            result.rows[1].outcome(),
            unity_core::models::SkillOutcome::WouldSkip
        );

        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };
        assert_eq!(skills.refresh_label, "Refresh now");
        let result = skills.result.as_ref().expect("result");
        assert_eq!(result.rows[0].outcome, SkillOutcomeVm::WouldUpdate);
        assert_eq!(result.rows[1].outcome, SkillOutcomeVm::WouldSkip);
        assert_eq!(
            result.rows[1].skip_reason.as_deref(),
            Some("Outdated copy.")
        );
    }

    #[test]
    fn the_same_advisory_is_admitted_only_once() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        let advisory = store_advisory(
            "CLI_UPDATE_AVAILABLE",
            "A new CLI version is available.",
            None,
        );

        assert!(app.notifications.push_advisory(&advisory));
        assert!(!app.notifications.push_advisory(&advisory));
        assert_eq!(app.notifications.snapshot().len(), 1);
    }

    #[test]
    fn a_successful_remediation_returns_to_idle_and_refreshes_the_page() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        app.notifications.push_advisory(&store_advisory(
            "CLI_COMPATIBILITY",
            "Compatibility mode is on.",
            Some(serde_json::json!({
                "command": "unity status",
                "requiresUserApproval": false
            })),
        ));
        let id = app.notifications.snapshot()[0].id;
        app.remediating = Some(id);
        app.notifications.set_state(id, ActionState::Running);

        let task = app.apply_job_terminal("remediation", JobState::Succeeded, Task::none());

        assert_eq!(
            app.notifications.get(id).map(|entry| entry.state),
            Some(ActionState::Idle),
            "a successful remediation is runnable again"
        );
        assert!(app.remediating.is_none(), "the finished job is taken");
        assert!(
            task.units() > 0,
            "the success path batches the page refresh"
        );
    }

    #[test]
    fn the_start_refresh_message_starts_the_dry_run_argv_on_the_first_click() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        app.cli.set_override_path(Some(startable_cli()));

        let _ = app.update(Message::StartSkillRefresh);
        let job = app.active_job.as_ref().expect("the refresh job started");
        assert_eq!(job.kind, "skill-refresh");
        assert!(job.command.contains("skill refresh"));
        assert!(job.command.contains("--yes"));
        assert!(
            job.command.contains("--dry-run"),
            "the first refresh message must start the dry-run argv: {}",
            job.command
        );
        assert!(!job.command.contains("--force"));
        let _ = app.update(Message::CancelJob);

        // After a preview, the same message applies instead of previewing.
        app.skill_refresh_previewed = true;
        app.active_job = None;
        let _ = app.update(Message::StartSkillRefresh);
        let job = app.active_job.as_ref().expect("the apply job started");
        assert!(!job.command.contains("--dry-run"));
        let _ = app.update(Message::CancelJob);
    }

    #[test]
    fn entering_the_skills_page_resets_the_preview_cycle() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Overview;
        app.skill_refresh_previewed = true;

        let _ = app.update(Message::Navigate(Page::Skills));

        assert_eq!(app.page, Page::Skills);
        assert!(
            !app.skill_refresh_previewed,
            "the first refresh click after opening the page must be a dry run"
        );
    }

    #[test]
    fn a_failed_remediation_stays_failed_without_a_refresh() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        app.notifications.push_advisory(&store_advisory(
            "CLI_COMPATIBILITY",
            "Compatibility mode is on.",
            Some(serde_json::json!({
                "command": "unity status",
                "requiresUserApproval": false
            })),
        ));
        let id = app.notifications.snapshot()[0].id;
        app.remediating = Some(id);
        app.notifications.set_state(id, ActionState::Running);

        let task = app.apply_job_terminal("remediation", JobState::Failed, Task::none());

        assert_eq!(
            app.notifications.get(id).map(|entry| entry.state),
            Some(ActionState::Failed)
        );
        assert!(app.remediating.is_none(), "the finished job is taken");
        assert_eq!(task.units(), 0, "a failed remediation does not refresh");
    }

    #[test]
    fn advisories_absorbed_once_each() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        let warning = store_advisory(
            "INSTALLER_DISK_SPACE",
            "Only 6.4 GB is free on the install volume.",
            None,
        );

        app.cli
            .advisory_sink()
            .push([warning.clone(), warning.clone()]);
        app.absorb_advisories();
        assert_eq!(
            app.notifications.snapshot().len(),
            1,
            "a repeated warning collapses"
        );

        let distinct = store_advisory(
            "INSTALLER_DISK_SPACE",
            "Only 2.1 GB is free on the install volume.",
            None,
        );
        app.cli.advisory_sink().push([distinct]);
        app.absorb_advisories();
        assert_eq!(
            app.notifications.snapshot().len(),
            2,
            "a different warning is a second entry"
        );
    }

    #[test]
    fn copying_a_notification_command_confirms_on_the_clipboard() {
        let (mut app, _) = App::boot();
        app.notifications.clear();
        app.notifications.push_advisory(&store_advisory(
            "CLI_UPDATE_AVAILABLE",
            "A new CLI version is available.",
            Some(serde_json::json!({
                "command": "unity self-update",
                "requiresUserApproval": true
            })),
        ));
        let id = app.notifications.snapshot()[0].id;
        app.clipboard = Box::new(RecordingClipboard);

        let _ = app.update(Message::CopyNotificationCommand(id));

        let notice = app.notice.as_ref().expect("copy confirmation");
        assert_eq!(notice.text, "Command copied to the clipboard.");
        // The confirmation is a notice, so the store keeps only the advisory.
        assert_eq!(app.notifications.unread(), 1);
    }

    #[test]
    fn start_skill_install_refuses_local_without_a_project_context() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        app.cli.set_override_path(Some(startable_cli()));
        app.project_context = None;
        // Simulate a stale or replayed form state: the controller must still
        // refuse `--local` without a project context.
        app.skill_install_local = true;

        let _ = app.update(Message::InstallSkill("cursor".to_string()));

        assert!(app.active_job.is_none());
        let notice = app.notice.as_ref().expect("refusal notice");
        assert!(notice.text.contains("--local"));
        assert!(notice.text.contains("project"));
    }

    #[test]
    fn force_skill_last_run_refuses_without_a_conflict_row() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        app.cli.set_override_path(Some(startable_cli()));
        app.skill_last_run = Some(SkillRun {
            scope: SkillScope::Install {
                client: Some("vscode".to_string()),
                all: false,
                local: false,
                skill: Some("unity-cli".to_string()),
                yes: false,
            },
            dry_run: false,
            force: false,
            project_context: None,
        });
        app.skill_result = Some(SkillResult {
            dry_run: false,
            rows: vec![skill_row("refreshed", None)],
        });

        let _ = app.update(Message::ForceSkillLastRun);

        assert!(
            app.active_job.is_none(),
            "a force run without a conflict row must not start"
        );
        let notice = app.notice.as_ref().expect("refusal notice");
        assert!(notice.text.contains("conflict"));

        // The same action is honoured once the result actually has a conflict.
        app.skill_result = Some(SkillResult {
            dry_run: false,
            rows: vec![skill_row("conflict", Some("Not written by the CLI."))],
        });
        let _ = app.update(Message::ForceSkillLastRun);
        let job = app.active_job.as_ref().expect("the forced job started");
        assert!(job.command.contains("--force"));
        let _ = app.update(Message::CancelJob);
    }

    #[test]
    fn force_explanation_names_the_all_and_refresh_scopes_without_duplicating_the() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        app.skill_result = Some(SkillResult {
            dry_run: false,
            rows: vec![skill_row("conflict", Some("Not written by the CLI."))],
        });
        app.skill_last_run = Some(SkillRun {
            scope: SkillScope::Install {
                client: None,
                all: true,
                local: false,
                skill: None,
                yes: false,
            },
            dry_run: false,
            force: false,
            project_context: None,
        });

        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };
        let explanation = &skills
            .result
            .as_ref()
            .expect("result")
            .force
            .as_ref()
            .expect("force offer")
            .explanation;
        assert!(!explanation.contains("the the"), "{explanation}");
        assert!(
            explanation.contains("the `--all` install copy"),
            "{explanation}"
        );

        app.skill_last_run = Some(SkillRun {
            scope: SkillScope::Refresh { yes: true },
            dry_run: false,
            force: false,
            project_context: None,
        });
        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };
        let explanation = &skills
            .result
            .as_ref()
            .expect("result")
            .force
            .as_ref()
            .expect("force offer")
            .explanation;
        assert!(!explanation.contains("the the"), "{explanation}");
        assert!(explanation.contains("the refresh copy"), "{explanation}");
    }

    #[test]
    fn skill_rows_parse_items_and_the_single_skipped_frame() {
        let frame = serde_json::json!({
            "type": "result",
            "success": true,
            "data": {
                "items": [
                    { "path": "/a", "client": "cursor", "skill": "unity-cli", "status": "refreshed" },
                    { "path": "/b", "client": "codex", "skill": "unity-cli", "outcome": "up-to-date" }
                ]
            }
        });
        let rows = skill_rows_from_result(&frame, None);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].outcome(),
            unity_core::models::SkillOutcome::Refreshed
        );
        assert_eq!(
            rows[1].outcome(),
            unity_core::models::SkillOutcome::UpToDate
        );

        let skipped = serde_json::json!({
            "type": "result",
            "success": true,
            "data": { "skipped": true, "skipReason": "The copy is older." }
        });
        let run = SkillRun {
            scope: SkillScope::Install {
                client: Some("cursor".to_string()),
                all: false,
                local: false,
                skill: Some("unity-cli".to_string()),
                yes: false,
            },
            dry_run: false,
            force: false,
            project_context: None,
        };
        let rows = skill_rows_from_result(&skipped, Some(&run));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client, "cursor");
        assert_eq!(rows[0].skill, "unity-cli");
        assert_eq!(rows[0].outcome(), unity_core::models::SkillOutcome::Skipped);
        assert_eq!(rows[0].reason(), Some("The copy is older."));

        // An explicit empty items array is the CLI saying "nothing tracked",
        // not a reason to invent a row.
        let empty = serde_json::json!({ "data": { "items": [] } });
        assert!(skill_rows_from_result(&empty, Some(&run)).is_empty());
    }

    #[test]
    fn skill_force_is_only_offered_after_a_conflict_and_names_the_request() {
        let mut app = connected_app("1.0.0-beta.17");
        // The command preview needs a resolvable CLI; without a pinned path it
        // degrades to the static fallback on machines without a `unity` binary.
        app.cli.set_override_path(Some(temporary_cli()));
        app.page = Page::Skills;
        app.skill_last_run = Some(SkillRun {
            scope: SkillScope::Install {
                client: Some("vscode".to_string()),
                all: false,
                local: false,
                skill: Some("unity-cli".to_string()),
                yes: true,
            },
            dry_run: false,
            force: false,
            project_context: None,
        });

        app.skill_result = Some(SkillResult {
            dry_run: false,
            rows: vec![skill_row("refreshed", None)],
        });
        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };
        assert!(skills.result.as_ref().expect("result").force.is_none());

        app.skill_result = Some(SkillResult {
            dry_run: false,
            rows: vec![skill_row("conflict", Some("Not written by the CLI."))],
        });
        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };
        let force = skills
            .result
            .as_ref()
            .expect("result")
            .force
            .as_ref()
            .expect("force offer");
        assert!(force.explanation.contains("did not write"));
        assert!(force.command.contains("--force"));
        assert!(force.command.contains("vscode"));
        assert_eq!(force.scope, "`vscode`");
    }

    #[test]
    fn every_skill_read_has_its_own_retry_target() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        app.skill_clients.error = Some(UnityError::new("TIMEOUT", "The read timed out."));
        app.skill_installs.error = Some(UnityError::new("TIMEOUT", "The read timed out."));
        app.note_read_failed(ReadKind::SkillClients);
        app.note_read_failed(ReadKind::SkillInstalls);

        let vm = app.view_model();
        let PageVm::Skills(skills) = &vm.page else {
            panic!("expected the Skills page");
        };
        assert_eq!(
            retry_kind(&skills.clients),
            &ErrorActionKind::Retry("read:skill-clients".to_string())
        );
        assert_eq!(
            retry_kind(&skills.installs),
            &ErrorActionKind::Retry("read:skill-installs".to_string())
        );
    }

    #[test]
    fn a_finished_skill_job_reloads_the_reads_without_a_restart() {
        let mut app = connected_app("1.0.0-beta.17");
        app.page = Page::Skills;
        app.skill_last_run = Some(SkillRun {
            scope: SkillScope::Refresh { yes: true },
            dry_run: false,
            force: false,
            project_context: None,
        });
        let frame = serde_json::json!({
            "type": "result",
            "success": true,
            "data": {
                "items": [
                    { "path": "/a", "client": "cursor", "skill": "unity-cli", "status": "refreshed" }
                ]
            }
        });

        let _ = app.handle_finished_job(
            "skill-refresh".to_string(),
            JobState::Succeeded,
            None,
            Some(frame),
        );

        assert_eq!(
            app.skill_result.as_ref().expect("result").rows[0].outcome(),
            unity_core::models::SkillOutcome::Refreshed
        );
        // Both reads are restarted so the new status is reflected on the next
        // frame without an app restart.
        assert!(app.skill_clients.loading);
        assert!(app.skill_installs.loading);
    }

    fn skill_row(status: &str, reason: Option<&str>) -> SkillRefreshRow {
        SkillRefreshRow {
            path: "/home/ada/.cursor/skills/unity-cli/SKILL.md".to_string(),
            client: "cursor".to_string(),
            skill: "unity-cli".to_string(),
            status: status.to_string(),
            skip_reason: reason.map(str::to_string),
            ..SkillRefreshRow::default()
        }
    }

    #[test]
    fn docs_cache_is_scoped_to_the_loaded_version_and_scope() {
        let mut app = app_with_changelog("1.0.0-beta.18");
        app.docs_cache.insert(
            DocsKey {
                topic: "install-modules".to_string(),
                version: "1.0.0-beta.17".to_string(),
                scope: DocsScope::Cli,
            },
            DocsPage {
                topic: "install-modules".to_string(),
                url: "https://docs.unity3d.com/old.html".to_string(),
                search: false,
            },
        );

        let vm = app.view_model();
        let PageVm::Changelog(page) = &vm.page else {
            panic!("expected the changelog page");
        };
        let first = &page.state.data().expect("changelog data").sections[0].entries[0];
        assert!(matches!(
            &first.docs,
            EntryDocsVm::Idle { topic, scope } if topic == "install-modules" && *scope == DocsScopeVm::Cli
        ));
    }
}
