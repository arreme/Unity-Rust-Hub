use std::fmt;

/// Top-level screens. Labels and titles are owned by the UI crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Overview,
    Editors,
    Projects,
    Diagnostics,
    Changelog,
    Settings,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::Overview,
        Page::Editors,
        Page::Projects,
        Page::Diagnostics,
        Page::Changelog,
        Page::Settings,
    ];
}

/// The kind of primary next step an error offers.
///
/// `Retry` carries the controller's stable identifier for the failed read, so
/// two visible errors of the same kind never retry each other's operation. The
/// controller decides which kinds it can honour; the UI only draws the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorActionKind {
    Retry(String),
    SignIn,
    OpenSettings,
    PickVersion,
    OpenEditor,
    EnterToken,
    InstallGit,
    ShowCommand,
    Confirm,
    ShowBusy,
    AnswerDialog,
    FixPermissions,
}

/// One primary action attached to a failed read or job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorActionVm {
    pub label: String,
    /// The exact command the CLI named, when it named one. It is shown and
    /// copied, never executed by this action.
    pub command: Option<String>,
    pub kind: ErrorActionKind,
}

/// A display-ready error. The controller resolves `UnityError` into this shape
/// so the UI never depends on the CLI adapter's error type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiError {
    /// Stable machine code (`NETWORK_UNREACHABLE`, `CLI_NOT_FOUND`, ...).
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    /// The code-specific next step, when the adapter's table has one and the
    /// controller can honour it.
    pub action: Option<ErrorActionVm>,
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

/// A CLI advisory (`notifications[]` or `warnings[]`) as a dismissible banner.
///
/// The controller resolves the CLI's `code`, `data`, and `remediation` into
/// display fields, so the UI never interprets a code or invents a meaning for
/// an unknown one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisoryVm {
    /// Stable identity for dismissal and remediation actions.
    pub id: String,
    pub tone: Tone,
    pub title: String,
    /// The CLI's own message, verbatim.
    pub message: String,
    /// Per-code summary derived from `data`, for example
    /// `1.0.0-beta.16 → 1.0.0-beta.17`.
    pub detail: Option<String>,
    /// The exact command the CLI named as the remedy, when it named one.
    pub command: Option<String>,
    /// Label for the run button. `None` means the adapter has no explicit
    /// builder for this remediation; the command stays copy-only.
    pub run_label: Option<String>,
    /// The CLI asked for an explicit human decision before the command runs.
    pub requires_approval: bool,
    pub dismissible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobPhase {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobLogVm {
    pub is_stderr: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobVm {
    pub kind: String,
    pub command: String,
    pub phase: JobPhase,
    pub progress: f32,
    pub message: Option<String>,
    pub error: Option<UiError>,
    pub logs: Vec<JobLogVm>,
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
    /// Advisories from the most recent reads and job frames, deduplicated and
    /// minus the ones dismissed for this CLI version.
    pub advisories: Vec<AdvisoryVm>,
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
    Diagnostics(DiagnosticsVm),
    Changelog(ChangelogVm),
    Settings(SettingsVm),
}

// One snapshot exists per frame, so the variant size gap costs nothing worth
// boxing for.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum ModalVm {
    InstallCli(InstallCliVm),
    EditorModules(EditorModulesVm),
    UpgradeProject(UpgradeProjectVm),
    PruneEditors(PruneEditorsVm),
    VerifyEditor(VerifyEditorVm),
    RollbackCli(RollbackCliVm),
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
    /// The CLI self-update surface: channel/target, the target release notes
    /// that must be shown before applying, the dry-run preview, and the
    /// rollback entry point.
    pub update_flow: CliUpdateFlowVm,
    pub account: AccountCardVm,
    pub running: LoadState<Vec<RunningEditorVm>>,
    /// Installed Editors, once the list has loaded.
    pub editor_count: Option<usize>,
    /// Registered projects, once the list has loaded.
    pub project_count: Option<usize>,
}

/// `unity self-update` controls and their current read states.
///
/// `can_apply` stays false until the target changelog has loaded, so the app
/// never replaces the CLI before showing what the target release changes.
#[derive(Debug, Clone)]
pub struct CliUpdateFlowVm {
    /// `stable` or `beta`.
    pub channel: String,
    pub channel_options: Vec<String>,
    /// Empty means the newest release on the channel.
    pub target_input: String,
    pub changelog: LoadState<CliChangelogVm>,
    pub dry_run: LoadState<String>,
    pub changelog_command: String,
    pub apply_command: String,
    pub rollback_command: String,
    pub can_apply: bool,
    pub apply_blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliChangelogVm {
    pub version: String,
    pub text: String,
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
    SignedIn { name: String, email: String },
    SignedOut { expired: bool },
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
    /// The last `unity editors prune` report. Report-only: opening it removes
    /// nothing, and the modal repeats it before any confirm.
    pub prune: LoadState<PruneReportVm>,
}

/// A prune report as the page and dialog draw it.
///
/// `unused` and `missing` are separate candidate classes because they have
/// different consequences: one deletes files, the other only edits the
/// registry. `reclaimable` is the CLI's own total, formatted for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneReportVm {
    pub unused: Vec<PruneCandidateVm>,
    pub missing: Vec<PruneCandidateVm>,
    pub reclaimable: String,
    pub removed: bool,
}

/// One candidate line: version, architecture, size when the CLI reports it,
/// and the CLI's reason or classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneCandidateVm {
    pub version: String,
    pub architecture: String,
    pub path: String,
    pub size: Option<String>,
    pub reason: String,
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
    /// The build the user asked for. `None` means "host default", which keeps
    /// `--architecture` out of the command exactly as before.
    pub architecture: Option<String>,
    pub architecture_options: Vec<ArchitectureOptionVm>,
    pub modules: LoadState<Vec<ModuleOptionVm>>,
    pub available_count: usize,
    /// Install the headless Editor (`--headless`), a CLI beta.17 option.
    pub headless: bool,
    /// False on platforms without a headless build (Linux only).
    pub headless_available: bool,
    pub accept_eula: bool,
    pub can_install: bool,
    /// Set when the requested version already has a build installed and the
    /// user is asking for another one. Both stay on disk; projects keep the
    /// build they were created with.
    pub second_build_warning: Option<String>,
    pub preview: Option<InstallPreviewVm>,
    pub preview_note: Option<String>,
    pub preview_error: Option<UiError>,
    pub preview_loading: bool,
    /// The collapsed Advanced area. All flags are off by default.
    pub advanced: InstallAdvancedVm,
    /// The exact `unity install` command the buttons would run.
    pub command_preview: String,
    /// Present after a failed/interrupted install when the CLI reported a
    /// cache/interrupted state (or the user asked for `--resume`).
    pub resume_offer: Option<ResumeInstallVm>,
    /// Set after a successful `--download-only` run: where the files landed.
    pub download_report: Option<String>,
}

