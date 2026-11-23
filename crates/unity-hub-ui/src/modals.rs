use iced::widget::{column, container, pick_list, row, space};
use iced::{Center, Element, Fill};

use unity_hub_model::{
    CheckStatus, EditorModulesVm, InstallCliVm, JobPhase, JobVm, LoadState, ModalVm,
    ModuleOptionVm, NewProjectPreflightState, NewProjectStep, NewProjectVm, Page, PruneCandidateVm,
    PruneEditorsVm, PruneReportVm, RemoveProjectVm, RollbackCliVm, TemplateAuthoringVm,
    TemplateField, TemplateRowVm, Tone, UiAction, UpgradeProjectVm, VerifyCheckVm, VerifyEditorVm,
    VerifyResultVm,
};

use crate::components::{
    badge, button, caption, checkbox, detail_column, divided, divider, error_text, field_label,
    hint, icon_button, loading, mono, notice, scroll, text_input, ButtonKind, PICK_LIST_PADDING,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::font;
use crate::ui::{self, text};

const DIALOG_WIDTH: f32 = 560.0;
const WIDE_DIALOG_WIDTH: f32 = 760.0;

pub fn view(modal: &ModalVm) -> Element<'_, UiAction> {
    let card = match modal {
        ModalVm::InstallCli(vm) => install_cli_modal(vm),
        ModalVm::EditorModules(vm) => editor_modules_modal(vm),
        ModalVm::UpgradeProject(vm) => upgrade_modal(vm),
        ModalVm::RemoveProject(vm) => remove_project_modal(vm),
        ModalVm::NewProject(vm) => new_project_modal(vm),
        ModalVm::TemplateAuthoring(vm) => template_authoring_modal(vm),
        ModalVm::PruneEditors(vm) => prune_modal(vm),
        ModalVm::VerifyEditor(vm) => verify_modal(vm),
        ModalVm::RollbackCli(vm) => rollback_cli_modal(vm),
    };

    container(iced::widget::center(card).padding(24))
        .width(Fill)
        .height(Fill)
        .style(theme::modal_screen)
        .into()
}

/// Shared dialog frame: title and subtitle with a round close button, the
/// body, and the actions right-aligned under a hairline.
fn dialog<'a>(
    title: impl Into<String>,
    subtitle: impl Into<String>,
    body: Element<'a, UiAction>,
    actions: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    dialog_sized(DIALOG_WIDTH, title, subtitle, body, actions)
}

fn dialog_sized<'a>(
    width: f32,
    title: impl Into<String>,
    subtitle: impl Into<String>,
    body: Element<'a, UiAction>,
    actions: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    let header = row![
        column![
            text(title.into())
                .size(font::MODAL_TITLE)
                .font(fonts::semibold()),
            text(subtitle.into())
                .size(font::BODY_LARGE)
                .style(theme::muted_text),
        ]
        .spacing(4)
        .width(Fill),
        iced::widget::button(
            container(icon(icons::X, font::ICON_SMALL))
                .width(20)
                .height(20)
                .center(20)
        )
        .on_press(UiAction::CloseModal)
        .padding(8)
        .style(theme::button_round),
    ]
    .spacing(16)
    .align_y(iced::Alignment::Start);

    container(
        column![
            header,
            body,
            divider(),
            row![space().width(Fill), actions].align_y(Center),
        ]
        .spacing(22),
    )
    .width(width)
    .padding(28)
    .style(theme::modal)
    .into()
}

fn labelled<'a>(label: &'a str, control: Element<'a, UiAction>) -> Element<'a, UiAction> {
    column![field_label(label), control].spacing(8).into()
}

fn styled_pick_list<'a>(
    options: Vec<String>,
    selected: Option<String>,
    on_select: impl Fn(String) -> UiAction + 'a,
    placeholder: &str,
) -> Element<'a, UiAction> {
    pick_list(options, selected, on_select)
        .text_size(ui::font_size(font::BODY))
        .font(fonts::regular())
        .padding(PICK_LIST_PADDING)
        .handle(icons::pick_list_handle(font::ICON_SMALL))
        .style(theme::pick_list)
        .menu_style(theme::menu)
        .width(Fill)
        .placeholder(placeholder.to_string())
        .into()
}

fn install_cli_modal(vm: &InstallCliVm) -> Element<'_, UiAction> {
    let body = column![
        labelled(
            "Release channel",
            styled_pick_list(
                vm.channels.clone(),
                Some(vm.selected_channel.clone()),
                UiAction::CliChannelChanged,
                "Select a channel",
            ),
        ),
        container(
            column![
                caption("Official installer command"),
                text(vm.command_preview.as_str())
                    .size(font::MONO)
                    .font(fonts::mono())
                    .line_height(1.55),
                hint("UNITY_CLI_SKIP_SETUP=1 is set so the installer does not block on its interactive setup."),
            ]
            .spacing(8),
        )
        .padding([14.0, 16.0])
        .width(Fill)
        .style(theme::raised),
        checkbox(
            "I understand this downloads and executes Unity's installer script from unity.com",
            vm.consent,
            UiAction::CliConsentToggled,
        ),
    ]
    .spacing(20);

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        icon_button(icons::DOWNLOAD, "Run installer", ButtonKind::Primary)
            .on_press_maybe(vm.can_install.then_some(UiAction::InstallCli)),
    ]
    .spacing(8);

    dialog(
        "Install the Unity CLI",
        "Unity's official installer runs locally. This app does not bundle the CLI.",
        body.into(),
        actions.into(),
    )
}

