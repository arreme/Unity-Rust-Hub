use crate::{
    DiagnoseTopic, DocsScopeVm, ErrorActionVm, NewProjectField, Page, TemplateAuthoringMode,
    TemplateField, TemplateKindFilter,
};

/// Everything a screen is allowed to ask the application to do.
///
/// Actions carry stable identifiers (an Editor version, a module id, a project
/// path) rather than indices into a snapshot, so a refresh between a click and
/// its handling cannot target the wrong row. Async results and job lifecycle
/// events stay inside the controller.
#[derive(Debug, Clone)]
pub enum UiAction {
    Navigate(Page),
    Refresh,
    /// Runs the primary action shown on a failed read or job. The controller
    /// resolves the target itself; the UI never retries by holding a closure.
    RunErrorAction(ErrorActionVm),
    /// Copies the combined diagnostics report (doctor + env + proxy) as plain
    /// text. Reads only; nothing on that page mutates the machine.
    CopyDiagnosticsReport,
    /// Runs `unity doctor` on demand. It is deliberately not part of the
    /// automatic Diagnostics load because its credential probe can trigger a
    /// polkit password prompt.
    LoadDoctor,
    /// Copies one redacted `unity diagnose` report.
    CopyDiagnoseReport(DiagnoseTopic),
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

    SignIn,
    TextScaleChanged(u16),
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
    /// Applies the update after the target changelog was shown.
    StartCliUpdate,
    /// Opens the confirmed `--rollback` dialog.
    OpenCliRollback,
    CliRollbackVersionChanged(String),
    CliRollbackConfirmed(bool),
    StartCliRollback,
    InstallVersionChanged(String),
    /// Selects the build to install. `None` is the host default and is left
    /// out of the command.
    InstallArchitectureChanged(Option<String>),
    LoadModules,
    ToggleModule(String, bool),
    SelectAllModules(bool),
    HeadlessToggled(bool),
    AcceptEulaToggled(bool),
    PreviewInstall,
    StartInstallEditor,
    /// Expands or collapses the install card's Advanced area.
    InstallAdvancedToggled(bool),
    InstallResumeToggled(bool),
    InstallDownloadOnlyToggled(bool),
    InstallNoWaitToggled(bool),
    InstallTimeoutChanged(String),
    InstallSetDefaultToggled(bool),
    InstallRetriesChanged(String),
    /// Re-runs the interrupted install with `--resume`, reusing its cache.
    ResumeInstallEditor,
    /// `install-modules --retries`.
    DialogRetriesChanged(String),
    /// `install-modules -d/--download-directory`.
    DialogDownloadDirectoryChanged(String),
    UseRelease(String, String),
    /// Version and architecture identify one installed build, because the CLI
    /// can have an `arm64` and an `x86_64` Editor of the same version.
    OpenEditorModules(String, String),
    /// Stores this build as the default Editor. The architecture is required
    /// so a two-build version is never resolved by guesswork.
    SetDefaultEditor(String, String),
    /// Clears the stored default Editor.
    ClearDefaultEditor,
    /// Opens the prune review dialog and starts the report-only scan.
    OpenPrune,
    /// Re-runs `unity editors prune --remove-missing` (still removes nothing).
    PruneScan,
    /// The user's explicit confirmation for deleting the unused candidates.
    PruneConfirmed(bool),
    /// The user's explicit confirmation for dropping missing registry entries.
    PruneMissingConfirmed(bool),
    /// Opts into `--force` after a failed running-Editor check.
    PruneForceToggled(bool),
    /// Runs the streamed `--remove` job. Ignored without `PruneConfirmed(true)`.
    StartPruneRemove,
    /// Runs the streamed `--remove-missing` job. Changes the registry only.
    StartPruneRemoveMissing,
    /// Opens the verify dialog and reads one build's structural check.
    OpenVerify(String, String),
    /// Expands or collapses the raw log of a failed verify.
    ToggleVerifyRaw,
    CloseModal,
    ToggleDialogModule(String, bool),
    DialogEulaToggled(bool),
    StartInstallModules,
    ProjectSearchChanged(String),
    AddProject,
    OpenProjectRemove(String),
    ConfirmProjectRemove,
    ToggleProjectFavorite(String, bool),
    OpenProject(String),
    OpenProjectUpgrade(String),
    UpgradeTargetChanged(String),
    UpgradeConfirmed(bool),
    StartUpgrade,
    /// Installs the Unity agent skill to one client (the stable client id).
    /// The current install options apply; the client is a token, never a row
    /// index.
    InstallSkill(String),
    /// Installs the skill to every supported client (`--all`).
    InstallAllSkills,
    /// `--local`: write into the current project. Ignored by the controller
    /// when no project context is known.
    SkillLocalToggled(bool),
    /// `--yes`: replace an outdated copy instead of letting the CLI skip it.
    SkillOverwriteToggled(bool),
    /// `--dry-run` for an install.
    SkillDryRunToggled(bool),
    /// Types the optional `--skill <name>` filter.
    SkillNameChanged(String),
    /// Runs the next refresh. The first click after opening the page is a
    /// dry run; later clicks apply.
    StartSkillRefresh,
    /// Runs an explicit `--dry-run` refresh, even after a real refresh.
    PreviewSkillRefresh,
    /// Re-runs the last skill install/refresh with `--force` after a conflict
    /// row explained that the CLI did not write that copy.
    ForceSkillLastRun,
    ChangelogTargetChanged(String),
    LoadChangelog,
    ToggleRawChangelog,
    /// Filters the release notes to entries touching this app's CLI surface.
    ToggleChangelogImpact(bool),
    /// Changes the advanced docs collection for entry lookups.
    ChangelogDocsScopeChanged(DocsScopeVm),
    /// Types the package name used by the package docs scope.
    ChangelogDocsPackageChanged(String),
    /// Resolves one entry's docs topic with `unity docs --url`.
    LookupChangelogDocs {
        topic: String,
        scope: DocsScopeVm,
    },
    /// Hands a resolved docs URL to the OS opener. The UI only emits this on
    /// a click; the app never launches a browser from a lookup.
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

    // New Project wizard
    OpenNewProject,
    NewProjectFieldChanged(NewProjectField, String),
    NewProjectBrowseParent,
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
}
