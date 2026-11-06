use crate::{DiagnoseTopic, ErrorActionVm, Page};

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
    DismissNotice,
    /// Dismisses one advisory banner. The id is the CLI advisory's stable
    /// identity, already scoped to the current CLI version by the controller.
    DismissAdvisory(String),
    /// Copies the CLI-named remediation command. Never runs it.
    CopyAdvisoryCommand(String),
    /// Runs the remediation command the user just clicked. The controller only
    /// honours this for advisories the adapter has an explicit builder for.
    RunAdvisoryRemediation(String),
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
    SignIn,
    TextScaleChanged(u16),
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
    OpenProject(String),
    OpenProjectUpgrade(String),
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