fn editor_modules_modal(vm: &EditorModulesVm) -> Element<'_, UiAction> {
    let installed: Element<'_, UiAction> = if vm.installed_modules.is_empty() {
        hint("No modules reported.")
    } else {
        text(vm.installed_modules.join(" · "))
            .size(font::BODY)
            .style(theme::muted_text)
            .into()
    };

    let module_list: Element<'_, UiAction> = if let Some(error) = vm.modules.error() {
        error_text(error)
    } else if let Some(modules) = vm.modules.data() {
        let list = divided(
            modules
                .iter()
                .map(|module| {
                    container(module_checkbox(module))
                        .padding([13.0, 16.0])
                        .width(Fill)
                        .into()
                })
                .collect(),
        );
        container(container(scroll(list)).max_height(280))
            .width(Fill)
            .style(theme::outline)
            .into()
    } else if vm.modules.is_loading() {
        loading("Loading modules…")
    } else {
        hint("Load modules for this Editor to add them.")
    };

    let mut body_column = column![
        labelled("Installed", installed),
        labelled("Available", module_list),
        checkbox(
            "I accept the module license agreements (--accept-eula)",
            vm.accept_eula,
            UiAction::DialogEulaToggled,
        ),
        row![
            container(
                column![
                    field_label("Download retries (0 disables)"),
                    text_input("2", &vm.retries_input)
                        .on_input(UiAction::DialogRetriesChanged),
                ]
                .spacing(6)
            )
            .width(Fill),
            container(
                column![
                    field_label("Portable download directory (optional)"),
                    text_input("./.unity", &vm.download_directory)
                        .on_input(UiAction::DialogDownloadDirectoryChanged),
                ]
                .spacing(6)
            )
            .width(Fill),
        ]
        .spacing(16),
        hint("Retries passed as --retries; 0 disables. A directory targets a portable install and is passed as --download-directory."),
    ]
    .spacing(20);

    if let Some(error) = &vm.retries_error {
        body_column = body_column.push(hint(error.as_str()));
    }

    body_column = body_column.push(hint("Exact command"));
    body_column = body_column.push(mono(vm.command_preview.as_str()));

    let body = body_column;

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        icon_button(
            icons::DOWNLOAD,
            format!("Install {} module(s)", vm.selected_count),
            ButtonKind::Primary,
        )
        .on_press_maybe(vm.can_install.then_some(UiAction::StartInstallModules)),
    ]
    .spacing(8);

    let subtitle = if vm.architecture.trim().is_empty() {
        "Only modules that are not already installed are listed.".to_string()
    } else {
        format!(
            "{} build · Only modules that are not already installed are listed.",
            vm.architecture
        )
    };

    dialog(
        format!("Modules for Unity {}", vm.editor_version),
        subtitle,
        body.into(),
        actions.into(),
    )
}

fn module_checkbox(module: &ModuleOptionVm) -> Element<'static, UiAction> {
    let id = module.id.clone();
    checkbox(module.label.clone(), module.selected, move |value| {
        UiAction::ToggleDialogModule(id.clone(), value)
    })
    .width(Fill)
    .into()
}

fn version_cell<'a>(
    label: &'a str,
    version: String,
    architecture: Option<String>,
    end: bool,
) -> Element<'a, UiAction> {
    let mut content = column![
        caption(label),
        text(version)
            .size(font::BODY_LARGE + 1.0)
            .font(fonts::mono()),
    ]
    .spacing(4);
    if let Some(architecture) = architecture.filter(|value| !value.trim().is_empty()) {
        content = content.push(
            text(architecture)
                .size(font::SMALL)
                .font(fonts::mono())
                .style(theme::muted_text),
        );
    }

    if end {
        container(content.align_x(iced::Alignment::End))
            .width(Fill)
            .align_right(Fill)
            .into()
    } else {
        container(content).width(Fill).into()
    }
}

fn upgrade_modal(vm: &UpgradeProjectVm) -> Element<'_, UiAction> {
    let picker: Element<'_, UiAction> = if vm.target_options.is_empty() {
        notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            text("No other Editor version is installed. Install one on the Editors page first.")
                .size(font::BODY)
                .style(theme::muted_text),
        )
    } else {
        styled_pick_list(
            vm.target_options.clone(),
            vm.selected_target.clone(),
            UiAction::UpgradeTargetChanged,
            "Select an installed Editor",
        )
    };

    let target_note: Element<'_, UiAction> = match &vm.target_note {
        Some(note) => notice(
            Tone::Info,
            icons::INFO,
            text(note.as_str())
                .size(font::BODY)
                .line_height(1.55)
                .style(theme::muted_text),
        ),
        None => space().height(0).into(),
    };

    let strip = container(
        row![
            version_cell(
                "Current",
                vm.current_version.clone(),
                Some(vm.current_architecture.clone()),
                false,
            ),
            icon(icons::ARROW_RIGHT, font::ICON).style(theme::faint_text),
            version_cell(
                "Target",
                vm.selected_target
                    .clone()
                    .unwrap_or_else(|| "—".to_string()),
                None,
                true,
            ),
        ]
        .spacing(16)
        .align_y(Center),
    )
    .padding([18.0, 20.0])
    .width(Fill)
    .style(theme::raised);

    let body = column![
        strip,
        column![
            labelled("Target Editor version", picker),
            target_note,
            mono(vm.project_path_short.as_str()),
        ]
        .spacing(8),
        notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            text("Runs `unity projects upgrade <path> --to <version> --yes`. It can update ProjectVersion.txt, open the Editor, and run API upgrades. Close the project in any running Editor and commit or back up first.")
                .size(font::BODY)
                .line_height(1.55)
                .style(theme::muted_text),
        ),
        checkbox(
            "I understand and want to upgrade this project now",
            vm.confirmed,
            UiAction::UpgradeConfirmed,
        ),
    ]
    .spacing(20);

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        button("Upgrade project", ButtonKind::Primary)
            .on_press_maybe(vm.can_upgrade.then_some(UiAction::StartUpgrade)),
    ]
    .spacing(8);

    dialog(
        "Change Editor version",
        vm.project_title.as_str(),
        body.into(),
        actions.into(),
    )
}

