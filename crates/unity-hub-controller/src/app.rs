use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::{Subscription, Task};
use serde_json::Value;
use unity_core::bootstrap;
use unity_core::changelog::{ChangelogRequest, ChangelogView, CliChangelogProvider};
use unity_core::models::{
    AuthStatus, CliStatus, CliUpdateInfo, CreateProjectOptions, CreateProjectResult, Editor,
    InstallPreview, LicenseStatus, Module, Project, Release, RunningEditor, RunningEditors,
    TemplateBrief, TemplateCreateOptions, TemplateFilter, TemplateInfo, TemplatePackOptions,
};
use unity_core::versions::{
    validate_archive_path, validate_output_dir, validate_package_id, validate_project_name,
    validate_project_path, validate_template_version,
};
use unity_core::{CommandSpec, UnityCli, UnityError};
use unity_hub_model::{
    AccountCardVm, AuthVm, BadgeVm, BreakingChangeVm, ChangelogDataVm, ChangelogSectionVm,
    ChangelogVm, CliCardVm, CliConnectionVm, CliDetailsVm, EditorModulesVm, EditorRowVm, EditorsVm,
    InstallCliVm, InstallEditorVm, InstallPreviewVm, JobPhase, JobVm, LicenseRowVm, LicenseVm,
    LoadState, ModalVm, ModuleOptionVm, NewProjectEditorVm, NewProjectField, NewProjectFormVm,
    NewProjectPreflightState, NewProjectPreflightVm, NewProjectResultVm, NewProjectStep,
    NewProjectVm, NoticeVm, OverviewVm, Page, PageVm, ProcessMetricsVm, ProcessRowVm, ProcessesVm,
    ProjectRowVm, ProjectsVm, ReleaseRowVm, RemoveProjectVm, RunningEditorVm, SectionKind,
    SettingsVm, ShellVm, TemplateAuthoringMode, TemplateAuthoringVm, TemplateDetailVm,
    TemplateField, TemplateFormVm, TemplateKindFilter, TemplateRowVm, TemplatesVm,
    TextScaleOptionVm, Tone, UiAction, UiError, UpdateVm, UpgradeProjectVm, ViewModel,
};