/// Advanced `unity install` fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallAdvancedVm {
    pub expanded: bool,
    pub resume: bool,
    pub download_only: bool,
    pub no_wait: bool,
    pub timeout_input: String,
    pub set_default: bool,
    pub retries_input: String,
    /// Explains a cleared conflicting option or a refused combination.
    pub conflict_note: Option<String>,
    pub timeout_error: Option<String>,
    pub retries_error: Option<String>,
    pub can_install: bool,
}

/// A resume action offered after an interrupted or failed download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeInstallVm {
    pub version: String,
    /// Empty is the host default.
    pub architecture: String,
    pub reason: String,
    pub can_resume: bool,
}

/// One choice in the install card's architecture picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureOptionVm {
    /// `None` is the host default: the CLI resolves the build and no
    /// `--architecture` token is sent.
    pub architecture: Option<String>,
    pub label: String,
    pub selected: bool,
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
    /// The build the CLI resolved, when it reports one.
    pub architecture: Option<String>,
    pub download_size: String,
    pub module_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRowVm {
    pub version: String,
    pub architecture: String,
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
    /// The build the project was created with; older CLI rows can omit it.
    pub architecture: String,
    pub pipeline: Option<BadgeVm>,
    pub build_target: String,
    pub short_path: String,
    pub modified: String,
    pub badges: Vec<BadgeVm>,
}

// ---------------------------------------------------------------------------
// Diagnostics

/// Read-only machine diagnostics, one independent read per section.
///
/// No section gates another: the page renders each `LoadState` on its own, so a
/// slow `unity diagnose accelerator` never hides the doctor checks that already
/// arrived. Every failed section carries its own Retry action.
#[derive(Debug, Clone)]
pub struct DiagnosticsVm {
    pub doctor: LoadState<DoctorVm>,
    pub environment: LoadState<EnvironmentVm>,
    pub proxy_report: LoadState<DiagnoseReportVm>,
    pub accelerator_report: LoadState<DiagnoseReportVm>,
    pub update_report: LoadState<DiagnoseReportVm>,
    /// True when at least one of doctor, environment, or proxy is loaded, so
    /// the combined Copy action has something to write.
    pub can_copy: bool,
}

