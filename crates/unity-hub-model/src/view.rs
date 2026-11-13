use std::fmt;
use std::sync::Arc;

/// Top-level screens. Labels and titles are owned by the UI crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Overview,
    Editors,
    Projects,
    Templates,
    Processes,
    Changelog,
    Settings,
}

impl Page {
    pub const ALL: [Page; 7] = [
        Page::Overview,
        Page::Editors,
        Page::Projects,
        Page::Templates,
        Page::Processes,
        Page::Changelog,
        Page::Settings,
    ];
}

/// A display-ready error. The controller resolves `UnityError` into this shape
/// so the UI never depends on the CLI adapter's error type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiError {
    pub message: String,
    pub detail: Option<String>,
}

impl UiError {
    pub fn text(&self) -> String {
        match &self.detail {
            Some(detail) if !detail.trim().is_empty() => {
                format!("{}\n\n{}", self.message, detail.trim())
            }
            _ => self.message.clone(),
        }
    }
}

/// A loaded value plus its in-flight/error state.
///
/// `previous` keeps the last good value when a refresh starts or fails, which
/// lets a screen keep showing content while a background read is running.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LoadState<T> {
    #[default]
    Idle,
    Loading {
        previous: Option<T>,
    },
    Ready(T),
    Failed {
        error: UiError,
        previous: Option<T>,
    },
}

impl<T> LoadState<T> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    pub fn error(&self) -> Option<&UiError> {
        match self {
            Self::Failed { error, .. } => Some(error),
            _ => None,
        }
    }

    /// The value to draw: the current value, or the last good value during a
    /// refresh/failure.
    pub fn data(&self) -> Option<&T> {
        match self {
            Self::Ready(data) => Some(data),
            Self::Loading { previous } | Self::Failed { previous, .. } => previous.as_ref(),
            Self::Idle => None,
        }
    }
}