fn remove_project_modal(vm: &RemoveProjectVm) -> Element<'_, UiAction> {
    let body = column![
        mono(vm.project_path_short.as_str()),
        notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            text("This removes the project from the Unity Hub registry. Files on disk are not deleted.")
                .size(font::BODY)
                .line_height(1.55)
                .style(theme::muted_text),
        ),
    ]
    .spacing(20);

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        button("Remove project", ButtonKind::Danger).on_press(UiAction::ConfirmProjectRemove),
    ]
    .spacing(8);

    dialog(
        "Remove project",
        vm.project_title.as_str(),
        body.into(),
        actions.into(),
    )
}

/// Review-and-confirm dialog for `unity editors prune`.
///
/// The candidate list is always drawn before either destructive action. The
/// two actions are deliberately separate: one deletes Editor files, the other
/// only edits the registry.
fn prune_modal(vm: &PruneEditorsVm) -> Element<'_, UiAction> {
    let mut body = column![
        mono(vm.scan_command.as_str()),
        hint("Report only. Scanning removes nothing; every removal needs the confirmation below."),
    ]
    .spacing(16);

    if vm.job_running {
        body = body.push(notice(
            Tone::Info,
            icons::LOADER,
            column![
                text("Removing… the CLI is still running.")
                    .size(font::BODY)
                    .font(fonts::medium()),
                hint("The raw NDJSON log and Cancel are on the job card. Close this dialog to reach them; closing does not stop the job."),
            ]
            .spacing(6),
        ));
    }

    body = body.push(prune_report_body(&vm.report));

    if let Some(error) = &vm.error {
        body = body.push(error_text(error));
    }

    let ready = match &vm.report {
        LoadState::Ready(report) => Some(report),
        _ => None,
    };

    if ready.is_some_and(|report| !report.unused.is_empty()) {
        body = body.push(checkbox(
            vm.confirm_label.clone(),
            vm.confirmed,
            UiAction::PruneConfirmed,
        ));
        if vm.force_available {
            body = body.push(checkbox(
                "The running-Editor check could not complete. Remove anyway (--force).",
                vm.force,
                UiAction::PruneForceToggled,
            ));
        }
        body = body.push(hint(format!(
            "Runs `{}`. The job streams its raw log to the job card.",
            vm.remove_command
        )));
    }

    if let Some(report) = ready.filter(|report| !report.missing.is_empty()) {
        body = body.push(missing_section(vm, report));
    }

    let mut actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        button("Refresh report", ButtonKind::Secondary).on_press(UiAction::PruneScan),
    ]
    .spacing(8);
    if ready.is_some_and(|report| !report.unused.is_empty()) {
        actions = actions.push(
            icon_button(icons::TRASH, vm.remove_label.clone(), ButtonKind::Danger)
                .on_press_maybe(vm.can_remove.then_some(UiAction::StartPruneRemove)),
        );
    }

    dialog_sized(
        WIDE_DIALOG_WIDTH,
        "Review unused Editors",
        "The CLI reports which installed builds no registered project uses. Nothing runs until you confirm.",
        body.into(),
        actions.into(),
    )
}

fn prune_report_body(report: &LoadState<PruneReportVm>) -> Element<'_, UiAction> {
    match report {
        LoadState::Idle => hint("Scan installed Editors to see prune candidates."),
        LoadState::Loading { previous } => match previous {
            Some(report) => column![
                loading("Rescanning installed Editors…"),
                prune_report_ready(report)
            ]
            .spacing(12)
            .into(),
            None => loading("Scanning installed Editors…"),
        },
        LoadState::Failed { error, .. } => error_text(error),
        LoadState::Ready(report) => prune_report_ready(report),
    }
}

fn prune_report_ready(report: &PruneReportVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(12);

    if report.unused.is_empty() {
        body = body.push(hint("No unused Editors were reported."));
    } else {
        body = body.push(caption(format!(
            "Unused · {} candidate{}",
            report.unused.len(),
            if report.unused.len() == 1 { "" } else { "s" }
        )));
        body = body.push(
            container(scroll(divided(
                report
                    .unused
                    .iter()
                    .map(prune_candidate_row)
                    .collect::<Vec<_>>(),
            )))
            .max_height(240)
            .width(Fill)
            .style(theme::outline),
        );
        body = body.push(
            row![
                detail_column("Reclaimable", report.reclaimable.as_str()),
                detail_column("Removal", "Deletes the Editor folders"),
            ]
            .spacing(20),
        );
    }

    if !report.missing.is_empty() {
        body = body.push(hint(format!(
            "{} registry entr{} also point at folders that no longer exist; see the separate action below.",
            report.missing.len(),
            if report.missing.len() == 1 {
                "y"
            } else {
                "ies"
            }
        )));
    }

    body.into()
}

fn prune_candidate_row(candidate: &PruneCandidateVm) -> Element<'_, UiAction> {
    let architecture = if candidate.architecture.trim().is_empty() {
        "host build".to_string()
    } else {
        candidate.architecture.clone()
    };

    container(
        column![
            row![
                text(candidate.version.as_str())
                    .size(font::BODY_LARGE)
                    .font(fonts::mono()),
                badge(architecture, Tone::Neutral),
                container(
                    text(candidate.size.as_deref().unwrap_or("size not reported"))
                        .size(font::SMALL)
                        .style(theme::muted_text)
                )
                .width(Fill)
                .align_right(Fill),
            ]
            .spacing(10)
            .align_y(Center),
            hint(candidate.reason.as_str()),
            mono(candidate.path.as_str()),
        ]
        .spacing(4),
    )
    .padding([10.0, 0.0])
    .width(Fill)
    .into()
}