use crate::jobs::{JobManager, JobStarted, JobState, StreamKind};
use crate::metrics::{ProcessMetrics, Sampler};
use crate::settings::{self, AppSettings, TextSize};
use crate::util::{
    error_text, format_bytes, format_duration, format_epoch_millis, format_iso, open_folder,
    run_blocking, short_path,
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

fn auth_broker_hint(error: &UnityError) -> String {
    format!(
        "The Unity CLI could not start its sign-in service, so each command may ask to unlock the system keyring. {}",
        error_text(error).replace('\n', " ")
    )
}

#[cfg(target_os = "linux")]
fn broker_env_notice() -> Option<Notice> {
    std::env::var_os("UNITY_NO_AUTH_BROKER").map(|_| {
        Notice::info(
            "UNITY_NO_AUTH_BROKER is set, so every Unity CLI command reads the system keyring directly and may ask to unlock it repeatedly. Unset it to let the CLI share one sign-in session.",
        )
    })
}

#[cfg(not(target_os = "linux"))]
fn broker_env_notice() -> Option<Notice> {
    None
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
    pub message: Option<String>,
    pub logs: VecDeque<LogLine>,
    /// The rendered form of `logs`, rebuilt only when the buffer changes so the
    /// view can hand iced a stable string.
    pub log_text: Arc<str>,
    pub error: Option<UnityError>,
    /// The last NDJSON result frame. Streaming writes read created paths and
    /// `steps[]` from it after the job ends.
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
    // UI intents. These are constructed from `UiAction` so the visual layer
    // never needs to know about the controller's async message variants.
    Navigate(Page),
    Refresh,
    DismissJob,
    CancelJob,
    DismissNotice,
    SignIn,
    TextSizeChanged(TextSize),
    FontFamilyChanged(String),
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
    OpenModules(String),
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
    ChangelogTargetChanged(String),
    LoadChangelog,
    ToggleRawChangelog,
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
    RawChangelogLoaded(Result<String, UnityError>),
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
            UiAction::DismissNotice => Self::DismissNotice,
            UiAction::DismissJob => Self::DismissJob,
            UiAction::CancelJob => Self::CancelJob,
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
            UiAction::InstallVersionChanged(version) => Self::InstallVersionChanged(version),
            UiAction::LoadModules => Self::LoadModules,
            UiAction::ToggleModule(id, value) => Self::ToggleModule(id, value),
            UiAction::SelectAllModules(value) => Self::SelectAllModules(value),
            UiAction::AcceptEulaToggled(value) => Self::AcceptEulaToggled(value),
            UiAction::PreviewInstall => Self::PreviewInstall,
            UiAction::StartInstallEditor => Self::StartInstallEditor,
            UiAction::UseRelease(version) => Self::UseRelease(version),
            UiAction::OpenEditorModules(version) => Self::OpenModules(version),
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
            UiAction::ChangelogTargetChanged(target) => Self::ChangelogTargetChanged(target),
            UiAction::LoadChangelog => Self::LoadChangelog,
            UiAction::ToggleRawChangelog => Self::ToggleRawChangelog,
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
            notice: broker_env_notice(),
            boot_background_started: false,
        };

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

        Subscription::batch([job, processes])
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
                        "AUTH_BROKER_UNAVAILABLE"
                            | "AUTH_BROKER_REFUSED"
                            | "CREDENTIAL_STORE_ACCESS_DENIED"
                    ) {
                        self.notice = Some(Notice::error(auth_broker_hint(error)));
                    }
                }
                // A missing broker only means each read may prompt again; the
                // Overview is still worth loading.
                self.load_overview_data(false)
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
                let mut task = Task::none();
                match result {
                    Ok(editors) => {
                        self.editors.succeeded(editors);
                        self.ensure_upgrade_target();
                        if self.page == Page::Templates
                            && self.template_editor.is_none()
                            && self.templates.data.is_none()
                        {
                            task = self.load_templates(false);
                        }
                    }
                    Err(error) => self.editors.failed(error),
                }
                task
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
            Message::FontFamilyChanged(family) => {
                let family = family.trim();
                self.app_settings.font_family = if family.is_empty() {
                    settings::default_font_family()
                } else {
                    family.to_string()
                };
                match settings::save(&self.app_settings) {
                    Ok(()) => {
                        self.notice = Some(Notice::success(format!(
                            "Font set to {}.",
                            self.app_settings.font_family
                        )));
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
            Message::OpenModules(version) => {
                let Some(editor) = self
                    .editors
                    .data
                    .as_ref()
                    .and_then(|editors| editors.iter().find(|editor| editor.version == version))
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
            Message::AddProject => {
                Task::perform(pick_project_folder(), Message::ProjectFolderPicked)
            }
            Message::ProjectFolderPicked(Some(path)) => {
                let path = path.display().to_string();
                self.notice = Some(Notice::info(format!("Adding {path}…")));
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
                self.notice = Some(Notice::info(format!("Removing {title} from the registry…")));
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
                    self.notice = Some(Notice::success(match mutation {
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
                    }));
                    self.load_projects_page(true)
                }
                Err(error) => {
                    self.notice = Some(Notice::error(error_text(&error)));
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
                self.notice = Some(Notice::info(format!("Opening {title}…")));
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
                    text_scale_percent: self.app_settings.text_scale_percent,
                    font_family: self.app_settings.font_family.clone(),
                    process_metrics: self.app_settings.process_metrics,
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
                self.notice = None;
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
                    self.notice = Some(Notice::error(error_text(&error)));
                }
                Task::none()
            }
            Message::CopyText(text) => {
                self.notice = Some(Notice::success("Copied to the clipboard."));
                iced::clipboard::write(text)
            }
            Message::OpenFolder(path) => {
                if let Err(error) = open_folder(&path) {
                    self.notice = Some(Notice::error(error_text(&error)));
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
                    .as_ref()
                    .map(error_text)
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
            Page::Templates => self.load_templates(false),
            Page::Processes => self.load_processes(false),
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
            notice: self.notice.as_ref().map(|notice| NoticeVm {
                tone: notice_tone(notice.kind),
                text: notice.text.clone(),
            }),
            job: self.active_job.as_ref().map(job_vm),
        }
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
                    stale: auth.session_state.as_deref() == Some("stale"),
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
            message: None,
            logs: VecDeque::new(),
            log_text: Arc::from(""),
            error: None,
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
}
