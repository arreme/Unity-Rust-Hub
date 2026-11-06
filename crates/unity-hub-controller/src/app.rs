use std::collections::{BTreeSet, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use iced::{Subscription, Task};
use serde_json::{Map, Value};
use unity_core::bootstrap;
use unity_core::changelog::{ChangelogRequest, ChangelogView, CliChangelogProvider};
use unity_core::errors::{redact_proxy, remediation_for, RemediationKind};
use unity_core::models::{
    AuthStatus, CliStatus, CliUpdateInfo, DoctorCheck, Editor, EnvironmentInfo, EnvironmentProxy,
    InstallPreview, LicenseStatus, Module, Project, Release, RunningEditor, RunningEditors,
};
use unity_core::{
    AdvisoryOrigin, CliAdvisory, CommandSpec, DiagnoseTopic as CliDiagnose, DoctorRead, UnityCli,
    UnityError,
};
use unity_hub_model::{
    AccountCardVm, AdvisoryVm, AuthVm, BadgeVm, BreakingChangeVm, ChangelogDataVm,
    ChangelogSectionVm, ChangelogVm, CheckStatus, CliCardVm, CliConnectionVm, CliDetailsVm,
    DiagnoseReportVm, DiagnoseTopic, DiagnosticsVm, DoctorCheckVm, DoctorVm, EditorModulesVm,
    EditorRowVm, EditorsVm, EnvironmentVm, ErrorActionKind, ErrorActionVm, InstallCliVm,
    InstallEditorVm, InstallPreviewVm, JobLogVm, JobPhase, JobVm, LicenseRowVm, LicenseVm,
    LoadState, ModalVm, ModuleOptionVm, NoticeVm, OverviewVm, Page, PageVm, ProjectRowVm,
    ProjectsVm, ProxyVm, ReleaseRowVm, RunningEditorVm, SectionKind, SettingsVm, ShellVm,
    TextScaleOptionVm, Tone, UiAction, UiError, UpdateVm, UpgradeProjectVm, ViewModel,
};

use crate::jobs::{JobManager, JobStarted, JobState, StreamKind};
use crate::settings::{self, AppSettings, TextSize};
use crate::util::{
    error_text, format_bytes, format_epoch_millis, format_iso, run_blocking, short_path,
};

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    InstallCli,
    EditorModules(Editor),
    UpgradeProject(Project),
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
        }
    }
}

