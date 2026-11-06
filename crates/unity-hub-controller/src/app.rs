use std::collections::{BTreeSet, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use iced::{Subscription, Task};
use serde_json::Value;
use unity_core::bootstrap;
use unity_core::changelog::{ChangelogRequest, ChangelogView, CliChangelogProvider};
use unity_core::models::{
    AuthStatus, CliStatus, CliUpdateInfo, Editor, InstallPreview, LicenseStatus, Module, Project,
    Release, RunningEditor, RunningEditors,
};
use unity_core::{AdvisoryOrigin, CliAdvisory, CommandSpec, UnityCli, UnityError};
use unity_hub_model::{
    AccountCardVm, AdvisoryVm, AuthVm, BadgeVm, BreakingChangeVm, ChangelogDataVm,
    ChangelogSectionVm, ChangelogVm, CliCardVm, CliConnectionVm, CliDetailsVm, EditorModulesVm,
    EditorRowVm, EditorsVm, InstallCliVm, InstallEditorVm, InstallPreviewVm, JobLogVm, JobPhase,
    JobVm, LicenseRowVm, LicenseVm, LoadState, ModalVm, ModuleOptionVm, NoticeVm, OverviewVm, Page,
    PageVm, ProjectRowVm, ProjectsVm, ReleaseRowVm, RunningEditorVm, SectionKind, SettingsVm,
    ShellVm, TextScaleOptionVm, Tone, UiAction, UiError, UpdateVm, UpgradeProjectVm, ViewModel,
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
    PollJobs,
    ProjectOpened(Result<(), UnityError>),
}

impl From<UiAction> for Message {
    fn from(action: UiAction) -> Self {
        match action {
            UiAction::Navigate(page) => Self::Navigate(page),
            UiAction::Refresh => Self::Refresh,
            UiAction::DismissNotice => Self::DismissNotice,
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

        let app = Self {
            cli: cli.clone(),
            jobs: JobManager::with_sink(cli.advisory_sink()),
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
            boot_background_started: false,
        };

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
                        self.cli_status.succeeded(status);
                        installed
                    }
                    Err(error) => {
                        self.cli_status.failed(error);
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
                    Ok(info) => self.update.succeeded(info),
                    Err(error) => self.update.failed(error),
                }
                Task::none()
            }
            Message::AuthLoaded(result) => {
                match result {
                    Ok(status) => self.auth.succeeded(status),
                    Err(error) => self.auth.failed(error),
                }
                Task::none()
            }
            Message::LicenseLoaded(result) => {
                match result {
                    Ok(status) => self.license.succeeded(status),
                    Err(error) => self.license.failed(error),
                }
                Task::none()
            }
            Message::RunningLoaded(result) => {
                match result {
                    Ok(status) => self.running.succeeded(status),
                    Err(error) => self.running.failed(error),
                }
                Task::none()
            }
            Message::EditorsLoaded(result) => {
                match result {
                    Ok(editors) => {
                        self.editors.succeeded(editors);
                        self.ensure_upgrade_target();
                    }
                    Err(error) => self.editors.failed(error),
                }
                Task::none()
            }
            Message::ReleasesLoaded(result) => {
                match result {
                    Ok(releases) => self.releases.succeeded(releases),
                    Err(error) => self.releases.failed(error),
                }
                Task::none()
            }
            Message::ProjectsLoaded(result) => {
                match result {
                    Ok(projects) => {
                        self.project_search_index = project_search_keys(&projects);
                        self.projects.succeeded(projects);
                    }
                    Err(error) => self.projects.failed(error),
                }
                Task::none()
            }
            Message::ModulesLoaded(result) => {
                match result {
                    Ok(modules) => self.modules.succeeded(modules),
                    Err(error) => self.modules.failed(error),
                }
                Task::none()
            }
            Message::DialogModulesLoaded(result) => {
                match result {
                    Ok(modules) => self.dialog_modules.succeeded(modules),
                    Err(error) => self.dialog_modules.failed(error),
                }
                Task::none()
            }
            Message::PreviewLoaded(result) => {
                self.preview_loading = false;
                match result {
                    Ok(value) => match serde_json::from_value::<InstallPreview>(value.clone()) {
                        Ok(preview) => {
                            self.preview = Some(preview);
                            self.preview_raw = None;
                        }
                        Err(_) => {
                            self.preview = None;
                            self.preview_raw = Some(value);
                        }
                    },
                    Err(error) => {
                        self.preview = None;
                        self.preview_raw = None;
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::ChangelogLoaded(result) => {
                match result {
                    Ok(view) => self.changelog.succeeded(view),
                    Err(error) => self.changelog.failed(error),
                }
                Task::none()
            }
            Message::RawChangelogLoaded(result) => {
                match result {
                    Ok(markdown) => self.raw_changelog.succeeded(markdown),
                    Err(error) => self.raw_changelog.failed(error),
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
                        self.notice = Some(Notice::info("Cancelling the job…"));
                    }
                }
                Task::none()
            }
            Message::DismissNotice => {
                self.notice = None;
                Task::none()
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
                let version = self.install_version.trim().to_string();
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
                let version = self.install_version.trim().to_string();
                let modules: Vec<String> = self.selected_modules.iter().cloned().collect();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        let headless = self.install_headless;
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
    fn load_overview_data(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if (force || self.update.data.is_none()) && !self.update.loading {
            self.update.begin();
            tasks.push(load_update(self.cli.clone()));
        }
        if (force || self.auth.data.is_none()) && !self.auth.loading {
            self.auth.begin();
            tasks.push(load_auth(self.cli.clone()));
        }
        if (force || self.license.data.is_none()) && !self.license.loading {
            self.license.begin();
            tasks.push(load_license(self.cli.clone()));
        }
        if (force || self.running.data.is_none()) && !self.running.loading {
            self.running.begin();
            tasks.push(load_running(self.cli.clone()));
        }
        // The summary row counts installed Editors and registered projects.
        if (force || self.editors.data.is_none()) && !self.editors.loading {
            self.editors.begin();
            tasks.push(load_editors(self.cli.clone()));
        }
        if (force || self.projects.data.is_none()) && !self.projects.loading {
            self.projects.begin();
            tasks.push(load_projects(self.cli.clone()));
        }

        Task::batch(tasks)
    }

    fn load_editors_page(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if (force || self.editors.data.is_none()) && !self.editors.loading {
            self.editors.begin();
            tasks.push(load_editors(self.cli.clone()));
        }
        if (force || self.releases.data.is_none()) && !self.releases.loading {
            self.releases.begin();
            tasks.push(load_releases(self.cli.clone()));
        }

        Task::batch(tasks)
    }

    fn load_projects_page(&mut self, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();

        if (force || self.projects.data.is_none()) && !self.projects.loading {
            self.projects.begin();
            tasks.push(load_projects(self.cli.clone()));
        }
        if (force || self.editors.data.is_none()) && !self.editors.loading {
            self.editors.begin();
            tasks.push(load_editors(self.cli.clone()));
        }
        if (force || self.running.data.is_none()) && !self.running.loading {
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
            job: self.active_job.as_ref().map(job_vm),
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
            update: load_state(&self.update, |update| UpdateVm {
                available: update.update_available,
                current_version: update.current_version.clone(),
                latest_version: update.latest_version.clone(),
            }),
            account: AccountCardVm {
                auth: self.auth_vm(),
                license: self.license_vm(),
            },
            running: load_state(&self.running, |running| {
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
                error: self.cli_status.error.as_ref().map(ui_error),
            };
        }

        match &self.cli_status.error {
            Some(error) => CliCardVm::Failed(ui_error(error)),
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
            Some(error) => AuthVm::Failed(ui_error(error)),
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
            Some(error) => LicenseVm::Failed(ui_error(error)),
            None => LicenseVm::Idle,
        }
    }

    fn editors_vm(&self) -> EditorsVm {
        let installed = self.installed_versions();

        EditorsVm {
            installed: load_state(&self.editors, |editors| {
                editors.iter().map(editor_row_vm).collect()
            }),
            install: self.install_editor_vm(),
            releases: load_state(&self.releases, |releases| {
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
        let modules = load_state(&self.modules, |modules| {
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
                error: ui_error(error),
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
            state: load_state(&self.changelog, |view| ChangelogDataVm {
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
            }),
            raw: load_state(&self.raw_changelog, |markdown| markdown.clone()),
        }
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
                let modules = load_state(&self.dialog_modules, |modules| {
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
}

fn ui_error(error: &UnityError) -> UiError {
    UiError {
        message: error.message.clone(),
        detail: error
            .detail
            .clone()
            .filter(|detail| !detail.trim().is_empty()),
    }
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
