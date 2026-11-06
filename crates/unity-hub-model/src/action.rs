use crate::Page;

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
    LoadModules,
    ToggleModule(String, bool),
    SelectAllModules(bool),
    HeadlessToggled(bool),
    AcceptEulaToggled(bool),
    PreviewInstall,
    StartInstallEditor,
    UseRelease(String),
    /// Version and architecture identify one installed build, because the CLI
    /// can have an `arm64` and an `x86_64` Editor of the same version.
    OpenEditorModules(String, String),
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
