use iced::widget::{column, container, pick_list, row, space};
use iced::{Center, Element, Fill};

use unity_hub_model::{
    CheckStatus, EditorModulesVm, InstallCliVm, LoadState, ModalVm, ModuleOptionVm,
    PruneCandidateVm, PruneEditorsVm, PruneReportVm, RollbackCliVm, Tone, UiAction,
    UpgradeProjectVm, VerifyCheckVm, VerifyEditorVm, VerifyResultVm,
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
                .font(fonts::SEMIBOLD),
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
                    .font(fonts::MONO)
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
        text(version).size(font::BODY_LARGE + 1.0).font(fonts::MONO),
    ]
    .spacing(4);
    if let Some(architecture) = architecture.filter(|value| !value.trim().is_empty()) {
        content = content.push(
            text(architecture)
                .size(font::SMALL)
                .font(fonts::MONO)
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
                    .font(fonts::MEDIUM),
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
                    .font(fonts::MONO),
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
                text(result.raw.as_str()).size(font::MONO).font(fonts::MONO),
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
                    .font(fonts::MEDIUM),
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
            .font(fonts::MEDIUM),
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