/// The registry-only action sits in its own outlined block so it cannot be
/// mistaken for the file-deleting removal above.
fn missing_section<'a>(vm: &'a PruneEditorsVm, report: &'a PruneReportVm) -> Element<'a, UiAction> {
    let list = divided(
        report
            .missing
            .iter()
            .map(prune_candidate_row)
            .collect::<Vec<_>>(),
    );

    container(
        column![
            caption(format!(
                "Missing install paths · {} entr{}",
                report.missing.len(),
                if report.missing.len() == 1 { "y" } else { "ies" }
            )),
            notice(
                Tone::Info,
                icons::INFO,
                text("Removing these entries changes the registry only. No files are deleted: the folders are already gone.")
                    .size(font::BODY)
                    .line_height(1.5)
                    .style(theme::muted_text),
            ),
            list,
            checkbox(
                vm.missing_confirm_label.clone(),
                vm.missing_confirmed,
                UiAction::PruneMissingConfirmed,
            ),
            row![
                button(vm.missing_label.clone(), ButtonKind::Secondary)
                    .on_press_maybe(
                        vm.can_remove_missing
                            .then_some(UiAction::StartPruneRemoveMissing)
                    ),
                hint(format!("Runs `{}`.", vm.missing_command)),
            ]
            .spacing(12)
            .align_y(Center),
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Fill)
    .style(theme::outline)
    .into()
}

fn verify_modal(vm: &VerifyEditorVm) -> Element<'_, UiAction> {
    let build = if vm.architecture.trim().is_empty() {
        "host build".to_string()
    } else {
        vm.architecture.clone()
    };
    let subtitle = format!("Structural check of Unity {} · {build}", vm.version);

    let body = column![
        mono(vm.command.as_str()),
        verify_state(&vm.state, vm.show_raw)
    ]
    .spacing(16);

    let failed = vm.state.data().map(|result| !result.ok).unwrap_or(false);
    let mut actions = row![
        button("Close", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        button("Verify again", ButtonKind::Secondary).on_press(UiAction::OpenVerify(
            vm.version.clone(),
            vm.architecture.clone(),
        )),
    ]
    .spacing(8);
    if failed {
        actions = actions.push(
            button(
                if vm.show_raw {
                    "Hide raw log"
                } else {
                    "Show raw log"
                },
                ButtonKind::Ghost,
            )
            .on_press(UiAction::ToggleVerifyRaw),
        );
    }

    dialog_sized(
        WIDE_DIALOG_WIDTH,
        format!("Verify Unity {}", vm.version),
        subtitle,
        body.into(),
        actions.into(),
    )
}

// ---------------------------------------------------------------------------
// New Project wizard

const WIZARD_WIDTH: f32 = 760.0;

fn steps(active: NewProjectStep) -> Element<'static, UiAction> {
    let mut row = row![].spacing(8).align_y(Center);
    for (index, step) in [
        NewProjectStep::Details,
        NewProjectStep::Editor,
        NewProjectStep::Template,
        NewProjectStep::Review,
    ]
    .into_iter()
    .enumerate()
    {
        if index > 0 {
            row = row.push(icon(icons::CHEVRON_RIGHT, font::ICON_SMALL).style(theme::faint_text));
        }
        let selected = step.index() <= active.index();
        let label: Element<'static, UiAction> = text(format!("{}. {}", index + 1, step.label()))
            .size(font::SMALL)
            .style(if selected {
                theme::primary_text
            } else {
                theme::faint_text
            })
            .into();
        row = row.push(label);
    }
    row.into()
}

fn new_project_modal(vm: &NewProjectVm) -> Element<'_, UiAction> {
    let body: Element<'_, UiAction> = match vm.step {
        NewProjectStep::Details => details_step(vm),
        NewProjectStep::Editor => editor_step(vm),
        NewProjectStep::Template => template_step(vm),
        NewProjectStep::Review => review_step(vm),
        NewProjectStep::Result => result_step(vm),
    };

    let mut actions = row![].spacing(8);
    if vm.step != NewProjectStep::Details && vm.step != NewProjectStep::Result {
        actions =
            actions.push(button("Back", ButtonKind::Ghost).on_press(UiAction::NewProjectBack));
    }
    if vm.step == NewProjectStep::Result {
        if let Some(result) = &vm.result {
            if result.success {
                if let Some(path) = &result.path {
                    actions = actions.push(
                        icon_button(icons::PLAY, "Open project", ButtonKind::Primary)
                            .on_press(UiAction::OpenProject(path.clone())),
                    );
                }
            }
        }
        actions =
            actions.push(button("Close", ButtonKind::Secondary).on_press(UiAction::CloseModal));
    } else {
        actions = actions.push(button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal));
        if vm.step == NewProjectStep::Review {
            actions = actions.push(
                icon_button(icons::PLUS, "Create project", ButtonKind::Primary).on_press_maybe(
                    (vm.preflight.can_continue && vm.issues.is_empty())
                        .then_some(UiAction::StartNewProject),
                ),
            );
        } else {
            actions = actions
                .push(button("Continue", ButtonKind::Primary).on_press(UiAction::NewProjectNext));
        }
    }

    wizard_dialog_actual(
        "New project",
        vm.step,
        vm.step.label().to_string(),
        body,
        actions.into(),
    )
}

fn wizard_dialog_actual<'a>(
    title: &'a str,
    active: NewProjectStep,
    subtitle: String,
    body: Element<'a, UiAction>,
    actions: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    dialog_frame(title, subtitle, steps(active), body, actions)
}

fn wide_dialog<'a>(
    title: &'a str,
    subtitle: String,
    body: Element<'a, UiAction>,
    actions: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    dialog_frame(title, subtitle, space().height(0).into(), body, actions)
}

