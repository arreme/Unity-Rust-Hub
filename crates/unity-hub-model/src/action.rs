use crate::{NewProjectField, Page, TemplateAuthoringMode, TemplateField, TemplateKindFilter};

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
    DismissJob,
    CancelJob,
    SignIn,
    TextScaleChanged(u16),
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
    OpenEditorModules(String),
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
