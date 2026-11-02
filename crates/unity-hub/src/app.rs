use std::collections::{BTreeSet, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::text::IntoFragment;
use iced::widget::{button, checkbox, column, container, pick_list, row, scrollable, space};
use iced::{Center, Element, Fill, Subscription, Task, Theme};
use serde_json::Value;
use unity_core::bootstrap;
use unity_core::changelog::{ChangelogRequest, ChangelogView, CliChangelogProvider};
use unity_core::models::{
    AuthStatus, CliStatus, CliUpdateInfo, Editor, InstallPreview, LicenseStatus, Module, Project,
    Release, RunningEditors,
};
use unity_core::{CommandSpec, UnityCli, UnityError};

use crate::jobs::{JobManager, JobStarted, JobState, StreamKind};
use crate::settings::{self, AppSettings, TextSize};
use crate::theme;
use crate::ui::{self, text};
use crate::util::{error_text, format_bytes, run_blocking, short_path};
use crate::views;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Editors,
    Projects,
    Changelog,
    Settings,
}

impl Page {
    pub const ALL: [Page; 5] = [
        Page::Overview,
        Page::Editors,
        Page::Projects,
        Page::Changelog,
        Page::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Editors => "Editors",
            Page::Projects => "Projects",
            Page::Changelog => "Changelog",
            Page::Settings => "Settings",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Editors => "Editors & modules",
            Page::Projects => "Projects",
            Page::Changelog => "Changelog & breaking changes",
            Page::Settings => "Settings",
        }
    }
}

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

    pub fn tone(&self) -> theme::Tone {
        match self.kind {
            NoticeKind::Info => theme::Tone::Blue,
            NoticeKind::Success => theme::Tone::Green,
            NoticeKind::Error => theme::Tone::Red,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub stream: StreamKind,
    pub text: String,
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
    pub boot_background_started: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    Refresh,
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
    DismissJob,
    CancelJob,
    DismissNotice,
    SignIn,
    TextSizeChanged(TextSize),
    OpenInstallCli,
    CliChannelChanged(String),
    CliConsentToggled(bool),
    InstallCli,
    UpdateCli,
    InstallVersionChanged(String),
    LoadModules,
    ToggleModule(usize, bool),
    SelectAllModules(bool),
    AcceptEulaToggled(bool),
    PreviewInstall,
    StartInstallEditor,
    UseRelease(usize),
    OpenModules(usize),
    CloseModal,
    ToggleDialogModule(usize, bool),
    DialogEulaToggled(bool),
    StartInstallModules,
    ProjectSearchChanged(String),
    OpenProject(usize),
    ProjectOpened(Result<(), UnityError>),
    OpenUpgrade(usize),
    UpgradeTargetChanged(String),
    UpgradeConfirmed(bool),
    StartUpgrade,
    ChangelogTargetChanged(String),
    LoadChangelog,
    ToggleRawChangelog,
    CliPathChanged(String),
    SaveCliPath,
    ClearCliPath,
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
            jobs: JobManager::new(),
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
            boot_background_started: false,
        };

        // The status read is the only critical boot work. The rest of the
        // Overview is loaded after it succeeds, one CLI process at a time.
        (app, load_status(cli))
    }

    pub fn text_scale(&self) -> f32 {
        self.app_settings.text_size().scale()
    }

    pub fn font_size(&self, base: f32) -> f32 {
        base * self.text_scale()
    }

    pub fn theme(&self) -> Theme {
        theme::app_theme()
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
        match message {
            Message::Navigate(page) => {
                self.page = page;
                self.ensure_page(page)
            }
            Message::Refresh => self.refresh_page(),
            Message::StatusLoaded(result) => {
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
                        move || cli.modules(&version)
                    }),
                    Message::ModulesLoaded,
                )
            }
            Message::ToggleModule(index, value) => {
                let Some(id) = self
                    .modules
                    .data
                    .as_ref()
                    .and_then(|modules| modules.get(index))
                    .map(|module| module.id.clone())
                else {
                    return Task::none();
                };
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
                        move || cli.dry_run_install(&version, &modules)
                    }),
                    Message::PreviewLoaded,
                )
            }
            Message::StartInstallEditor => {
                let modules: Vec<String> = self.selected_modules.iter().cloned().collect();
                match self.cli.install_editor_spec(
                    &self.install_version,
                    &modules,
                    self.accept_eula,
                ) {
                    Ok(spec) => {
                        if self.start_job("editor-install", spec) {
                            self.modules.data = None;
                            self.selected_modules.clear();
                            self.accept_eula = false;
                        }
                    }
                    Err(error) => {
                        self.notice = Some(Notice::error(error_text(&error)));
                    }
                }
                Task::none()
            }
            Message::UseRelease(index) => {
                let Some(version) = self
                    .releases
                    .data
                    .as_ref()
                    .and_then(|releases| releases.get(index))
                    .map(|release| release.version.clone())
                else {
                    return Task::none();
                };
                self.install_version = version.clone();
                self.modules.begin();
                self.selected_modules.clear();
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version)
                    }),
                    Message::ModulesLoaded,
                )
            }
            Message::OpenModules(index) => {
                let Some(editor) = self
                    .editors
                    .data
                    .as_ref()
                    .and_then(|editors| editors.get(index))
                    .cloned()
                else {
                    return Task::none();
                };
                self.dialog_modules.begin();
                self.dialog_selected.clear();
                self.dialog_eula = false;
                let version = editor.version.clone();
                self.modal = Modal::EditorModules(editor);
                Task::perform(
                    run_blocking({
                        let cli = self.cli.clone();
                        move || cli.modules(&version)
                    }),
                    Message::DialogModulesLoaded,
                )
            }
            Message::CloseModal => {
                self.modal = Modal::None;
                Task::none()
            }
            Message::ToggleDialogModule(index, value) => {
                let Some(id) = self
                    .dialog_modules
                    .data
                    .as_ref()
                    .and_then(|modules| modules.get(index))
                    .map(|module| module.id.clone())
                else {
                    return Task::none();
                };
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
                match self
                    .cli
                    .install_modules_spec(&editor.version, &modules, self.dialog_eula)
                {
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
            Message::OpenProject(index) => {
                let Some(project) = self
                    .projects
                    .data
                    .as_ref()
                    .and_then(|projects| projects.get(index))
                else {
                    return Task::none();
                };
                let title = project.title.clone();
                let path = project.path.clone();
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
            Message::OpenUpgrade(index) => {
                let Some(project) = self
                    .projects
                    .data
                    .as_ref()
                    .and_then(|projects| projects.get(index))
                    .cloned()
                else {
                    return Task::none();
                };
                self.upgrade_target = self.first_other_editor_version(&project.version);
                self.upgrade_confirmed = false;
                self.modal = Modal::UpgradeProject(project);
                Task::none()
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
                let Some(target) = self.upgrade_target.clone() else {
                    return Task::none();
                };
                if !self.upgrade_confirmed {
                    return Task::none();
                }
                match self.cli.upgrade_project_spec(&project.path, &target) {
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
                    text_scale_percent: self.app_settings.text_scale_percent,
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

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        ui::set_scale(self.text_scale());

        // A full-screen modal screen instead of `stack!`: layering a scrollable
        // base under an opaque card miscomposites with iced's tiny-skia backend
        // (the base is drawn over the dialog), so modals get their own view.
        match self.modal_view() {
            Some(modal) => modal,
            None => self.shell(),
        }
    }

    fn shell<'a>(&'a self) -> Element<'a, Message> {
        let mut content = column![].spacing(16).width(Fill).padding(24);

        if let Some(notice) = &self.notice {
            content = content.push(self.notice_view(notice));
        }

        content = content.push(self.page_view());

        if let Some(job) = &self.active_job {
            content = content.push(self.job_view(job));
        }

        let main = column![self.topbar(), scrollable(content).height(Fill).width(Fill)]
            .width(Fill)
            .height(Fill);

        row![self.sidebar(), main].height(Fill).width(Fill).into()
    }

    fn sidebar<'a>(&'a self) -> Element<'a, Message> {
        let brand = row![
            container(text("U").size(18).style(theme::accent_text))
                .width(38)
                .height(38)
                .center(38)
                .style(theme::badge(theme::Tone::Accent)),
            column![
                text("Unity Rust Hub").size(14),
                text("Native · iced").size(10).style(theme::faint_text),
            ]
            .spacing(1),
        ]
        .spacing(10)
        .align_y(Center);

        let nav = column(
            Page::ALL
                .iter()
                .map(|page| {
                    button(text(page.label()).size(13))
                        .width(Fill)
                        .padding([10.0, 12.0])
                        .style(theme::nav_button(self.page == *page))
                        .on_press(Message::Navigate(*page))
                        .into()
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(4);

        let cli_version = self
            .cli_status
            .data
            .as_ref()
            .and_then(|status| status.version.as_ref())
            .map(|version| version.version.as_str())
            .unwrap_or("not detected");
        let installed = self
            .cli_status
            .data
            .as_ref()
            .map(|status| status.installed)
            .unwrap_or(false);
        let loading = self.cli_status.loading && self.cli_status.data.is_none();

        let footer = container(
            column![
                row![
                    container(text("●").size(9).style(if installed {
                        theme::success_text
                    } else if loading {
                        theme::muted_text
                    } else {
                        theme::danger_text
                    })),
                    text(if loading {
                        "Checking CLI…"
                    } else if installed {
                        "CLI connected"
                    } else {
                        "CLI missing"
                    })
                    .size(11),
                ]
                .spacing(6)
                .align_y(Center),
                text(cli_version).size(10).style(theme::faint_text),
            ]
            .spacing(3),
        )
        .padding(10)
        .width(Fill)
        .style(theme::card);

        container(
            column![
                brand,
                nav,
                space().height(Fill),
                footer,
                text("All mutations go through the official Unity CLI.")
                    .size(9)
                    .style(theme::faint_text),
            ]
            .spacing(18)
            .width(Fill)
            .height(Fill),
        )
        .width(232)
        .height(Fill)
        .padding(16)
        .style(theme::sidebar)
        .into()
    }

    fn topbar<'a>(&'a self) -> Element<'a, Message> {
        let version = self
            .cli_status
            .data
            .as_ref()
            .and_then(|status| status.version.as_ref())
            .map(|version| version.version.as_str());

        let badge = if self.cli_status.loading && version.is_none() {
            container(text("checking CLI…").size(10).style(theme::muted_text))
                .padding([3.0, 8.0])
                .style(theme::badge(theme::Tone::Neutral))
        } else {
            match version {
                Some(version) => container(text(version).size(10).style(theme::success_text))
                    .padding([3.0, 8.0])
                    .style(theme::badge(theme::Tone::Green)),
                None => container(text("CLI not found").size(10).style(theme::danger_text))
                    .padding([3.0, 8.0])
                    .style(theme::badge(theme::Tone::Red)),
            }
        };

        container(
            row![
                text(self.page.title()).size(17),
                space().width(Fill),
                badge,
                button(text("Refresh").size(11))
                    .on_press(Message::Refresh)
                    .padding([6.0, 10.0])
                    .style(button::text),
            ]
            .spacing(10)
            .align_y(Center),
        )
        .padding([12.0, 22.0])
        .width(Fill)
        .style(theme::topbar)
        .into()
    }

    fn page_view<'a>(&'a self) -> Element<'a, Message> {
        match self.page {
            Page::Overview => views::overview::view(self),
            Page::Editors => views::editors::view(self),
            Page::Projects => views::projects::view(self),
            Page::Changelog => views::changelog::view(self),
            Page::Settings => views::settings::view(self),
        }
    }

    fn notice_view<'a>(&'a self, notice: &'a Notice) -> Element<'a, Message> {
        container(
            row![
                text(&notice.text).size(12).width(Fill),
                button(text("Dismiss").size(10))
                    .on_press(Message::DismissNotice)
                    .padding([4.0, 8.0])
                    .style(button::text),
            ]
            .spacing(10)
            .align_y(Center),
        )
        .padding([10.0, 12.0])
        .width(Fill)
        .style(theme::notice_card(notice.tone()))
        .into()
    }

    fn job_view<'a>(&'a self, job: &'a ActiveJob) -> Element<'a, Message> {
        let state_badge = match job.state {
            JobState::Succeeded => theme::Tone::Green,
            JobState::Failed => theme::Tone::Red,
            JobState::Cancelled => theme::Tone::Amber,
            JobState::Running => theme::Tone::Blue,
        };

        let header = row![
            text(&job.kind).size(13).width(Fill),
            container(text(job.state.label()).size(10))
                .padding([3.0, 8.0])
                .style(theme::badge(state_badge)),
            if job.is_running() {
                Element::from(
                    button(text("Cancel").size(10))
                        .on_press(Message::CancelJob)
                        .padding([4.0, 8.0])
                        .style(button::danger),
                )
            } else {
                Element::from(
                    button(text("Dismiss").size(10))
                        .on_press(Message::DismissJob)
                        .padding([4.0, 8.0])
                        .style(button::text),
                )
            },
        ]
        .spacing(8)
        .align_y(Center);

        let mut body = column![header].spacing(8);

        if job.is_running() {
            body = body.push(
                iced::widget::progress_bar(0.0..=100.0, job.progress)
                    .length(Fill)
                    .girth(6.0),
            );
        }

        if let Some(message) = &job.message {
            body = body.push(text(message).size(11).style(theme::muted_text));
        }

        if let Some(error) = &job.error {
            body = body.push(text(error_text(error)).size(11).style(theme::danger_text));
        }

        body = body.push(
            text(&job.command)
                .size(9)
                .font(iced::Font::MONOSPACE)
                .style(theme::faint_text),
        );

        if !job.logs.is_empty() {
            let mut log_text = String::new();
            for line in &job.logs {
                if line.stream == StreamKind::Stderr {
                    log_text.push_str("! ");
                }
                log_text.push_str(&line.text);
                log_text.push('\n');
            }

            body = body.push(
                container(
                    scrollable(
                        text(log_text)
                            .size(9)
                            .font(iced::Font::MONOSPACE)
                            .style(theme::muted_text),
                    )
                    .height(150),
                )
                .padding(8)
                .width(Fill)
                .style(theme::card),
            );
        }

        container(body)
            .padding(14)
            .width(Fill)
            .style(theme::job_card)
            .into()
    }

    fn modal_view<'a>(&'a self) -> Option<Element<'a, Message>> {
        let card = match &self.modal {
            Modal::None => return None,
            Modal::InstallCli => self.install_cli_modal(),
            Modal::EditorModules(editor) => self.editor_modules_modal(editor),
            Modal::UpgradeProject(project) => self.upgrade_modal(project),
        };

        Some(
            container(iced::widget::center(card))
                .width(Fill)
                .height(Fill)
                .style(theme::modal_screen)
                .into(),
        )
    }

    fn install_cli_modal<'a>(&'a self) -> Element<'a, Message> {
        let channels = vec![
            "stable".to_string(),
            "beta".to_string(),
            "alpha".to_string(),
        ];
        let command = if cfg!(target_os = "windows") {
            "irm https://unity.com/install.ps1 | iex"
        } else {
            "curl -fsSL https://unity.com/install.sh | bash"
        };

        let content = column![
            text("Install the Unity CLI").size(20),
            text("Unity's official installer runs locally. This app does not bundle the CLI.")
                .size(11)
                .style(theme::muted_text),
            text("Release channel").size(10).style(theme::faint_text),
            pick_list(channels, Some(self.cli_channel.clone()), Message::CliChannelChanged)
                .text_size(self.font_size(12.0))
                .width(Fill),
            container(
                column![
                    text("Official installer command").size(10).style(theme::faint_text),
                    text(command)
                        .size(11)
                        .font(iced::Font::MONOSPACE)
                        .style(theme::accent_text),
                    text("UNITY_CLI_SKIP_SETUP=1 is set so the installer does not block on its interactive setup.")
                        .size(9)
                        .style(theme::faint_text),
                ]
                .spacing(5),
            )
            .padding(10)
            .width(Fill)
            .style(theme::card),
            checkbox(self.cli_consent)
                .label("I understand this downloads and executes Unity's installer script from unity.com.")
                .on_toggle(Message::CliConsentToggled)
                .size(14)
                .text_size(self.font_size(11.0)),
            row![
                space().width(Fill),
                button(text("Cancel").size(11))
                    .on_press(Message::CloseModal)
                    .padding([7.0, 12.0])
                    .style(button::text),
                button(text("Run installer").size(11))
                    .on_press_maybe(self.cli_consent.then_some(Message::InstallCli))
                    .padding([7.0, 12.0])
                    .style(button::primary),
            ]
            .spacing(8),
        ]
        .spacing(12);

        container(content)
            .width(560)
            .padding(22)
            .style(theme::modal)
            .into()
    }

    fn editor_modules_modal<'a>(&'a self, editor: &'a Editor) -> Element<'a, Message> {
        let installed: Element<'_, Message> = if editor.module_names.is_empty() {
            text("No modules reported.")
                .size(10)
                .style(theme::faint_text)
                .into()
        } else {
            text(editor.module_names.join(" · "))
                .size(10)
                .style(theme::success_text)
                .into()
        };

        let module_list: Element<'_, Message> = if self.dialog_modules.is_loading() {
            text("Loading modules…")
                .size(11)
                .style(theme::muted_text)
                .into()
        } else if let Some(error) = &self.dialog_modules.error {
            text(error_text(error))
                .size(10)
                .style(theme::danger_text)
                .into()
        } else if let Some(modules) = &self.dialog_modules.data {
            scrollable(
                column(
                    modules
                        .iter()
                        .enumerate()
                        .filter(|(_, module)| !module.is_installed)
                        .map(|(index, module)| {
                            checkbox(self.dialog_selected.contains(&module.id))
                                .label(format!(
                                    "{}  ·  {}",
                                    module.name,
                                    format_bytes(module.download_bytes)
                                ))
                                .on_toggle(move |value| Message::ToggleDialogModule(index, value))
                                .size(14)
                                .text_size(self.font_size(10.0))
                                .into()
                        })
                        .collect::<Vec<Element<'_, Message>>>(),
                )
                .spacing(6),
            )
            .height(280)
            .into()
        } else {
            text("Load modules for this Editor to add them.")
                .size(10)
                .style(theme::faint_text)
                .into()
        };

        let content = column![
            text(format!("Modules for Unity {}", editor.version)).size(20),
            text("Only modules that are not already installed are listed.")
                .size(11)
                .style(theme::muted_text),
            text("Installed").size(10).style(theme::faint_text),
            installed,
            module_list,
            checkbox(self.dialog_eula)
                .label("I accept the module license agreements (--accept-eula).")
                .on_toggle(Message::DialogEulaToggled)
                .size(14)
                .text_size(self.font_size(11.0)),
            row![
                space().width(Fill),
                button(text("Cancel").size(11))
                    .on_press(Message::CloseModal)
                    .padding([7.0, 12.0])
                    .style(button::text),
                button(text(format!("Install {} module(s)", self.dialog_selected.len())).size(11))
                    .on_press_maybe(
                        (!self.dialog_selected.is_empty() && self.dialog_eula)
                            .then_some(Message::StartInstallModules)
                    )
                    .padding([7.0, 12.0])
                    .style(button::primary),
            ]
            .spacing(8),
        ]
        .spacing(12);

        container(content)
            .width(620)
            .padding(22)
            .style(theme::modal)
            .into()
    }

    fn upgrade_modal<'a>(&'a self, project: &'a Project) -> Element<'a, Message> {
        let versions = self
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

        let picker: Element<'_, Message> = if versions.is_empty() {
            text("No other Editor version is installed. Install one on the Editors page first.")
                .size(11)
                .style(theme::warning_text)
                .into()
        } else {
            pick_list(
                versions,
                self.upgrade_target.clone(),
                Message::UpgradeTargetChanged,
            )
            .text_size(self.font_size(12.0))
            .width(Fill)
            .placeholder("Select an installed Editor")
            .into()
        };

        let content = column![
            text("Change project Editor version").size(20),
            text(&project.title).size(12).style(theme::muted_text),
            row![
                column![
                    text("Current version").size(10).style(theme::faint_text),
                    text(&project.version).size(12),
                ]
                .spacing(2)
                .width(Fill),
                column![
                    text("Project path").size(10).style(theme::faint_text),
                    text(short_path(&project.path, 60))
                        .size(10)
                        .font(iced::Font::MONOSPACE)
                        .style(theme::muted_text),
                ]
                .spacing(2)
                .width(Fill),
            ]
            .spacing(14),
            text("Target Editor version").size(10).style(theme::faint_text),
            picker,
            container(
                text("This runs `unity projects upgrade <path> --to <version> --yes`. It can update ProjectVersion.txt, open the Editor, and run API upgrades. Commit or back up first.")
                    .size(10)
                    .style(theme::warning_text),
            )
            .padding(10)
            .width(Fill)
            .style(theme::notice_card(theme::Tone::Amber)),
            checkbox(self.upgrade_confirmed)
                .label("I understand and want to upgrade this project now.")
                .on_toggle(Message::UpgradeConfirmed)
                .size(14)
                .text_size(self.font_size(11.0)),
            row![
                space().width(Fill),
                button(text("Cancel").size(11))
                    .on_press(Message::CloseModal)
                    .padding([7.0, 12.0])
                    .style(button::text),
                button(text("Upgrade project").size(11))
                    .on_press_maybe((self.upgrade_confirmed && self.upgrade_target.is_some()).then_some(Message::StartUpgrade))
                    .padding([7.0, 12.0])
                    .style(button::primary),
            ]
            .spacing(8),
        ]
        .spacing(12);

        container(content)
            .width(600)
            .padding(22)
            .style(theme::modal)
            .into()
    }

    fn first_other_editor_version(&self, current: &str) -> Option<String> {
        self.editors
            .data
            .as_ref()?
            .iter()
            .map(|editor| editor.version.clone())
            .find(|version| version != current)
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

pub fn detail_column<'a>(label: &'a str, value: impl IntoFragment<'a>) -> Element<'a, Message> {
    column![
        text(label).size(9).style(theme::faint_text),
        text(value).size(11).style(theme::muted_text),
    ]
    .spacing(2)
    .width(Fill)
    .into()
}

pub fn badge<'a>(label: impl IntoFragment<'a>, tone: theme::Tone) -> Element<'a, Message> {
    container(text(label).size(10))
        .padding([3.0, 8.0])
        .style(theme::badge(tone))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
