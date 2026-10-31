use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, space, stack, text,
};
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
use crate::settings::{self, AppSettings};
use crate::theme;
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
    pub logs: Vec<LogLine>,
    pub result: Option<Value>,
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
            logs: Vec::new(),
            result: None,
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
    pub upgrade_target: Option<String>,
    pub upgrade_confirmed: bool,
    pub app_settings: AppSettings,
    pub cli_path_input: String,
    pub config_dir: Option<String>,
    pub active_job: Option<ActiveJob>,
    pub notice: Option<Notice>,
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
    PollJobs,
    DismissJob,
    CancelJob,
    DismissNotice,
    OpenInstallCli,
    CliChannelChanged(String),
    CliConsentToggled(bool),
    InstallCli,
    UpdateCli,
    InstallVersionChanged(String),
    LoadModules,
    ToggleModule(String, bool),
    SelectAllModules(bool),
    AcceptEulaToggled(bool),
    PreviewInstall,
    StartInstallEditor,
    UseRelease(String),
    OpenModules(Editor),
    CloseModal,
    ToggleDialogModule(String, bool),
    DialogEulaToggled(bool),
    StartInstallModules,
    ProjectSearchChanged(String),
    OpenUpgrade(Project),
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
            upgrade_target: None,
            upgrade_confirmed: false,
            cli_path_input: app_settings.cli_path.clone().unwrap_or_default(),
            app_settings,
            config_dir: settings::config_dir().map(|path| path.display().to_string()),
            active_job: None,
            notice: None,
        };

        let tasks = Task::batch(vec![
            load_status(app.cli.clone()),
            load_update(app.cli.clone()),
            load_auth(app.cli.clone()),
            load_license(app.cli.clone()),
            load_running(app.cli.clone()),
        ]);

        (app, tasks)
    }

    pub fn theme(&self) -> Theme {
        theme::app_theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // iced 0.14 no longer exposes `time::every`; a tiny timer thread keeps
        // the job event queue polled without depending on a specific executor.
        Subscription::run(|| {
            let (sender, receiver) = futures::channel::mpsc::unbounded();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(120));
                if sender.unbounded_send(Message::PollJobs).is_err() {
                    break;
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
                match result {
                    Ok(status) => {
                        if let Some(version) = &status.version {
                            if version.channel == "beta" || version.channel == "alpha" {
                                self.cli_channel = version.channel.clone();
                            }
                        }
                        self.cli_status.succeeded(status);
                    }
                    Err(error) => self.cli_status.failed(error),
                }
                Task::none()
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
                    Ok(editors) => self.editors.succeeded(editors),
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
                    Ok(projects) => self.projects.succeeded(projects),
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
                    Ok(value) => {
                        self.preview_raw = Some(value.clone());
                        self.preview = serde_json::from_value::<InstallPreview>(value).ok();
                    }
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
            Message::UseRelease(version) => {
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
            Message::OpenModules(editor) => {
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
            Message::OpenUpgrade(project) => {
                let installed = self
                    .editors
                    .data
                    .as_ref()
                    .map(|editors| {
                        editors
                            .iter()
                            .map(|editor| editor.version.clone())
                            .filter(|version| version != &project.version)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                self.upgrade_target = installed.first().cloned();
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
                if self.changelog_loaded {
                    self.changelog.begin();
                    Task::perform(
                        load_changelog_future(self.cli.clone(), self.changelog_target.clone()),
                        Message::ChangelogLoaded,
                    )
                } else {
                    Task::none()
                }
            }
            Message::LoadChangelog => {
                self.changelog_loaded = true;
                self.changelog.begin();
                Task::perform(
                    load_changelog_future(self.cli.clone(), self.changelog_target.clone()),
                    Message::ChangelogLoaded,
                )
            }
            Message::ToggleRawChangelog => {
                self.show_raw_changelog = !self.show_raw_changelog;
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
                        load_status(self.cli.clone())
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
                        load_status(self.cli.clone())
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
        let events = self.jobs.poll();
        let mut finished: Option<(String, JobState, Option<UnityError>)> = None;

        for event in events {
            let Some(job) = self.active_job.as_mut() else {
                continue;
            };
            if job.id != event.job_id {
                continue;
            }

            if let Some(line) = event.line {
                let text = if line.chars().count() > 4000 {
                    line.chars().take(4000).collect::<String>() + "…"
                } else {
                    line
                };
                job.logs.push(LogLine {
                    stream: event.stream.unwrap_or(StreamKind::Stdout),
                    text,
                });
                if job.logs.len() > 600 {
                    job.logs.drain(..100);
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
                job.result = event.result.clone();
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
            "cli-install" | "cli-update" => Task::batch(vec![
                load_status(self.cli.clone()),
                load_update(self.cli.clone()),
                load_auth(self.cli.clone()),
                load_license(self.cli.clone()),
                load_running(self.cli.clone()),
            ]),
            "editor-install" | "module-install" => Task::batch(vec![
                load_editors(self.cli.clone()),
                load_releases(self.cli.clone()),
            ]),
            "project-upgrade" => load_projects(self.cli.clone()),
            _ => Task::none(),
        }
    }

    fn ensure_page(&mut self, page: Page) -> Task<Message> {
        match page {
            Page::Editors if self.editors.data.is_none() => {
                self.editors.begin();
                self.releases.begin();
                Task::batch(vec![
                    load_editors(self.cli.clone()),
                    load_releases(self.cli.clone()),
                ])
            }
            Page::Projects if self.projects.data.is_none() => {
                self.projects.begin();
                if self.editors.data.is_none() {
                    self.editors.begin();
                }
                Task::batch(vec![
                    load_projects(self.cli.clone()),
                    load_editors(self.cli.clone()),
                ])
            }
            Page::Changelog if !self.changelog_loaded => {
                self.changelog_loaded = true;
                self.changelog.begin();
                Task::perform(
                    load_changelog_future(self.cli.clone(), self.changelog_target.clone()),
                    Message::ChangelogLoaded,
                )
            }
            _ => Task::none(),
        }
    }

    fn refresh_page(&mut self) -> Task<Message> {
        match self.page {
            Page::Overview => {
                self.cli_status.begin();
                self.update.begin();
                self.auth.begin();
                self.license.begin();
                self.running.begin();
                Task::batch(vec![
                    load_status(self.cli.clone()),
                    load_update(self.cli.clone()),
                    load_auth(self.cli.clone()),
                    load_license(self.cli.clone()),
                    load_running(self.cli.clone()),
                ])
            }
            Page::Editors => {
                self.editors.begin();
                self.releases.begin();
                Task::batch(vec![
                    load_editors(self.cli.clone()),
                    load_releases(self.cli.clone()),
                ])
            }
            Page::Projects => {
                self.projects.begin();
                self.editors.begin();
                Task::batch(vec![
                    load_projects(self.cli.clone()),
                    load_editors(self.cli.clone()),
                ])
            }
            Page::Changelog if self.changelog_loaded => {
                self.changelog.begin();
                Task::perform(
                    load_changelog_future(self.cli.clone(), self.changelog_target.clone()),
                    Message::ChangelogLoaded,
                )
            }
            Page::Changelog => Task::none(),
            Page::Settings => load_status(self.cli.clone()),
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let base = self.shell();
        if let Some(overlay) = self.modal_view() {
            stack![base, overlay].width(Fill).height(Fill).into()
        } else {
            base
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
            .map(|version| version.version.clone())
            .unwrap_or_else(|| "not detected".to_string());
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
            .map(|version| version.version.clone());

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
            let lines = column(
                job.logs
                    .iter()
                    .map(|line| {
                        text(&line.text)
                            .size(9)
                            .font(iced::Font::MONOSPACE)
                            .style(if line.stream == StreamKind::Stderr {
                                theme::danger_text
                            } else {
                                theme::muted_text
                            })
                            .into()
                    })
                    .collect::<Vec<Element<'_, Message>>>(),
            )
            .spacing(2);

            body = body.push(
                container(scrollable(lines).height(150))
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
                .style(theme::backdrop)
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
                .text_size(11),
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
                        .filter(|module| !module.is_installed)
                        .map(|module| {
                            checkbox(self.dialog_selected.contains(&module.id))
                                .label(format!(
                                    "{}  ·  {}",
                                    module.name,
                                    format_bytes(module.download_bytes)
                                ))
                                .on_toggle({
                                    let id = module.id.clone();
                                    move |value| Message::ToggleDialogModule(id.clone(), value)
                                })
                                .size(14)
                                .text_size(10)
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
                .text_size(11),
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
                    .map(|editor| editor.version.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let picker: Element<'_, Message> = if versions.is_empty() {
            text("No Editors installed. Install one on the Editors page first.")
                .size(11)
                .style(theme::warning_text)
                .into()
        } else {
            pick_list(
                versions,
                self.upgrade_target.clone(),
                Message::UpgradeTargetChanged,
            )
            .width(Fill)
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
                .text_size(11),
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

    pub fn filtered_projects(&self) -> Vec<Project> {
        let needle = self.project_search.trim().to_lowercase();
        let projects = self.projects.data.clone().unwrap_or_default();
        if needle.is_empty() {
            return projects;
        }

        projects
            .into_iter()
            .filter(|project| {
                project.title.to_lowercase().contains(&needle)
                    || project.path.to_lowercase().contains(&needle)
                    || project.version.to_lowercase().contains(&needle)
            })
            .collect()
    }

    pub fn installed_versions(&self) -> Vec<String> {
        self.editors
            .data
            .as_ref()
            .map(|editors| {
                editors
                    .iter()
                    .map(|editor| editor.version.clone())
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
        run_blocking(move || cli.releases(40, 0, None)),
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

pub fn detail_column<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    column![
        text(label).size(9).style(theme::faint_text),
        text(value).size(11).style(theme::muted_text),
    ]
    .spacing(2)
    .width(Fill)
    .into()
}

pub fn badge<'a>(label: impl Into<String>, tone: theme::Tone) -> Element<'a, Message> {
    container(text(label.into()).size(10))
        .padding([3.0, 8.0])
        .style(theme::badge(tone))
        .into()
}