fn dialog_frame<'a>(
    title: &'a str,
    subtitle: String,
    progress: Element<'a, UiAction>,
    body: Element<'a, UiAction>,
    actions: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    let header = row![
        column![
            text(title).size(font::MODAL_TITLE).font(fonts::semibold()),
            text(subtitle).size(font::BODY).style(theme::muted_text),
        ]
        .spacing(4)
        .width(Fill),
        iced::widget::button(
            container(icon(icons::X, font::ICON_SMALL))
                .width(20)
                .height(20)
                .center(20)
        )
        .on_press(UiAction::CloseModal)
        .padding(8)
        .style(theme::button_round),
    ]
    .spacing(16)
    .align_y(iced::Alignment::Start);

    container(
        column![
            header,
            progress,
            container(scroll(body))
                .max_height(440)
                .width(Fill)
                .padding([0.0, 2.0]),
            divider(),
            row![space().width(Fill), actions].align_y(Center),
        ]
        .spacing(18),
    )
    .width(WIZARD_WIDTH)
    .padding(28)
    .style(theme::modal)
    .into()
}

fn details_step(vm: &NewProjectVm) -> Element<'_, UiAction> {
    let parent_row = row![
        text_input("Choose a folder", vm.form.parent_dir.as_str())
            .on_input(|value| UiAction::NewProjectFieldChanged(
                unity_hub_model::NewProjectField::ParentDir,
                value
            ))
            .width(Fill),
        icon_button(icons::FOLDER_OPEN, "Browse", ButtonKind::Secondary)
            .on_press(UiAction::NewProjectBrowseParent),
    ]
    .spacing(10)
    .align_y(Center);

    column![
        labelled(
            "Project name",
            text_input("MyGame", vm.form.name.as_str())
                .on_input(|value| UiAction::NewProjectFieldChanged(
                    unity_hub_model::NewProjectField::Name,
                    value
                ))
                .into(),
        ),
        labelled("Parent folder", parent_row.into()),
        hint("The project is created in a new folder inside the parent folder."),
        issues(vm),
        preflight(vm),
    ]
    .spacing(20)
    .into()
}

fn editor_step(vm: &NewProjectVm) -> Element<'_, UiAction> {
    let editors = vm.editors.data().cloned().unwrap_or_default();
    let picker: Element<'_, UiAction> = if editors.is_empty() {
        notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            column![
                text("No Unity Editor is installed.")
                    .size(font::BODY_LARGE)
                    .font(fonts::medium()),
                text("Install an Editor first; this wizard only offers installed versions.")
                    .size(font::BODY)
                    .style(theme::muted_text),
                button("Open Editors", ButtonKind::Secondary)
                    .on_press(UiAction::Navigate(Page::Editors)),
            ]
            .spacing(10),
        )
    } else {
        styled_pick_list(
            editors
                .iter()
                .map(|editor| editor.version.clone())
                .collect(),
            Some(vm.form.editor_version.clone()),
            UiAction::NewProjectEditorChanged,
            "Select an installed Editor",
        )
    };

    let architectures = crate::components::segmented(vec![
        (
            "x86_64".to_string(),
            vm.form.architecture == "x86_64",
            UiAction::NewProjectArchitectureChanged("x86_64".to_string()),
        ),
        (
            "arm64".to_string(),
            vm.form.architecture == "arm64",
            UiAction::NewProjectArchitectureChanged("arm64".to_string()),
        ),
    ]);

    column![
        labelled("Editor version", picker),
        labelled("Architecture", architectures),
        row![
            hint(
                "The Editor's own architecture is selected by default. The CLI validates the pair."
            ),
            button("Manage modules", ButtonKind::Ghost).on_press(UiAction::Navigate(Page::Editors)),
        ]
        .spacing(12)
        .align_y(Center),
        issues(vm),
        preflight(vm),
    ]
    .spacing(20)
    .into()
}

fn template_step(vm: &NewProjectVm) -> Element<'_, UiAction> {
    let list: Element<'_, UiAction> = if let Some(error) = vm.templates.error() {
        error_text(error)
    } else if let Some(templates) = vm.templates.data() {
        if templates.is_empty() {
            notice(
                Tone::Warning,
                icons::TRIANGLE_ALERT,
                text("No templates are available for this Editor. Enable built-in templates or install the Editor's core templates."),
            )
        } else {
            container(scroll(
                column(
                    templates
                        .iter()
                        .map(|template| template_choice(template, vm))
                        .collect::<Vec<Element<'_, UiAction>>>(),
                )
                .spacing(6),
            ))
            .max_height(260)
            .width(Fill)
            .into()
        }
    } else {
        loading("Reading templates for this Editor…")
    };

    let selected: Element<'_, UiAction> = match &vm.selected_template {
        Some(detail) => column![
            row![
                text(detail.display_name.as_str())
                    .size(font::BODY_LARGE)
                    .font(fonts::medium()),
                badge(
                    detail
                        .render_pipeline
                        .clone()
                        .unwrap_or_else(|| "Pipeline not reported".to_string()),
                    Tone::Highlight,
                ),
            ]
            .spacing(8)
            .align_y(Center),
            text(detail.description.as_str())
                .size(font::BODY)
                .line_height(1.55)
                .style(theme::muted_text),
        ]
        .spacing(6)
        .into(),
        None => hint("Select a template to see its render pipeline and description."),
    };

    column![
        checkbox(
            "Show built-in templates (3D / 2D)",
            vm.form.show_builtin_templates,
            UiAction::NewProjectBuiltinToggled,
        ),
        list,
        selected,
        issues(vm),
    ]
    .spacing(18)
    .into()
}

