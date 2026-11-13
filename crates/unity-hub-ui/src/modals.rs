use iced::widget::{column, container, pick_list, row, space};
use iced::{Center, Element, Fill};

use unity_hub_model::{
    EditorModulesVm, InstallCliVm, JobPhase, JobVm, ModalVm, ModuleOptionVm,
    NewProjectPreflightState, NewProjectStep, NewProjectVm, Page, RemoveProjectVm,
    TemplateAuthoringVm, TemplateField, TemplateRowVm, Tone, UiAction, UpgradeProjectVm,
};

use crate::components::{
    badge, button, caption, checkbox, divided, divider, error_text, field_label, hint, icon_button,
    loading, mono, notice, scroll, text_input, ButtonKind, PICK_LIST_PADDING,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::font;
use crate::ui::{self, text};

const DIALOG_WIDTH: f32 = 560.0;

pub fn view(modal: &ModalVm) -> Element<'_, UiAction> {
    let card = match modal {
        ModalVm::InstallCli(vm) => install_cli_modal(vm),
        ModalVm::EditorModules(vm) => editor_modules_modal(vm),
        ModalVm::UpgradeProject(vm) => upgrade_modal(vm),
        ModalVm::RemoveProject(vm) => remove_project_modal(vm),
        ModalVm::NewProject(vm) => new_project_modal(vm),
        ModalVm::TemplateAuthoring(vm) => template_authoring_modal(vm),
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
    .width(DIALOG_WIDTH)
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

    let body = column![
        labelled("Installed", installed),
        labelled("Available", module_list),
        checkbox(
            "I accept the module license agreements (--accept-eula)",
            vm.accept_eula,
            UiAction::DialogEulaToggled,
        ),
    ]
    .spacing(20);

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

    dialog(
        format!("Modules for Unity {}", vm.editor_version),
        "Only modules that are not already installed are listed.",
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

fn version_cell<'a>(label: &'a str, version: String, end: bool) -> Element<'a, UiAction> {
    let content = column![
        caption(label),
        text(version)
            .size(font::BODY_LARGE + 1.0)
            .font(fonts::mono()),
    ]
    .spacing(4);

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

    let strip = container(
        row![
            version_cell("Current", vm.current_version.clone(), false),
            icon(icons::ARROW_RIGHT, font::ICON).style(theme::faint_text),
            version_cell(
                "Target",
                vm.selected_target
                    .clone()
                    .unwrap_or_else(|| "—".to_string()),
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