/// A read the controller can repeat after a retryable failure. Parameterised
/// reads keep the request they were started with, so Retry re-runs the exact
/// query the user saw fail instead of whatever the form contains now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryRequest {
    Read(ReadKind),
    Modules(String),
    DialogModules {
        version: String,
        architecture: String,
    },
    PreviewInstall {
        version: String,
        modules: Vec<String>,
        headless: bool,
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

struct OperatingSystemClipboard;

impl ClipboardBackend for OperatingSystemClipboard {
    fn write(&self, text: &str) -> Result<(), String> {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|error| format!("clipboard unavailable: {error}"))?;
        clipboard
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

#[derive(Debug, Clone)]
pub struct LogLine {
    pub stream: StreamKind,
    pub text: String,
}

/// One advisory kept in controller state. The identity is computed once, when
/// the advisory is absorbed, so dedupe, dismissal, and actions all agree.
#[derive(Debug, Clone)]
pub struct AdvisoryEntry {
    pub advisory: CliAdvisory,
    pub id: String,
    /// The remediation kind the app can run after an explicit click. `None`
    /// when the CLI named a remediation the adapter has no builder for: the
    /// banner still shows the command and offers Copy.
    pub run: Option<AdvisoryRun>,
}

/// A remediation the adapter knows how to build as argv. The command itself is
/// built on click, so it always uses the CLI path configured at that moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvisoryRun {
    CliUpdate,
}

impl AdvisoryRun {
    fn label(self) -> &'static str {
        match self {
            Self::CliUpdate => "Update CLI",
        }
    }

    fn job_kind(self) -> &'static str {
        match self {
            Self::CliUpdate => "cli-update",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActiveJob {
    pub id: String,
    pub kind: String,
    pub command: String,
    pub state: JobState,
    pub progress: f32,
    pub message: Option<String>,
    pub logs: VecDeque<LogLine>,
    pub error: Option<UnityError>,
}

impl ActiveJob {
    fn new(started: JobStarted) -> Self {
        Self {
            id: started.id,
            kind: started.kind,
            command: started.command,
            state: JobState::Running,
            progress: 0.0,
            message: None,
            logs: VecDeque::new(),
            error: None,
        }
    }

    fn is_running(&self) -> bool {
        self.state == JobState::Running
    }
}

pub struct App {
    pub cli: UnityCli,
    pub jobs: JobManager,
    clipboard: Box<dyn ClipboardBackend>,
    pub page: Page,
    pub cli_status: Loadable<CliStatus>,
    pub update: Loadable<CliUpdateInfo>,
    pub auth: Loadable<AuthStatus>,
    pub license: Loadable<LicenseStatus>,
    pub running: Loadable<RunningEditors>,
    pub editors: Loadable<Vec<Editor>>,
    pub releases: Loadable<Vec<Release>>,
    pub projects: Loadable<Vec<Project>>,
    pub changelog: Loadable<ChangelogView>,
    pub changelog_loaded: bool,
    pub changelog_target: Option<String>,
    pub show_raw_changelog: bool,
    pub raw_changelog: Loadable<String>,
    pub doctor: Loadable<DoctorRead>,
    pub environment: Loadable<EnvironmentInfo>,
    pub diagnose_proxy: Loadable<String>,
    pub diagnose_accelerator: Loadable<String>,
    pub diagnose_update: Loadable<String>,
    pub modules: Loadable<Vec<Module>>,
    pub dialog_modules: Loadable<Vec<Module>>,
    pub modal: Modal,
    pub cli_channel: String,
    pub cli_consent: bool,
    pub install_version: String,
    pub selected_modules: BTreeSet<String>,
    pub install_headless: bool,
    pub accept_eula: bool,
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
    pub notice: Option<Notice>,
    pub advisories: Vec<AdvisoryEntry>,
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
    DismissNotice,
    RunErrorAction(ErrorActionVm),
    CopyDiagnosticsReport,
    CopyDiagnoseReport(DiagnoseTopic),
    DismissAdvisory(String),
    CopyAdvisoryCommand(String),
    RunAdvisoryRemediation(String),
    SignIn,
    TextSizeChanged(TextSize),
    OpenInstallCli,
    CliChannelChanged(String),
    CliConsentToggled(bool),
    InstallCli,
    UpdateCli,
    InstallVersionChanged(String),
    LoadModules,
    ToggleModule(String, bool),
    SelectAllModules(bool),
    HeadlessToggled(bool),
    AcceptEulaToggled(bool),
    PreviewInstall,
    StartInstallEditor,
    UseRelease(String),
    OpenModules(String, String),
    CloseModal,
    ToggleDialogModule(String, bool),
    DialogEulaToggled(bool),
    StartInstallModules,
    ProjectSearchChanged(String),
    OpenProject(String),
    OpenUpgrade(String),
    UpgradeTargetChanged(String),
    UpgradeConfirmed(bool),
    StartUpgrade,
    ChangelogTargetChanged(String),
    LoadChangelog,
    ToggleRawChangelog,
    CliPathChanged(String),
    SaveCliPath,
    ClearCliPath,

    // Async results and job lifecycle stay controller-private.
    StatusLoaded(Result<CliStatus, UnityError>),
    UpdateLoaded(Result<CliUpdateInfo, UnityError>),
    AuthLoaded(Result<AuthStatus, UnityError>),
    LicenseLoaded(Result<LicenseStatus, UnityError>),
    RunningLoaded(Result<RunningEditors, UnityError>),
    EditorsLoaded(Result<Vec<Editor>, UnityError>),
    ReleasesLoaded(Result<Vec<Release>, UnityError>),
    ProjectsLoaded(Result<Vec<Project>, UnityError>),
    ModulesLoaded(Result<Vec<Module>, UnityError>),
    DialogModulesLoaded(Result<Vec<Module>, UnityError>),
    PreviewLoaded(Result<Value, UnityError>),
    ChangelogLoaded(Result<ChangelogView, UnityError>),
    RawChangelogLoaded(Result<String, UnityError>),
    DoctorLoaded(Result<DoctorRead, UnityError>),
    EnvironmentLoaded(Result<EnvironmentInfo, UnityError>),
    DiagnoseProxyLoaded(Result<String, UnityError>),
    DiagnoseAcceleratorLoaded(Result<String, UnityError>),
    DiagnoseUpdateLoaded(Result<String, UnityError>),
    PollJobs,
    ProjectOpened(Result<(), UnityError>),
}

impl From<UiAction> for Message {
    fn from(action: UiAction) -> Self {
        match action {
            UiAction::Navigate(page) => Self::Navigate(page),
            UiAction::Refresh => Self::Refresh,
            UiAction::DismissNotice => Self::DismissNotice,
            UiAction::RunErrorAction(action) => Self::RunErrorAction(action),
            UiAction::CopyDiagnosticsReport => Self::CopyDiagnosticsReport,
            UiAction::CopyDiagnoseReport(topic) => Self::CopyDiagnoseReport(topic),
            UiAction::DismissAdvisory(id) => Self::DismissAdvisory(id),
            UiAction::CopyAdvisoryCommand(id) => Self::CopyAdvisoryCommand(id),
            UiAction::RunAdvisoryRemediation(id) => Self::RunAdvisoryRemediation(id),
            UiAction::DismissJob => Self::DismissJob,
            UiAction::CancelJob => Self::CancelJob,
            UiAction::SignIn => Self::SignIn,
            UiAction::TextScaleChanged(percent) => {
                Self::TextSizeChanged(TextSize::from_percent(percent))
            }
            UiAction::OpenInstallCli => Self::OpenInstallCli,
            UiAction::CliChannelChanged(channel) => Self::CliChannelChanged(channel),
            UiAction::CliConsentToggled(value) => Self::CliConsentToggled(value),
            UiAction::InstallCli => Self::InstallCli,
            UiAction::UpdateCli => Self::UpdateCli,
            UiAction::InstallVersionChanged(version) => Self::InstallVersionChanged(version),
            UiAction::LoadModules => Self::LoadModules,
            UiAction::ToggleModule(id, value) => Self::ToggleModule(id, value),
            UiAction::SelectAllModules(value) => Self::SelectAllModules(value),
            UiAction::HeadlessToggled(value) => Self::HeadlessToggled(value),
            UiAction::AcceptEulaToggled(value) => Self::AcceptEulaToggled(value),
            UiAction::PreviewInstall => Self::PreviewInstall,
            UiAction::StartInstallEditor => Self::StartInstallEditor,
            UiAction::UseRelease(version) => Self::UseRelease(version),
            UiAction::OpenEditorModules(version, architecture) => {
                Self::OpenModules(version, architecture)
            }
            UiAction::CloseModal => Self::CloseModal,
            UiAction::ToggleDialogModule(id, value) => Self::ToggleDialogModule(id, value),
            UiAction::DialogEulaToggled(value) => Self::DialogEulaToggled(value),
            UiAction::StartInstallModules => Self::StartInstallModules,
            UiAction::ProjectSearchChanged(value) => Self::ProjectSearchChanged(value),
            UiAction::OpenProject(path) => Self::OpenProject(path),
            UiAction::OpenProjectUpgrade(path) => Self::OpenUpgrade(path),
            UiAction::UpgradeTargetChanged(version) => Self::UpgradeTargetChanged(version),
            UiAction::UpgradeConfirmed(value) => Self::UpgradeConfirmed(value),
            UiAction::StartUpgrade => Self::StartUpgrade,
            UiAction::ChangelogTargetChanged(target) => Self::ChangelogTargetChanged(target),
            UiAction::LoadChangelog => Self::LoadChangelog,
            UiAction::ToggleRawChangelog => Self::ToggleRawChangelog,
            UiAction::CliPathChanged(value) => Self::CliPathChanged(value),
            UiAction::SaveCliPath => Self::SaveCliPath,
            UiAction::ClearCliPath => Self::ClearCliPath,
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
            clipboard: Box::new(OperatingSystemClipboard),
            page: Page::Overview,
            cli_status: Loadable::default(),
            update: Loadable::default(),
            auth: Loadable::default(),
            license: Loadable::default(),
            running: Loadable::default(),
            editors: Loadable::default(),
            releases: Loadable::default(),
            projects: Loadable::default(),
            changelog: Loadable::default(),
            changelog_loaded: false,
            changelog_target: None,
            show_raw_changelog: false,
            raw_changelog: Loadable::default(),
            doctor: Loadable::default(),
            environment: Loadable::default(),
            diagnose_proxy: Loadable::default(),
            diagnose_accelerator: Loadable::default(),
            diagnose_update: Loadable::default(),
            modules: Loadable::default(),
            dialog_modules: Loadable::default(),
            modal: Modal::None,
            cli_channel: "stable".to_string(),
            cli_consent: false,
            install_version: "lts".to_string(),
            selected_modules: BTreeSet::new(),
            install_headless: false,
            accept_eula: false,
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
            notice: None,
            advisories: Vec::new(),
            retry_targets: std::collections::HashMap::new(),
            boot_background_started: false,
        };
        // The boot status read is the first failing read the user can retry.
        app.retry_targets.insert(
            ReadKind::Status.id().to_string(),
            RetryRequest::Read(ReadKind::Status),
        );

        // The status read is the only critical boot work. The rest of the
        // Overview is loaded after it succeeds, one CLI process at a time.
        (app, load_status(cli))
    }

    pub fn text_scale(&self) -> f32 {
        self.app_settings.text_size().scale()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // Only wake the update loop while a job is streaming. An always-on
        // timer keeps the app awake for no reason and prevents the process from
        // idling when nothing is running.
        if !self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false)
        {
            return Subscription::none();
        }

        // iced 0.14 no longer exposes `time::every`. JobManager wakes this
        // subscription only when it actually has an event, and the notifier
        // coalesces bursts into at most one UI poll every 100 ms.
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
            Message::StatusLoaded(result) => {
                let previous_version = self.cli_version_key();
                let installed = match result {
                    Ok(status) => {
                        if let Some(version) = &status.version {
                            if version.channel == "beta" || version.channel == "alpha" {
                                self.cli_channel = version.channel.clone();
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
                    if installed {
                        self.load_overview_data(false)
                    } else {
                        Task::none()
                    }
                }
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
                match result {
                    Ok(editors) => {
                        self.editors.succeeded(editors);
                        self.clear_retry(ReadKind::Editors);
                        self.ensure_upgrade_target();
                    }
                    Err(error) => {
                        self.editors.failed(error);
                        self.note_read_failed(ReadKind::Editors);
                    }
                }
                Task::none()
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
            Message::PollJobs => self.poll_jobs(),
            Message::DismissJob => {
                self.active_job = None;
                Task::none()
            }
            Message::CancelJob => {
                if let Some(job) = &self.active_job {
                    if self.jobs.cancel(&job.id) {
                        self.notice = Some(Notice::info("Cancelling the job…"));
                    }
                }
                Task::none()
            }
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
            Message::DismissAdvisory(id) => {
                let version = self.cli_version_key();
                self.app_settings.dismiss_advisory(&version, &id);
                self.notice = Some(match settings::save(&self.app_settings) {
                    Ok(()) => Notice::info("Advisory dismissed for this CLI version."),
                    Err(error) => Notice::error(error_text(&error)),
                });
                Task::none()
            }
            Message::CopyAdvisoryCommand(id) => {
                let Some(command) = self.advisory_command(&id) else {
                    return Task::none();
                };
                self.notice = Some(Notice::success("Command copied to the clipboard."));
                iced::clipboard::write(command)
            }
            Message::RunAdvisoryRemediation(id) => {
                let Some(entry) = self.advisories.iter().find(|entry| entry.id == id) else {
                    return Task::none();
                };
                let Some(run) = entry.run else {
                    self.notice = Some(Notice::error(
                        "This advisory's command cannot run from the app. Copy it and run it in a terminal.",
                    ));
                    return Task::none();
                };
                // An explicit click is the only path to this command; nothing
                // here is ever triggered by a read or a refresh.
                match self.advisory_spec(run) {
                    Ok(spec) => {
                        self.start_job(run.job_kind(), spec);
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::SignIn => {
                match self.cli.auth_login_spec() {
                    Ok(spec) => {
                        self.start_job("auth-login", spec);
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::TextSizeChanged(size) => {
                self.app_settings.text_scale_percent = size.percent();
                match settings::save(&self.app_settings) {
                    Ok(()) => {
                        self.notice = Some(Notice::success(format!("Text size set to {size}.")));
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
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
                match self.cli.self_update_spec() {
                    Ok(spec) => {
                        self.start_job("cli-update", spec);
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
            Message::LoadModules => {
                self.modules.begin();
                self.selected_modules.clear();
                self.preview = None;
                self.preview_raw = None;
                self.preview_error = None;
                let version = self.install_version.trim().to_string();
                self.retry_targets.insert(
                    ReadKind::Modules.id().to_string(),
                    RetryRequest::Modules(version.clone()),
                );
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version, None)
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
            Message::PreviewInstall => {
                self.preview_loading = true;
                self.preview_error = None;
                let version = self.install_version.trim().to_string();
                let modules: Vec<String> = self.selected_modules.iter().cloned().collect();
                let headless = self.install_headless;
                self.retry_targets.insert(
                    ReadKind::PreviewInstall.id().to_string(),
                    RetryRequest::PreviewInstall {
                        version: version.clone(),
                        modules: modules.clone(),
                        headless,
                    },
                );
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.dry_run_install(&version, &modules, headless)
                    }),
                    Message::PreviewLoaded,
                )
            }
            Message::StartInstallEditor => {
                let modules: Vec<String> = self.selected_modules.iter().cloned().collect();
                match self.cli.install_editor_spec(
                    &self.install_version,
                    &modules,
                    self.install_headless,
                    self.accept_eula,
                ) {
                    Ok(spec) => {
                        if self.start_job("editor-install", spec) {
                            self.modules.data = None;
                            self.selected_modules.clear();
                            self.install_headless = false;
                            self.accept_eula = false;
                        }
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::UseRelease(version) => {
                self.install_version = version.clone();
                self.modules.begin();
                self.selected_modules.clear();
                self.preview_error = None;
                self.retry_targets.insert(
                    ReadKind::Modules.id().to_string(),
                    RetryRequest::Modules(version.clone()),
                );
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version, None)
                    }),
                    Message::ModulesLoaded,
                )
            }
            Message::OpenModules(version, architecture) => {
                let Some(editor) = self
                    .editors
                    .data
                    .as_ref()
                    .and_then(|editors| {
                        editors
                            .iter()
                            .find(|editor| {
                                editor.version == version && editor.architecture == architecture
                            })
                            .or_else(|| editors.iter().find(|editor| editor.version == version))
                    })
                    .cloned()
                else {
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
            Message::StartInstallModules => {
                let Modal::EditorModules(editor) = &self.modal else {
                    return Task::none();
                };
                if self.dialog_selected.is_empty() || !self.dialog_eula {
                    return Task::none();
                }
                let modules: Vec<String> = self.dialog_selected.iter().cloned().collect();
                let architecture = editor.architecture.clone();
                match self.cli.install_modules_spec(
                    &editor.version,
                    Some(&architecture),
                    &modules,
                    self.dialog_eula,
                ) {
                    Ok(spec) => {
                        if self.start_job("module-install", spec) {
                            self.modal = Modal::None;
                            self.dialog_selected.clear();
                        }
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::ProjectSearchChanged(value) => {
                self.project_search = value;
                Task::none()
            }
            Message::OpenProject(path) => {
                let Some(project) = self
                    .projects
                    .data
                    .as_ref()
                    .and_then(|projects| projects.iter().find(|project| project.path == path))
                else {
                    return Task::none();
                };
                let title = project.title.clone();
                self.notice = Some(Notice::info(format!("Opening {title}…")));
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.open_project(&path)
                    }),
                    Message::ProjectOpened,
                )
            }
            Message::ProjectOpened(result) => {
                match result {
                    Ok(()) => {
                        self.notice = Some(Notice::success("Unity Editor is launching."));
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
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
                    self.notice = Some(Notice::error(format!(
                        "Close the running Unity Editor for {label} before upgrading this project."
                    )));
                    return Task::none();
                }
                match self.cli.upgrade_project_spec(&project_path, &target) {
                    Ok(spec) => {
                        if self.start_job("project-upgrade", spec) {
                            self.modal = Modal::None;
                        }
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
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
                        self.notice = Some(Notice::error(format!(
                            "`{value}` is not a file. Choose the unity executable."
                        )));
                        return Task::none();
                    }
                }
                match settings::save(&settings) {
                    Ok(()) => {
                        self.app_settings = settings;
                        self.cli.set_override_path(
                            self.app_settings.cli_path.as_ref().map(PathBuf::from),
                        );
                        self.notice = Some(Notice::success("CLI path saved."));
                        self.refresh_status()
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
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
                        self.notice = Some(Notice::success("CLI path override cleared."));
                        self.refresh_status()
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                        Task::none()
                    }
                }
            }
        }
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
            RetryRequest::Read(ReadKind::Modules)
            | RetryRequest::Read(ReadKind::DialogModules)
            | RetryRequest::Read(ReadKind::PreviewInstall) => {
                // Parameterised reads always use the variants below, which
                // carry the request they were started with.
                Task::none()
            }
            RetryRequest::Modules(version) => {
                self.modules.begin();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version, None)
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
                modules,
                headless,
            } => {
                self.preview_loading = true;
                self.preview_error = None;
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.dry_run_install(&version, &modules, headless)
                    }),
                    Message::PreviewLoaded,
                )
            }
        }
    }

    fn start_job(&mut self, kind: &str, spec: CommandSpec) -> bool {
        if self
            .active_job
            .as_ref()
            .map(ActiveJob::is_running)
            .unwrap_or(false)
        {
            self.notice = Some(Notice::error("Another job is already running."));
            return false;
        }

        match self.jobs.start(kind, spec) {
            Ok(started) => {
                self.notice = Some(Notice::info(format!("Started: {}", started.command)));
                self.active_job = Some(ActiveJob::new(started));
                true
            }
            Err(error) => {
                self.notice = Some(Notice::error(error_text(&error)));
                false
            }
        }
    }

    fn poll_jobs(&mut self) -> Task<Message> {
        const MAX_LOG_LINES: usize = 300;
        const MAX_LOG_CHARS: usize = 1000;

        let events = self.jobs.poll();
        let mut finished: Option<(String, JobState, Option<UnityError>)> = None;

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
                    job.logs.push_back(LogLine {
                        stream: event.stream.unwrap_or(StreamKind::Stdout),
                        text,
                    });
                    while job.logs.len() > MAX_LOG_LINES {
                        job.logs.pop_front();
                    }
                }
            }
            if let Some(progress) = event.progress {
                job.progress = progress;
            }
            if let Some(message) = event.message {
                job.message = Some(message);
            }
            if event.state != JobState::Running {
                job.state = event.state;
                job.error = event.error.clone();
                if event.state == JobState::Succeeded {
                    job.progress = 100.0;
                }
                finished = Some((job.kind.clone(), event.state, event.error));
            }
        }

        let Some((kind, state, error)) = finished else {
            return Task::none();
        };

        match state {
            JobState::Succeeded => {
                self.notice = Some(Notice::success("Job finished successfully."));
            }
            JobState::Failed => {
                let message = error
                    .map(|error| error_text(&error))
                    .unwrap_or_else(|| "The job failed.".to_string());
                self.notice = Some(Notice::error(message));
            }
            JobState::Cancelled => {
                self.notice = Some(Notice::info("Job cancelled."));
            }
            JobState::Running => {}
        }

        match kind.as_str() {
            "cli-install" | "cli-update" => {
                Task::batch(vec![self.refresh_status(), self.load_overview_data(true)])
            }
            "editor-install" | "module-install" => self.load_editors_page(true),
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
            _ => Task::none(),
        }
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

    /// Starts every Diagnostics read that is missing (or all of them on a
    /// refresh). Each section owns its `Loadable`, so one slow report never
    /// holds back the doctor checks or the environment paths.
    fn load_diagnostics_page(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if self.doctor.should_start(force) {
            self.doctor.begin();
            tasks.push(load_doctor(self.cli.clone()));
        }
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
            advisories: self.advisory_vms(),
            notice: self.notice.as_ref().map(|notice| NoticeVm {
                tone: notice_tone(notice.kind),
                text: notice.text.clone(),
            }),
            job: self.active_job.as_ref().map(|job| self.job_vm(job)),
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

    /// Advisories describe the CLI that reported them: a version change drops
    /// the in-memory list and the previous version's persisted dismissals, so
    /// a new CLI shows its advisories once more.
    fn on_cli_version_resolved(&mut self, previous_version: &str) {
        let version = self.cli_version_key();
        if version == previous_version {
            return;
        }
        self.advisories.clear();
        if version == UNKNOWN_CLI_VERSION {
            return;
        }
        // Prune lazily: the next settings write persists it, and a boot must
        // not rewrite the settings file on its own.
        self.app_settings.keep_advisories_for(&version);
    }

    /// Merges every advisory a read or job reported since the last update.
    /// `CliAdvisory::identity` collapses repeated frames into one entry.
    fn absorb_advisories(&mut self) {
        let mut incoming = self.cli.take_advisories();
        incoming.append(&mut self.jobs.take_advisories());

        for advisory in incoming {
            let id = advisory.identity();
            if self.advisories.iter().any(|entry| entry.id == id) {
                continue;
            }
            let run = self.advisory_run(&advisory);
            self.advisories.push(AdvisoryEntry { advisory, id, run });
        }
    }

    /// Maps a remediation onto a command the adapter can build. Only codes with
    /// an explicit builder get a Run button; every other named command stays
    /// visible and copyable so the app never runs an unvetted argv.
    fn advisory_run(&self, advisory: &CliAdvisory) -> Option<AdvisoryRun> {
        let remediation = advisory.remediation.as_ref()?;
        if remediation.command.trim().is_empty() {
            return None;
        }

        match advisory.code.as_deref()? {
            // #7 (CLI rollback) and #9 (plugins) add their builders here.
            "CLI_UPDATE_AVAILABLE" => Some(AdvisoryRun::CliUpdate),
            _ => None,
        }
    }

    /// Builds the argv for a remediation at click time. The CLI names
    /// `unity self-update`; the adapter's builder is that command with the
    /// standard machine flags plus `--yes`.
    fn advisory_spec(&self, run: AdvisoryRun) -> Result<CommandSpec, UnityError> {
        match run {
            AdvisoryRun::CliUpdate => self.cli.self_update_spec(),
        }
    }

    fn advisory_command(&self, id: &str) -> Option<String> {
        self.advisories
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| entry.advisory.remediation.as_ref())
            .map(|remediation| remediation.command.trim().to_string())
            .filter(|command| !command.is_empty())
    }

    /// The banners the shell draws: deduplicated, in first-seen order, minus
    /// the ones dismissed for the current CLI version.
    fn advisory_vms(&self) -> Vec<AdvisoryVm> {
        let version = self.cli_version_key();
        self.advisories
            .iter()
            .filter(|entry| !self.app_settings.is_advisory_dismissed(&version, &entry.id))
            .map(advisory_vm)
            .collect()
    }

    fn overview_vm(&self) -> OverviewVm {
        OverviewVm {
            cli: self.cli_card_vm(),
            update: self.load_state(ReadKind::Update, &self.update, |update| UpdateVm {
                available: update.update_available,
                current_version: update.current_version.clone(),
                latest_version: update.latest_version.clone(),
            }),
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
                };
            }
            return AuthVm::SignedOut {
                expired: auth.session_state.as_deref() == Some("SESSION_EXPIRED"),
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
        let installed = self.installed_versions();

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
                        stream: release.stream.clone(),
                        stream_tone: stream_tone(&release.stream),
                        installed: release.installed
                            || installed.contains(release.version.as_str()),
                    })
                    .collect()
            }),
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

        InstallEditorVm {
            version_input: self.install_version.clone(),
            modules,
            available_count,
            headless: self.install_headless && unity_core::runner::headless_editor_supported(),
            headless_available: unity_core::runner::headless_editor_supported(),
            accept_eula: self.accept_eula,
            can_install: self.selected_modules.is_empty() || self.accept_eula,
            preview,
            preview_note: self
                .preview_raw
                .as_ref()
                .map(|raw| format!("Dry-run returned: {raw}")),
            preview_error: self.preview_error.clone(),
            preview_loading: self.preview_loading,
        }
    }

    fn projects_vm(&self) -> ProjectsVm {
        let filtered = self.filtered_projects();
        let total_count = self.projects.data.as_ref().map(Vec::len).unwrap_or(0);
        let visible_count = filtered.len();
        let installed = self.installed_versions();
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

    fn changelog_vm(&self) -> ChangelogVm {
        ChangelogVm {
            loaded: self.changelog_loaded,
            versions: self.changelog_versions(),
            selected_target: self.changelog_target.clone(),
            show_raw: self.show_raw_changelog,
            state: self.load_state(ReadKind::Changelog, &self.changelog, |view| {
                ChangelogDataVm {
                    version: view.version.clone(),
                    source: view.source.clone(),
                    source_detail: view.source_detail.clone(),
                    entry_count: view.entry_count,
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
                            entries: section.entries.clone(),
                        })
                        .collect(),
                }
            }),
            raw: self.load_state(ReadKind::RawChangelog, &self.raw_changelog, |markdown| {
                markdown.clone()
            }),
        }
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

                Some(ModalVm::EditorModules(EditorModulesVm {
                    editor_version: editor.version.clone(),
                    architecture: editor.architecture.clone(),
                    installed_modules: editor.module_names.clone(),
                    modules,
                    selected_count: self.dialog_selected.len(),
                    accept_eula: self.dialog_eula,
                    can_install: !self.dialog_selected.is_empty() && self.dialog_eula,
                }))
            }
            Modal::UpgradeProject(project) => {
                let target_options = self
                    .editors
                    .data
                    .as_ref()
                    .map(|editors| {
                        editors
                            .iter()
                            .filter(|editor| editor.version != project.version)
                            .map(|editor| editor.version.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                Some(ModalVm::UpgradeProject(UpgradeProjectVm {
                    project_title: project.title.clone(),
                    project_path_short: short_path(&project.path, 60).into_owned(),
                    current_version: project.version.clone(),
                    target_options,
                    selected_target: self.upgrade_target.clone(),
                    confirmed: self.upgrade_confirmed,
                    can_upgrade: self.upgrade_confirmed && self.upgrade_target.is_some(),
                }))
            }
        }
    }

    fn first_other_editor_version(&self, current: &str) -> Option<String> {
        self.editors
            .data
            .as_ref()?
            .iter()
            .map(|editor| editor.version.clone())
            .find(|version| version != current)
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

    pub fn installed_versions(&self) -> HashSet<&str> {
        self.editors
            .data
            .as_ref()
            .map(|editors| {
                editors
                    .iter()
                    .map(|editor| editor.version.as_str())
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
            logs: job
                .logs
                .iter()
                .map(|line| JobLogVm {
                    is_stderr: line.stream == StreamKind::Stderr,
                    text: line.text.clone(),
                })
                .collect(),
        }
    }
}

/// No job kind in this app is retryable today: every job installs, upgrades,
/// signs in, or updates the CLI itself. Replaying any of those after a partial
/// run is exactly what the ticket forbids. A future read-only job opts in here
/// with an explicit kind; it must never default to retryable.
fn job_allows_retry(_kind: &str) -> bool {
    false
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

fn notice_tone(kind: NoticeKind) -> Tone {
    match kind {
        NoticeKind::Info => Tone::Info,
        NoticeKind::Success => Tone::Success,
        NoticeKind::Error => Tone::Danger,
    }
}

const UNKNOWN_CLI_VERSION: &str = "unknown";

/// Branches on `code`, keeps the CLI's message verbatim, and only derives
/// fields for the codes the CLI documents. Unknown codes and warnings keep the
/// CLI's own text and get a neutral title instead of invented semantics.
fn advisory_vm(entry: &AdvisoryEntry) -> AdvisoryVm {
    let advisory = &entry.advisory;
    let (tone, title) = match advisory.code.as_deref() {
        Some("CLI_UPDATE_AVAILABLE") => (Tone::Accent, "CLI update available"),
        Some("PLUGIN_UPDATE_AVAILABLE") => (Tone::Accent, "Plugin update available"),
        Some("FEX_EMU_EXPERIMENTAL") => (Tone::Warning, "Experimental environment"),
        Some("AGENT_PLUGIN_AVAILABLE") => (Tone::Info, "Agent integration available"),
        _ => match advisory.origin {
            AdvisoryOrigin::Notification => (Tone::Info, "Advisory"),
            AdvisoryOrigin::Warning => (Tone::Warning, "CLI warning"),
        },
    };

    AdvisoryVm {
        id: entry.id.clone(),
        tone,
        title: title.to_string(),
        message: advisory.message.clone(),
        detail: advisory_detail(advisory),
        command: advisory
            .remediation
            .as_ref()
            .map(|remediation| remediation.command.trim().to_string())
            .filter(|command| !command.is_empty()),
        run_label: entry.run.map(|run| run.label().to_string()),
        requires_approval: advisory
            .remediation
            .as_ref()
            .is_some_and(|remediation| remediation.requires_user_approval),
        dismissible: true,
    }
}

/// Per-code `data` summary. Codes without a documented shape render no
/// detail rather than a speculative one.
fn advisory_detail(advisory: &CliAdvisory) -> Option<String> {
    let data = advisory.data.as_ref()?;
    let field = |key: &str| {
        data.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };

    match advisory.code.as_deref()? {
        "CLI_UPDATE_AVAILABLE" => match (field("current"), field("latest")) {
            (Some(current), Some(latest)) => Some(format!("{current} → {latest}")),
            _ => None,
        },
        "PLUGIN_UPDATE_AVAILABLE" => {
            let versions = match (field("current"), field("latest")) {
                (Some(current), Some(latest)) => format!("{current} → {latest}"),
                _ => return None,
            };
            Some(match field("plugin") {
                Some(plugin) => format!("{plugin} · {versions}"),
                None => versions,
            })
        }
        _ => None,
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
    redact_proxy(text)
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
            let redacted = redact_proxy(text);
            if redacted != *text {
                *text = redacted;
            }
        }
        _ => {}
    }
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

fn editor_row_vm(editor: &Editor) -> EditorRowVm {
    EditorRowVm {
        version: editor.version.clone(),
        architecture: editor.architecture.clone(),
        is_default: editor.default,
        modules: editor.module_names.clone(),
    }
}

fn project_row_vm(project: &Project, installed: &HashSet<&str>) -> ProjectRowVm {
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
    if !installed.is_empty() && !installed.contains(project.version.as_str()) {
        badges.push(BadgeVm {
            label: "Editor not installed".to_string(),
            tone: Tone::Warning,
        });
    }
    if project.is_favorite {
        badges.push(BadgeVm {
            label: "favorite".to_string(),
            tone: Tone::Accent,
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
        pipeline: project.render_pipeline.as_ref().map(|pipeline| BadgeVm {
            label: pipeline.clone(),
            tone: Tone::Highlight,
        }),
        build_target: project
            .build_target
            .clone()
            .unwrap_or_else(|| "—".to_string()),
        short_path: short_path(&project.path, 42).into_owned(),
        modified: format_epoch_millis(project.last_modified),
        badges,
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

fn load_doctor(cli: UnityCli) -> Task<Message> {
    Task::perform(run_blocking(move || cli.doctor()), Message::DoctorLoaded)
}

fn load_environment(cli: UnityCli) -> Task<Message> {
    Task::perform(
        run_blocking(move || cli.environment()),
        Message::EnvironmentLoaded,
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
    use unity_core::models::CliVersion;

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
                version: "1.0.0-beta.16".to_string(),
                channel: "beta".to_string(),
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
            } if version == "1.0.0-beta.16"
        ));
        match &vm.page {
            PageVm::Overview(overview) => {
                match &overview.cli {
                    CliCardVm::Ready { details, .. } => {
                        assert_eq!(details.channel, "beta");
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

    #[test]
    fn projects_force_load_starts_a_running_editor_refresh() {
        let (mut app, _) = App::boot();

        let _task = app.load_projects_page(true);

        assert!(app.running.loading);
    }

    // ------------------------------------------------------------------
    // CLI advisories

    fn connected_app(version: &str) -> App {
        let (mut app, _) = App::boot();
        // Tests must not inherit the developer's persisted dismissals.
        app.app_settings.dismissed_advisories.clear();
        app.cli_status = Loadable::default();
        app.cli_status.data = Some(CliStatus {
            installed: true,
            version: Some(CliVersion {
                version: version.to_string(),
                ..CliVersion::default()
            }),
            ..CliStatus::default()
        });
        app.advisories.clear();
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
    fn duplicate_advisories_from_a_refresh_collapse_into_one_banner() {
        let mut app = connected_app("1.0.0-beta.17");

        absorb(&mut app, update_advisory());
        absorb(&mut app, update_advisory());
        absorb(&mut app, update_advisory());

        let advisories = app.view_model().shell.advisories;
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].message, "A new Unity CLI is available.");
    }

    #[test]
    fn update_advisory_shows_current_to_latest_and_a_runnable_command() {
        let mut app = connected_app("1.0.0-beta.16");
        let binary = temporary_cli();
        app.cli.set_override_path(Some(binary.clone()));

        absorb(&mut app, update_advisory());

        let vm = app.view_model();
        assert_eq!(vm.shell.advisories.len(), 1);
        let advisory = &vm.shell.advisories[0];
        assert_eq!(advisory.title, "CLI update available");
        assert_eq!(
            advisory.detail.as_deref(),
            Some("1.0.0-beta.16 → 1.0.0-beta.17")
        );
        assert_eq!(advisory.command.as_deref(), Some("unity self-update"));
        assert!(advisory.requires_approval);
        assert_eq!(advisory.run_label.as_deref(), Some("Update CLI"));
        assert!(advisory.dismissible);

        // The Run button executes a builder, never the raw CLI string.
        let run = app.advisories[0].run.expect("a builder exists");
        assert_eq!(run, AdvisoryRun::CliUpdate);
        let spec = app.advisory_spec(run).expect("the builder resolves");
        assert!(spec.args.iter().any(|arg| arg == "self-update"));
        assert!(spec.args.iter().any(|arg| arg == "--yes"));
        std::fs::remove_file(binary).ok();
    }

    #[test]
    fn update_advisory_run_builds_the_command_at_click_time() {
        let mut app = connected_app("1.0.0-beta.16");
        // A path that does not exist makes resolution fail deterministically,
        // even on a machine with a real `unity` on PATH.
        app.cli
            .set_override_path(Some(PathBuf::from("/nonexistent/unity-rust-hub-test")));

        absorb(&mut app, update_advisory());

        let advisory = &app.view_model().shell.advisories[0];
        assert_eq!(advisory.command.as_deref(), Some("unity self-update"));
        assert_eq!(advisory.run_label.as_deref(), Some("Update CLI"));
        assert!(app.advisory_command(&advisory.id).is_some());

        // The command is only built when the user clicks, so a missing CLI is
        // reported then instead of silently removing the button.
        let run = app.advisories[0].run.expect("a builder exists");
        assert!(app.advisory_spec(run).is_err());
    }

    #[test]
    fn dismissal_hides_the_banner_for_one_cli_version_and_survives_a_refresh() {
        let mut app = connected_app("1.0.0-beta.16");
        absorb(&mut app, update_advisory());
        let id = app.advisories[0].id.clone();
        let version = app.cli_version_key();

        app.app_settings.dismiss_advisory(&version, &id);
        assert!(app.view_model().shell.advisories.is_empty());

        // A polled refresh reports the same advisory again.
        absorb(&mut app, update_advisory());
        assert!(app.view_model().shell.advisories.is_empty());

        // A CLI version change drops the store and the dismissal scope, so
        // the new CLI shows its advisory once.
        let previous = app.cli_version_key();
        app.cli_status.data = Some(CliStatus {
            installed: true,
            version: Some(CliVersion {
                version: "1.0.0-beta.17".to_string(),
                ..CliVersion::default()
            }),
            ..CliStatus::default()
        });
        app.on_cli_version_resolved(&previous);
        assert!(app.advisories.is_empty());
        assert!(app.app_settings.dismissed_advisories.is_empty());

        absorb(&mut app, update_advisory());
        assert_eq!(app.view_model().shell.advisories.len(), 1);
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

        let advisories = app.view_model().shell.advisories;
        assert_eq!(advisories.len(), 2);
        assert_eq!(advisories[0].title, "Advisory");
        assert_eq!(advisories[0].message, "Something new happened.");
        assert!(advisories[0].command.is_none());
        assert_eq!(advisories[1].title, "CLI warning");
        assert_eq!(advisories[1].message, "The --skip flag is deprecated.");
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

        let advisories = app.view_model().shell.advisories;
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].title, "CLI warning");
        assert_eq!(
            advisories[0].message,
            "This plugin is a beta release. Beta plugins can break projects."
        );
        assert!(advisories[0].command.is_none());
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
            message: Some("Working…".to_string()),
            logs: VecDeque::new(),
            error: Some(error),
        }
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
            RetryRequest::Modules("6000.1.1f1".to_string()),
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
            Some(&RetryRequest::Modules("6000.1.1f1".to_string()))
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
    // Diagnostics

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
    fn opening_diagnostics_starts_every_section_without_waiting() {
        let (mut app, _) = App::boot();
        let _task = app.update(Message::Navigate(Page::Diagnostics));

        assert!(app.doctor.loading);
        assert!(app.environment.loading);
        assert!(app.diagnose_proxy.loading);
        assert!(app.diagnose_accelerator.loading);
        assert!(app.diagnose_update.loading);
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

        let advisory = &app.view_model().shell.advisories[0];
        assert_eq!(advisory.title, "Plugin update available");
        assert_eq!(
            advisory.detail.as_deref(),
            Some("com.unity.ai · 1.0.0 → 1.1.0")
        );
        assert_eq!(
            advisory.command.as_deref(),
            Some("unity plugin upgrade com.unity.ai")
        );
        assert_eq!(advisory.run_label, None);
    }
}