fn template_choice<'a>(template: &'a TemplateRowVm, vm: &'a NewProjectVm) -> Element<'a, UiAction> {
    let selected = template.name == vm.form.template;
    let name = template.name.clone();
    let title = row![
        text(template.display_name.as_str())
            .size(font::BODY_LARGE)
            .font(if selected {
                fonts::semibold()
            } else {
                fonts::regular()
            }),
        if selected {
            badge("selected", Tone::Success)
        } else {
            badge("available", Tone::Neutral)
        },
    ]
    .spacing(8)
    .align_y(Center);

    let mut meta = row![mono(template.name.as_str())]
        .spacing(10)
        .align_y(Center);
    if let Some(pipeline) = &template.pipeline {
        meta = meta.push(badge(pipeline.clone(), Tone::Highlight));
    }

    iced::widget::button(
        container(column![title, meta].spacing(4))
            .padding([10.0, 12.0])
            .width(Fill)
            .style(if selected {
                theme::raised
            } else {
                theme::outline
            }),
    )
    .width(Fill)
    .padding(0)
    .style(theme::button_ghost)
    .on_press(UiAction::NewProjectTemplateChanged(name))
    .into()
}

fn review_step(vm: &NewProjectVm) -> Element<'_, UiAction> {
    let rows = vec![
        summary_row("Name", vm.form.name.as_str()),
        summary_row("Parent folder", vm.form.parent_dir.as_str()),
        summary_row("Editor", vm.form.editor_version.as_str()),
        summary_row("Architecture", vm.form.architecture.as_str()),
        summary_row("Template", vm.form.template.as_str()),
        summary_row(
            "Open after create",
            if vm.form.open_after_create {
                "Yes"
            } else {
                "No"
            },
        ),
    ];

    column![
        container(divided(rows)).style(theme::card),
        caption("Command preview"),
        container(
            text(vm.command_preview.as_str())
                .size(font::MONO)
                .font(fonts::mono())
                .line_height(1.5)
        )
        .padding([12.0, 14.0])
        .width(Fill)
        .style(theme::inset),
        checkbox(
            "Open the project when creation finishes",
            vm.form.open_after_create,
            UiAction::NewProjectOpenToggled,
        ),
        hint("The GUI always passes --no-cloud. Nothing runs until you choose Create project."),
        preflight(vm),
        issues(vm),
    ]
    .spacing(16)
    .into()
}

fn result_step(vm: &NewProjectVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(14);
    if let Some(job) = &vm.job {
        body = body.push(job_block(job));
    }
    if let Some(result) = &vm.result {
        let tone = if result.success {
            Tone::Success
        } else {
            Tone::Warning
        };
        body = body.push(notice(
            tone,
            if result.success {
                icons::CIRCLE_CHECK
            } else {
                icons::TRIANGLE_ALERT
            },
            text(result.summary.as_str())
                .size(font::BODY)
                .line_height(1.55),
        ));
        if let Some(path) = &result.path {
            body = body.push(
                row![
                    mono(path.as_str()),
                    icon_button(icons::COPY, "Copy path", ButtonKind::Ghost)
                        .on_press(UiAction::CopyText(path.clone()))
                        .padding(crate::components::COMPACT),
                ]
                .spacing(8)
                .align_y(Center),
            );
        }
        if !result.failed_steps.is_empty() {
            body = body.push(caption("Failed setup steps"));
            body = body.push(divided(
                result
                    .failed_steps
                    .iter()
                    .map(|step| {
                        text(step.as_str())
                            .size(font::BODY)
                            .style(theme::danger_text)
                            .into()
                    })
                    .collect(),
            ));
        }
    } else if vm.job.is_none() {
        body = body.push(hint("Waiting for the CLI result…"));
    }

    body.push(issues(vm)).into()
}

fn summary_row<'a>(label: &'a str, value: &'a str) -> Element<'a, UiAction> {
    crate::components::list_row(
        row![
            text(label)
                .size(font::SMALL)
                .style(theme::faint_text)
                .width(160),
            text(value)
                .size(font::BODY)
                .style(theme::primary_text)
                .width(Fill),
        ]
        .spacing(12)
        .align_y(Center),
    )
}

fn preflight(vm: &NewProjectVm) -> Element<'_, UiAction> {
    match vm.preflight.state {
        NewProjectPreflightState::Checking => loading("Checking the Unity license…"),
        NewProjectPreflightState::Ready => notice(
            Tone::Success,
            icons::CIRCLE_CHECK,
            text("Unity license is active.")
                .size(font::BODY)
                .style(theme::muted_text),
        ),
        NewProjectPreflightState::Blocked => notice(
            Tone::Danger,
            icons::CIRCLE_ALERT,
            text(
                vm.preflight
                    .message
                    .clone()
                    .unwrap_or_else(|| "Project creation is blocked.".to_string()),
            )
            .size(font::BODY)
            .line_height(1.55)
            .style(theme::muted_text),
        ),
        NewProjectPreflightState::Idle => hint("License status has not been read yet."),
    }
}

fn issues(vm: &NewProjectVm) -> Element<'_, UiAction> {
    if vm.issues.is_empty() {
        return space().height(0).into();
    }
    notice(
        Tone::Danger,
        icons::CIRCLE_ALERT,
        column(
            vm.issues
                .iter()
                .map(|issue| {
                    text(issue.as_str())
                        .size(font::BODY)
                        .line_height(1.5)
                        .into()
                })
                .collect::<Vec<Element<'_, UiAction>>>(),
        )
        .spacing(4),
    )
}

