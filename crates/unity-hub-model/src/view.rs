use std::fmt;
use std::sync::Arc;

/// Top-level screens. Labels and titles are owned by the UI crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Changelog(ChangelogVm),
    Settings(SettingsVm),
}

#[derive(Debug, Clone)]
pub enum ModalVm {
    InstallCli(InstallCliVm),
    EditorModules(EditorModulesVm),
    UpgradeProject(UpgradeProjectVm),
    RemoveProject(RemoveProjectVm),
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
    pub font_options: Vec<String>,
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