/// `unity doctor --format json`, mapped for display.
#[derive(Debug, Clone)]
pub struct DoctorVm {
    pub platform: String,
    pub arch: String,
    pub runtime_version: String,
    pub node_version: String,
    pub cli_version: String,
    pub install_path: String,
    pub log_directory: String,
    pub checks: Vec<DoctorCheckVm>,
    /// A short summary of the CLI's auth section, when it reports one.
    pub auth: Option<String>,
    /// A short summary of the CLI's license section, when it reports one.
    pub license: Option<String>,
    /// The raw `data` document, pretty-printed. Report sections this build does
    /// not model are never hidden from the user.
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheckVm {
    pub id: String,
    /// Humanized from `id`/`params`, with `messageKey` as the fallback.
    pub label: String,
    pub status: CheckStatus,
    /// The `params` values the CLI would interpolate, rendered as key/value
    /// pairs so a path or version is visible without guessing a template.
    pub detail: Option<String>,
}

/// The four documented doctor statuses plus an explicit unknown state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Info,
    Unknown,
}

/// `unity env --format json`, mapped for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentVm {
    pub user_data_path: String,
    pub editor_install_path: String,
    pub download_cache_path: String,
    pub config_path: String,
    pub cli_version: String,
    pub hub_version: String,
    pub proxy: Option<ProxyVm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyVm {
    pub url: String,
    pub source: String,
    pub auth_source: String,
    pub no_proxy: Vec<String>,
}

/// One of the CLI's redacted `unity diagnose` reports, preformatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnoseReportVm {
    pub topic: DiagnoseTopic,
    pub text: String,
}

/// The fixed diagnose topics. None of them accepts a user-supplied value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnoseTopic {
    Proxy,
    Accelerator,
    Update,
}

impl DiagnoseTopic {
    pub fn label(self) -> &'static str {
        match self {
            Self::Proxy => "Proxy",
            Self::Accelerator => "Accelerator",
            Self::Update => "Update",
        }
    }
}

// ---------------------------------------------------------------------------
// Changelog

#[derive(Debug, Clone)]
pub struct ChangelogVm {
    pub loaded: bool,
    pub versions: Vec<String>,
    pub selected_target: Option<String>,
    pub show_raw: bool,
    /// The app-impact filter, off by default. When on, section cards show only
    /// entries whose `app_impact` is true.
    pub app_impact_only: bool,
    /// The advanced docs scope picker. `Automatic` uses the entry's own
    /// preference (CLI docs for command candidates, script reference for
    /// class-like ones).
    pub docs_scope: DocsScopeVm,
    /// The package name for `DocsScopeVm::Package`; empty until typed.
    pub docs_package: String,
    /// A quiet explanation when the package scope cannot be used yet.
    pub docs_scope_note: Option<String>,
    pub state: LoadState<ChangelogDataVm>,
    pub raw: LoadState<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogDataVm {
    pub version: String,
    pub source: String,
    pub source_detail: String,
    pub entry_count: usize,
    /// Number of entries the filter keeps. It is computed from the same
    /// signals the section cards filter on, so the toggle label and the
    /// visible list cannot disagree.
    pub app_impact_count: usize,
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
    pub entries: Vec<ChangelogEntryVm>,
}

/// One release-note entry in the view contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogEntryVm {
    pub text: String,
    /// Mentions a command the adapter runs, a flag it passes, or an error/exit
    /// code it maps.
    pub app_impact: bool,
    pub docs: EntryDocsVm,
}

/// Per-entry docs status. A lookup never blocks the entry: every state keeps
/// the text visible and the failure states are quiet inline lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryDocsVm {
    /// No backticked candidate, or the selected scope needs a package name.
    Unavailable,
    /// A candidate exists and the lookup has not started.
    Idle {
        topic: String,
        scope: DocsScopeVm,
    },
    Loading {
        topic: String,
    },
    /// A resolved URL. `search` means the CLI fell back to a search page for an
    /// ambiguous name.
    Ready {
        topic: String,
        url: String,
        search: bool,
    },
    /// The CLI answered successfully but resolved no page.
    Missing {
        topic: String,
    },
    /// The lookup failed. `message` is the CLI's own message, kept short.
    Failed {
        topic: String,
        message: String,
    },
}

/// The advanced docs collection picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocsScopeVm {
    /// Prefer CLI docs for command candidates and the script reference for
    /// class-like ones.
    Automatic,
    ScriptReference,
    Manual,
    Cli,
    Hub,
    Cloud,
    Package,
}

impl DocsScopeVm {
    pub const ALL: [DocsScopeVm; 7] = [
        DocsScopeVm::Automatic,
        DocsScopeVm::ScriptReference,
        DocsScopeVm::Manual,
        DocsScopeVm::Cli,
        DocsScopeVm::Hub,
        DocsScopeVm::Cloud,
        DocsScopeVm::Package,
    ];
}