fn job_block(job: &JobVm) -> Element<'_, UiAction> {
    let tone = match job.phase {
        JobPhase::Succeeded => Tone::Success,
        JobPhase::Failed => Tone::Danger,
        JobPhase::Cancelled => Tone::Warning,
        JobPhase::Running => Tone::Neutral,
    };
    let message: Element<'_, UiAction> = match &job.message {
        Some(message) => text(message.as_str())
            .size(font::SMALL)
            .style(theme::muted_text)
            .into(),
        None => space().height(0).into(),
    };
    let error: Element<'_, UiAction> = match &job.error {
        Some(error) => error_text(error),
        None => space().height(0).into(),
    };
    container(
        column![
            row![
                text(job.kind.as_str())
                    .size(font::BODY_LARGE)
                    .font(fonts::medium()),
                space().width(Fill),
                badge(phase_label(job.phase), tone),
            ]
            .align_y(Center),
            text(job.command.as_str())
                .size(font::MONO)
                .font(fonts::mono())
                .style(theme::faint_text),
            iced::widget::progress_bar(0.0..=100.0, job.progress)
                .length(Fill)
                .girth(6.0)
                .style(theme::progress_bar),
            message,
            error,
        ]
        .spacing(10),
    )
    .padding([14.0, 16.0])
    .width(Fill)
    .style(theme::raised)
    .into()
}

fn phase_label(phase: JobPhase) -> &'static str {
    match phase {
        JobPhase::Running => "running",
        JobPhase::Succeeded => "succeeded",
        JobPhase::Failed => "failed",
        JobPhase::Cancelled => "cancelled",
    }
}

// ---------------------------------------------------------------------------
// Template authoring

fn template_authoring_modal(vm: &TemplateAuthoringVm) -> Element<'_, UiAction> {
    let pack = vm.mode.is_pack();
    let project_picker: Element<'_, UiAction> = if vm.projects.is_empty() {
        notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            text("Register a Unity project first, on the Projects page.")
                .size(font::BODY)
                .style(theme::muted_text),
        )
    } else {
        styled_pick_list(
            vm.projects.clone(),
            vm.selected_project.clone(),
            UiAction::TemplateAuthoringProjectChanged,
            "Select a registered project",
        )
    };

    let fields = column![
        labelled(
            "Package id",
            text_input("com.myorg.template.game", vm.form.name.as_str())
                .on_input(|value| UiAction::TemplateFieldChanged(TemplateField::Name, value))
                .into(),
        ),
        labelled(
            "Display name",
            text_input("My template", vm.form.display_name.as_str())
                .on_input(|value| UiAction::TemplateFieldChanged(TemplateField::DisplayName, value))
                .into(),
        ),
        labelled(
            "Description (optional)",
            text_input(
                "What this template starts with",
                vm.form.description.as_str()
            )
            .on_input(|value| UiAction::TemplateFieldChanged(TemplateField::Description, value))
            .into(),
        ),
        labelled(
            "Version (optional)",
            text_input("1.0.0", vm.form.version.as_str())
                .on_input(|value| UiAction::TemplateFieldChanged(TemplateField::Version, value))
                .into(),
        ),
        labelled(
            if pack {
                "Archive path (.tgz)"
            } else {
                "Output directory"
            },
            text_input(
                if pack {
                    "C:\\templates\\game-template.tgz"
                } else {
                    "C:\\templates"
                },
                vm.form.output.as_str(),
            )
            .on_input(|value| UiAction::TemplateFieldChanged(TemplateField::Output, value))
            .into(),
        ),
    ]
    .spacing(16);

    let mut body = column![
        labelled("Source project", project_picker),
        fields,
        if pack {
            checkbox(
                "Replace the archive if it already exists",
                vm.overwrite,
                UiAction::TemplateOverwriteToggled,
            )
            .into()
        } else {
            hint("The template is registered in the user template store.")
        },
        hint(if pack {
            "The destination must not be inside the source project."
        } else {
            "The output directory is created if it does not exist."
        }),
    ]
    .spacing(18);

    if !vm.issues.is_empty() {
        body = body.push(notice(
            Tone::Danger,
            icons::CIRCLE_ALERT,
            column(
                vm.issues
                    .iter()
                    .map(|issue| text(issue.as_str()).size(font::BODY).into())
                    .collect::<Vec<Element<'_, UiAction>>>(),
            )
            .spacing(4),
        ));
    }

    body = body.push(caption("Command preview"));
    body = body.push(
        container(
            text(vm.command_preview.as_str())
                .size(font::MONO)
                .font(fonts::mono())
                .line_height(1.5),
        )
        .padding([12.0, 14.0])
        .width(Fill)
        .style(theme::inset),
    );

    if let Some(job) = &vm.job {
        body = body.push(job_block(job));
    }
    if let Some(result) = &vm.result {
        body = body.push(notice(
            Tone::Success,
            icons::CIRCLE_CHECK,
            row![
                text(result.as_str())
                    .size(font::MONO)
                    .font(fonts::mono())
                    .width(Fill),
                icon_button(icons::COPY, "Copy path", ButtonKind::Ghost)
                    .on_press(UiAction::CopyText(result.clone()))
                    .padding(crate::components::COMPACT),
            ]
            .spacing(8)
            .align_y(Center),
        ));
    }

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        icon_button(
            if pack { icons::PACKAGE } else { icons::PLUS },
            vm.mode.label(),
            ButtonKind::Primary,
        )
        .on_press_maybe(vm.can_submit.then_some(UiAction::StartTemplateAuthoring)),
    ]
    .spacing(8);

    wide_dialog(
        vm.mode.label(),
        if pack {
            "portable .tgz archive".to_string()
        } else {
            "registered template".to_string()
        },
        body.into(),
        actions.into(),
    )
}

fn verify_state<'a>(state: &'a LoadState<VerifyResultVm>, show_raw: bool) -> Element<'a, UiAction> {
    match state {
        LoadState::Idle => hint("Verifying this build…"),
        LoadState::Loading { previous } => match previous {
            Some(result) => column![
                loading("Checking files and modules…"),
                verify_ready(result, show_raw)
            ]
            .spacing(12)
            .into(),
            None => loading("Checking files and modules…"),
        },
        LoadState::Failed { error, .. } => error_text(error),
        LoadState::Ready(result) => verify_ready(result, show_raw),
    }
}