/// Semantic tone. The UI maps these to design-token colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
    Info,
    Highlight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadgeVm {
    pub label: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeVm {
    pub tone: Tone,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobPhase {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobVm {
    pub kind: String,
    pub command: String,
    pub phase: JobPhase,
    pub progress: f32,
    pub message: Option<String>,
    pub error: Option<UiError>,
    /// The bounded log tail, preformatted by the controller: stderr lines are
    /// prefixed with `!` and every line ends with a newline. Shared, so a view
    /// that only changes progress does not copy or reshape the log.
    pub logs: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliConnectionVm {
    Checking,
    Connected { version: Option<String> },
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellVm {
    pub page: Page,
    pub cli: CliConnectionVm,
    pub notice: Option<NoticeVm>,
    pub job: Option<JobVm>,
}

// One snapshot exists per frame, so the variant size gap costs nothing worth
// boxing for.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum PageVm {
    Overview(OverviewVm),
    Editors(EditorsVm),
    Projects(ProjectsVm),
    Templates(TemplatesVm),
    Processes(ProcessesVm),
    Changelog(ChangelogVm),
    Settings(SettingsVm),
}

#[derive(Debug, Clone)]
pub enum ModalVm {
    InstallCli(InstallCliVm),
    EditorModules(EditorModulesVm),
    UpgradeProject(UpgradeProjectVm),
    RemoveProject(RemoveProjectVm),
    NewProject(Box<NewProjectVm>),
    TemplateAuthoring(Box<TemplateAuthoringVm>),
}

#[derive(Debug, Clone)]
pub struct ViewModel {
    pub shell: ShellVm,
    pub page: PageVm,
    pub modal: Option<ModalVm>,
    pub text_scale: f32,
}

// ---------------------------------------------------------------------------
// Overview

#[derive(Debug, Clone)]
pub struct OverviewVm {
    pub cli: CliCardVm,
    pub update: LoadState<UpdateVm>,
    pub account: AccountCardVm,
    pub running: LoadState<Vec<RunningEditorVm>>,
    /// Installed Editors, once the list has loaded.
    pub editor_count: Option<usize>,
    /// Registered projects, once the list has loaded.
    pub project_count: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum CliCardVm {
    Idle,
    Loading,
    Missing,
    Ready {
        details: CliDetailsVm,
        error: Option<UiError>,
    },
    Failed(UiError),
}

#[derive(Debug, Clone)]
pub struct CliDetailsVm {
    pub version: String,
    pub channel: String,
    pub platform: String,
    pub runtime: String,
    pub executable: String,
    pub resolved_via: String,
    pub install_path: String,
    pub build_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateVm {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
}

#[derive(Debug, Clone)]
pub struct AccountCardVm {
    pub auth: AuthVm,
    pub license: LicenseVm,
}

#[derive(Debug, Clone)]
pub enum AuthVm {
    Idle,
    Loading,
    SignedIn {
        name: String,
        email: String,
        /// The CLI still has a session, but its token needs a refresh.
        stale: bool,
    },
    SignedOut {
        expired: bool,
    },
    Failed(UiError),
}

#[derive(Debug, Clone)]
pub enum LicenseVm {
    Idle,
    Ready {
        active: bool,
        licenses: Vec<LicenseRowVm>,
    },
    Failed(UiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseRowVm {
    pub product: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningEditorVm {
    pub project_path: String,
    pub editor_version: String,
    pub pid: String,
    pub phase: Option<String>,
    pub pipeline_version: Option<String>,
    pub update_available: bool,
    pub safe_mode: bool,
    pub dialog: Option<String>,
}

// ---------------------------------------------------------------------------
// Editors

#[derive(Debug, Clone)]
pub struct EditorsVm {
    pub installed: LoadState<Vec<EditorRowVm>>,
    pub install: InstallEditorVm,
    pub releases: LoadState<Vec<ReleaseRowVm>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorRowVm {
    pub version: String,
    pub architecture: String,
    pub is_default: bool,
    /// Installed module names; empty when the CLI reports none.
    pub modules: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InstallEditorVm {
    pub version_input: String,
    pub modules: LoadState<Vec<ModuleOptionVm>>,
    pub available_count: usize,
    pub accept_eula: bool,
    pub can_install: bool,
    pub preview: Option<InstallPreviewVm>,
    pub preview_note: Option<String>,
    pub preview_loading: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleOptionVm {
    pub id: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPreviewVm {
    pub editor_version: String,
    pub download_size: String,
    pub module_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRowVm {
    pub version: String,
    pub stream: String,
    pub stream_tone: Tone,
    pub installed: bool,
}

// ---------------------------------------------------------------------------
// Projects

#[derive(Debug, Clone)]
pub struct ProjectsVm {
    pub search: String,
    pub total_count: usize,
    pub visible_count: usize,
    pub projects: LoadState<Vec<ProjectRowVm>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRowVm {
    pub path: String,
    pub title: String,
    pub version: String,
    pub pipeline: Option<BadgeVm>,
    pub build_target: String,
    pub short_path: String,
    pub modified: String,
    pub is_favorite: bool,
    pub badges: Vec<BadgeVm>,
}

// ---------------------------------------------------------------------------
// Changelog

#[derive(Debug, Clone)]
pub struct ChangelogVm {
    pub loaded: bool,
    pub versions: Vec<String>,
    pub selected_target: Option<String>,
    pub show_raw: bool,
    pub state: LoadState<ChangelogDataVm>,
    pub raw: LoadState<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogDataVm {
    pub version: String,
    pub source: String,
    pub source_detail: String,
    pub entry_count: usize,
    pub breaking_changes: Vec<BreakingChangeVm>,
    pub sections: Vec<ChangelogSectionVm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakingChangeVm {
    pub text: String,
    pub section: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogSectionVm {
    pub title: String,
    pub kind: SectionKind,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Removed,
    Changed,
    Added,
    Fixed,
    Security,
    Other,
}

// ---------------------------------------------------------------------------
// Settings

#[derive(Debug, Clone)]
pub struct SettingsVm {
    pub resolved_executable: String,
    pub resolved_via: String,
    pub cli_path_input: String,
    pub config_dir: String,
    pub settings_file: String,
    pub text_scale_options: Vec<TextScaleOptionVm>,
    pub text_scale_percent: u16,
    pub font_options: Arc<[String]>,
    pub font_family: String,
    pub engine_info: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextScaleOptionVm {
    pub percent: u16,
    pub label: String,
}

impl fmt::Display for TextScaleOptionVm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

// ---------------------------------------------------------------------------
// Modals

#[derive(Debug, Clone)]
pub struct InstallCliVm {
    pub channels: Vec<String>,
    pub selected_channel: String,
    pub consent: bool,
    pub command_preview: String,
    pub can_install: bool,
}

#[derive(Debug, Clone)]
pub struct EditorModulesVm {
    pub editor_version: String,
    pub installed_modules: Vec<String>,
    pub modules: LoadState<Vec<ModuleOptionVm>>,
    pub selected_count: usize,
    pub accept_eula: bool,
    pub can_install: bool,
}

#[derive(Debug, Clone)]
pub struct UpgradeProjectVm {
    pub project_title: String,
    pub project_path_short: String,
    pub current_version: String,
    pub target_options: Vec<String>,
    pub selected_target: Option<String>,
    pub confirmed: bool,
    pub can_upgrade: bool,
}

#[derive(Debug, Clone)]
pub struct RemoveProjectVm {
    pub project_title: String,
    pub project_path_short: String,
}

// ---------------------------------------------------------------------------
// Templates

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemplateKindFilter {
    #[default]
    All,
    Core,
    Custom,
}

impl TemplateKindFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Core, Self::Custom];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Core => "Core",
            Self::Custom => "Custom",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TemplatesVm {
    /// Installed Editor versions, newest first.
    pub editors: LoadState<Vec<String>>,
    pub selected_editor: Option<String>,
    pub filter: TemplateKindFilter,
    pub installed_only: bool,
    pub list: LoadState<Vec<TemplateRowVm>>,
    pub detail: Option<TemplateDetailVm>,
    pub detail_loading: bool,
    pub detail_error: Option<UiError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRowVm {
    pub name: String,
    pub display_name: String,
    pub kind: String,
    pub version: String,
    pub status: String,
    pub status_tone: Tone,
    pub installed: bool,
    pub pipeline: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDetailVm {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub kind: String,
    pub version: String,
    pub status: String,
    pub installed: bool,
    pub render_pipeline: Option<String>,
    pub packages: Vec<String>,
    pub previous_versions: Vec<String>,
}

/// What a template authoring form is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateAuthoringMode {
    /// `templates create` — register a template in the user template store.
    Create,
    /// `templates pack` — write a portable `.tgz`.
    Pack,
}

impl TemplateAuthoringMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Create => "Create template",
            Self::Pack => "Pack template",
        }
    }

    pub fn is_pack(self) -> bool {
        matches!(self, Self::Pack)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateField {
    Name,
    DisplayName,
    Description,
    Version,
    Output,
}

#[derive(Debug, Clone, Default)]
pub struct TemplateFormVm {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct TemplateAuthoringVm {
    pub mode: TemplateAuthoringMode,
    pub projects: Vec<String>,
    pub selected_project: Option<String>,
    pub form: TemplateFormVm,
    pub overwrite: bool,
    pub overwrite_confirmed: bool,
    pub can_submit: bool,
    pub issues: Vec<String>,
    pub command_preview: String,
    pub result: Option<String>,
    pub job: Option<JobVm>,
}

// ---------------------------------------------------------------------------
// Processes

#[derive(Debug, Clone)]
pub struct ProcessesVm {
    pub rows: LoadState<Vec<ProcessRowVm>>,
    pub metrics_enabled: bool,
    /// True when at least one row came from the Pipeline package.
    pub pipeline_available: bool,
    pub last_refreshed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRowVm {
    pub project_path: String,
    pub project_name: String,
    pub editor_version: String,
    pub pid: Option<String>,
    pub port: Option<String>,
    pub state: String,
    pub state_tone: Tone,
    pub phase: Option<String>,
    pub play_mode: bool,
    pub frame_count: Option<String>,
    pub pipeline_version: Option<String>,
    pub safe_mode: bool,
    pub dialog: Option<String>,
    pub metrics: Option<ProcessMetricsVm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessMetricsVm {
    pub cpu: String,
    pub memory: String,
    pub uptime: String,
}

// ---------------------------------------------------------------------------
// New Project wizard

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewProjectStep {
    Details,
    Editor,
    Template,
    Review,
    Result,
}

impl NewProjectStep {
    pub fn index(self) -> usize {
        match self {
            Self::Details => 0,
            Self::Editor => 1,
            Self::Template => 2,
            Self::Review => 3,
            Self::Result => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Details => "Name and folder",
            Self::Editor => "Editor",
            Self::Template => "Template",
            Self::Review => "Review",
            Self::Result => "Result",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewProjectField {
    Name,
    ParentDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewProjectPreflightState {
    Idle,
    Checking,
    Ready,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct NewProjectPreflightVm {
    pub state: NewProjectPreflightState,
    pub license_ok: bool,
    pub message: Option<String>,
    pub can_continue: bool,
}

#[derive(Debug, Clone)]
pub struct NewProjectEditorVm {
    pub version: String,
    pub architecture: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NewProjectFormVm {
    pub name: String,
    pub parent_dir: String,
    pub editor_version: String,
    pub architecture: String,
    pub template: String,
    pub open_after_create: bool,
    pub show_builtin_templates: bool,
}

#[derive(Debug, Clone)]
pub struct NewProjectVm {
    pub step: NewProjectStep,
    pub form: NewProjectFormVm,
    pub editors: LoadState<Vec<NewProjectEditorVm>>,
    pub templates: LoadState<Vec<TemplateRowVm>>,
    pub selected_template: Option<TemplateDetailVm>,
    pub preflight: NewProjectPreflightVm,
    pub issues: Vec<String>,
    pub command_preview: String,
    pub result: Option<NewProjectResultVm>,
    pub job: Option<JobVm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProjectResultVm {
    pub success: bool,
    pub path: Option<String>,
    pub summary: String,
    pub failed_steps: Vec<String>,
}