impl fmt::Display for DocsScopeVm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Automatic => "Automatic (per entry)",
            Self::ScriptReference => "Script reference",
            Self::Manual => "Manual",
            Self::Cli => "CLI reference",
            Self::Hub => "Hub",
            Self::Cloud => "Cloud",
            Self::Package => "Unity package",
        })
    }
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
    /// The build the modules belong to; two builds can share a version.
    pub architecture: String,
    pub installed_modules: Vec<String>,
    pub modules: LoadState<Vec<ModuleOptionVm>>,
    pub selected_count: usize,
    pub accept_eula: bool,
    pub can_install: bool,
    /// `install-modules --retries <n>`; empty leaves the CLI default.
    pub retries_input: String,
    /// `install-modules -d/--download-directory`; empty means the registered
    /// install, a path targets a portable install.
    pub download_directory: String,
    pub retries_error: Option<String>,
    /// The exact `unity install-modules` command the button would run.
    pub command_preview: String,
}

#[derive(Debug, Clone)]
pub struct UpgradeProjectVm {
    pub project_title: String,
    pub project_path_short: String,
    pub current_version: String,
    /// The project's build, when the CLI reports one.
    pub current_architecture: String,
    /// One entry per installed version. The CLI's upgrade command targets a
    /// version, so two builds of a version cannot be offered separately.
    pub target_options: Vec<String>,
    /// Explains a target version that has more than one installed build.
    pub target_note: Option<String>,
    pub selected_target: Option<String>,
    pub confirmed: bool,
    pub can_upgrade: bool,
}

/// Review-and-confirm dialog for `unity editors prune`.
///
/// The report is always shown before a destructive button becomes available.
/// `--remove` and `--remove-missing` are two independent actions with separate
/// confirmations; the second one never deletes files.
#[derive(Debug, Clone)]
pub struct PruneEditorsVm {
    pub report: LoadState<PruneReportVm>,
    pub confirmed: bool,
    pub missing_confirmed: bool,
    /// Only meaningful when `force_available` is true.
    pub force: bool,
    /// True after a remove run failed because the running-Editor check could
    /// not complete. `--force` is never offered before that.
    pub force_available: bool,
    /// The exact report command, shown so the user sees what ran.
    pub scan_command: String,
    /// The exact commands the two buttons would run.
    pub remove_command: String,
    pub missing_command: String,
    /// Names the count and total that the removal button acts on.
    pub remove_label: String,
    /// Names the registry-only action and says no files are deleted.
    pub missing_label: String,
    /// Enumerates every build the remove confirmation would uninstall.
    pub confirm_label: String,
    /// Enumerates every registry entry the missing confirmation would drop.
    pub missing_confirm_label: String,
    pub can_remove: bool,
    pub can_remove_missing: bool,
    /// A failed remove run, kept beside the candidates so `--force` has context.
    pub error: Option<UiError>,
    pub job_running: bool,
}

/// `unity self-update --rollback [version]` confirmation.
///
/// The dialog never carries a target, channel, changelog, check or dry-run
/// flag: beta.17 refuses that combination with `BAD_ARGS`.
#[derive(Debug, Clone)]
pub struct RollbackCliVm {
    /// Empty restores the most recent kept version.
    pub version_input: String,
    /// Names exactly what is restored (`Unity CLI 1.0.0-beta.16` or
    /// "the most recent kept version").
    pub restore_label: String,
    pub confirmed: bool,
    pub can_rollback: bool,
    pub command: String,
    pub error: Option<UiError>,
    pub job_running: bool,
}

/// The verify dialog: one row per checked file/module, plus the raw log when
/// the CLI's own verdict is a failure.
#[derive(Debug, Clone)]
pub struct VerifyEditorVm {
    pub version: String,
    /// The build the row named; empty when the registry did not report one.
    pub architecture: String,
    /// The exact `unity editors verify` command this dialog ran.
    pub command: String,
    pub state: LoadState<VerifyResultVm>,
    pub show_raw: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyResultVm {
    pub version: String,
    pub architecture: String,
    /// The CLI's own verdict (`ok` in its report).
    pub ok: bool,
    pub checks: Vec<VerifyCheckVm>,
    pub failed_count: usize,
    /// The exact `unity install-modules` command the CLI named, if any.
    pub repair_command: Option<String>,
    /// The raw JSON envelope, exposed when the verify fails.
    pub raw: String,
}

/// One `unity editors verify` component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyCheckVm {
    /// `component` from the CLI, e.g. `editor` or `windows-mono`.
    pub component: String,
    /// `editor` or `module`, as the CLI labels it.
    pub kind: String,
    pub status: CheckStatus,
    /// The exact path checked. Shown for a failing row.
    pub path: Option<String>,
    /// A per-component repair command, when the CLI names one.
    pub repair_command: Option<String>,
}
