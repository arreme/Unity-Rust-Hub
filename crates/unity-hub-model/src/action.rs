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
    UseRelease(String, String),
    /// Version and architecture identify one installed build, because the CLI
    /// can have an `arm64` and an `x86_64` Editor of the same version.
    OpenEditorModules(String, String),
    /// Stores this build as the default Editor. The architecture is required
    /// so a two-build version is never resolved by guesswork.
    SetDefaultEditor(String, String),
    /// Clears the stored default Editor.
    ClearDefaultEditor,
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