fn verify_ready(result: &VerifyResultVm, show_raw: bool) -> Element<'_, UiAction> {
    let summary = if result.ok {
        status_line("All checks passed", Tone::Success, icons::CIRCLE_CHECK)
    } else if result.failed_count == 0 {
        status_line(
            "The CLI reported the verify as failed",
            Tone::Danger,
            icons::CIRCLE_X,
        )
    } else {
        status_line(
            format!(
                "{} check{} failed",
                result.failed_count,
                if result.failed_count == 1 { "" } else { "s" }
            ),
            Tone::Danger,
            icons::CIRCLE_X,
        )
    };

    let mut body = column![summary].spacing(14);

    if result.checks.is_empty() {
        body = body.push(hint("The CLI reported no components."));
    } else {
        body = body.push(
            container(scroll(divided(
                result
                    .checks
                    .iter()
                    .map(verify_check_row)
                    .collect::<Vec<_>>(),
            )))
            .max_height(300)
            .width(Fill)
            .style(theme::outline),
        );
    }

    if let Some(command) = &result.repair_command {
        body = body.push(notice(
            Tone::Info,
            icons::TERMINAL,
            column![
                text("The CLI named this repair command; the app never runs it for you.")
                    .size(font::BODY)
                    .style(theme::muted_text),
                mono(command.as_str()),
            ]
            .spacing(6),
        ));
    }

    if !result.ok && show_raw {
        body = body.push(caption("Raw log"));
        body = body.push(
            container(scroll(
                text(result.raw.as_str())
                    .size(font::MONO)
                    .font(fonts::mono()),
            ))
            .max_height(220)
            .width(Fill)
            .padding(12)
            .style(theme::outline),
        );
    }

    body.into()
}

fn verify_check_row(check: &VerifyCheckVm) -> Element<'_, UiAction> {
    let tone = check_status_tone(check.status);
    let mut details = column![].spacing(4);
    if !check.kind.trim().is_empty() {
        details = details.push(hint(check.kind.as_str()));
    }
    if check.status == CheckStatus::Fail {
        if let Some(path) = &check.path {
            details = details.push(mono(path.as_str()));
        }
        if let Some(command) = &check.repair_command {
            details = details.push(mono(command.as_str()));
        }
    }

    container(
        row![
            badge(check_status_label(check.status), tone),
            column![
                text(check.component.as_str()).size(font::BODY_LARGE),
                details
            ]
            .spacing(4)
            .width(Fill),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Start),
    )
    .padding([10.0, 0.0])
    .width(Fill)
    .into()
}

/// Confirm `unity self-update --rollback`. The dialog names exactly what is
/// restored and never carries a target, channel, changelog, check or dry-run
/// flag, because beta.17 refuses that combination with `BAD_ARGS`.
fn rollback_cli_modal(vm: &RollbackCliVm) -> Element<'_, UiAction> {
    let mut body = column![
        notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            column![
                text(format!("Restore {}.", vm.restore_label))
                    .size(font::BODY_LARGE)
                    .font(fonts::medium()),
                text("The CLI replaces its current binary with the kept one. Open Editors and running jobs keep their files, but the command line changes immediately.")
                    .size(font::BODY)
                    .line_height(1.5)
                    .style(theme::muted_text),
            ]
            .spacing(6),
        ),
        labelled(
            "Kept version (leave empty for the most recent)",
            text_input("e.g. 1.0.0-beta.16", &vm.version_input)
                .on_input(UiAction::CliRollbackVersionChanged)
                .into(),
        ),
        row![
            checkbox(
                format!("I understand the CLI will restore {}.", vm.restore_label),
                vm.confirmed,
                UiAction::CliRollbackConfirmed,
            )
            .width(Fill),
        ],
        hint("Exact command"),
        mono(vm.command.as_str()),
        hint("Rollback is never combined with --check, --changelog, --target, --channel or --dry-run."),
    ]
    .spacing(18);

    if vm.job_running {
        body = body.push(notice(
            Tone::Info,
            icons::LOADER,
            text("Rolling back… the job card shows the live log and Cancel.")
                .size(font::BODY)
                .style(theme::muted_text),
        ));
    }

    if let Some(error) = &vm.error {
        body = body.push(error_text(error));
    }

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        icon_button(icons::CIRCLE_ARROW_UP, "Roll back", ButtonKind::Danger)
            .on_press_maybe(vm.can_rollback.then_some(UiAction::StartCliRollback)),
    ]
    .spacing(8);

    dialog(
        "Roll back the Unity CLI",
        "A rollback restores a binary the CLI kept from a previous update.",
        body.into(),
        actions.into(),
    )
}

fn status_line(label: impl Into<String>, tone: Tone, glyph: char) -> Element<'static, UiAction> {
    row![
        icon(glyph, font::ICON_SMALL).style(theme::tone_text(tone)),
        text(label.into())
            .size(font::BODY_LARGE)
            .font(fonts::medium()),
    ]
    .spacing(8)
    .align_y(Center)
    .into()
}

fn check_status_tone(status: CheckStatus) -> Tone {
    match status {
        CheckStatus::Pass => Tone::Success,
        CheckStatus::Warn => Tone::Warning,
        CheckStatus::Fail => Tone::Danger,
        CheckStatus::Info => Tone::Info,
        CheckStatus::Unknown => Tone::Neutral,
    }
}

fn check_status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "pass",
        CheckStatus::Warn => "warn",
        CheckStatus::Fail => "fail",
        CheckStatus::Info => "info",
        CheckStatus::Unknown => "unknown",
    }
}
