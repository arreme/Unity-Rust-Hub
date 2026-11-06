use iced::widget::{column, container, row};
use iced::{Center, Element, Fill};

use unity_hub_model::{
    LoadState, SkillClientVm, SkillInstallVm, SkillOutcomeRowVm, SkillsVm, Tone, UiAction,
};

use crate::components::{
    badge, button, card, card_with, checkbox, divided, empty, error_text, hint, icon_button,
    list_row, loading, mono, page_header, text_input, ButtonKind, COMPACT,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::text;

pub fn view(vm: &SkillsVm) -> Element<'_, UiAction> {
    let reload =
        icon_button(icons::REFRESH, "Reload", ButtonKind::Secondary).on_press(UiAction::Refresh);

    column![
        page_header(
            "Skills",
            "The Unity agent skill, the AI clients on this machine, and the copies the CLI tracks.",
            Some(reload.into()),
        ),
        hint("Read first. Installing or refreshing writes only when you click, and every outcome is shown row by row."),
        clients_card(vm),
        installs_card(vm),
        install_card(vm),
        refresh_card(vm),
        result_card(vm),
    ]
    .spacing(spacing::SECTION)
    .into()
}

fn clients_card(vm: &SkillsVm) -> Element<'_, UiAction> {
    let installed = vm
        .clients
        .data()
        .map(|clients| clients.iter().filter(|client| client.installed).count());

    let trailing = match installed {
        Some(count) => badge(format!("{count} installed"), Tone::Neutral),
        None => badge("unity skill install --list", Tone::Neutral),
    };

    card_with(
        "Supported clients",
        trailing,
        section_body(
            &vm.clients,
            "Reading `unity skill install --list`…",
            "No supported clients reported",
            "The CLI did not list an AI client for the Unity skill.",
            |clients| client_rows(clients, vm.job_running),
        ),
    )
}

fn installs_card(vm: &SkillsVm) -> Element<'_, UiAction> {
    let trailing = match vm.installs.data() {
        Some(installs) => badge(format!("{} tracked", installs.len()), Tone::Neutral),
        None => badge("unity skills", Tone::Neutral),
    };

    card_with(
        "Tracked installs",
        trailing,
        section_body(
            &vm.installs,
            "Reading `unity skills`…",
            "The CLI tracks no skill installs",
            "Install the skill into a client above; the CLI will report the copy here.",
            |installs| tracked_rows(installs),
        ),
    )
}

fn install_card(vm: &SkillsVm) -> Element<'_, UiAction> {
    let local: Element<'_, UiAction> = if vm.local_available {
        checkbox(
            "Project copy (`--local`)",
            vm.install.local,
            UiAction::SkillLocalToggled,
        )
        .into()
    } else {
        column![
            row![
                text("Project copy (`--local`)").size(font::BODY),
                badge("unavailable", Tone::Neutral),
            ]
            .spacing(8)
            .align_y(Center),
            hint(
                vm.local_disabled_reason
                    .as_deref()
                    .unwrap_or("`--local` needs a Unity project context.")
            ),
        ]
        .spacing(3)
        .into()
    };

    let project = match &vm.project_context {
        Some(path) => detail("Project context", path),
        None => hint("No project context: `--local` stays off."),
    };

    let skill_input = column![
        text("Only one skill (`--skill`)")
            .size(font::BODY)
            .style(theme::primary_text),
        text_input("Leave empty for every skill", &vm.install.skill_input)
            .on_input(UiAction::SkillNameChanged)
            .width(Fill),
    ]
    .spacing(6);

    let mut options = column![
        local,
        checkbox(
            "Replace an outdated copy (`--yes`)",
            vm.install.overwrite,
            UiAction::SkillOverwriteToggled,
        ),
        checkbox(
            "Preview only (`--dry-run`)",
            vm.install.dry_run,
            UiAction::SkillDryRunToggled,
        ),
        skill_input,
    ]
    .spacing(10);

    if let Some(error) = &vm.install.skill_input_error {
        options = options.push(hint(error.as_str()));
    }

    let all: Element<'_, UiAction> = if vm.job_running {
        hint("A job is running; wait for it to finish.")
    } else {
        button("Install to all clients", ButtonKind::Primary)
            .on_press(UiAction::InstallAllSkills)
            .into()
    };

    card(
        "Install options",
        column![
            options,
            hint("Each client row runs `unity skill install <client>` with these options. Without `--yes` the CLI leaves an outdated copy alone and reports it as `skipped`."),
            project,
            hint("Install to every supported client runs:"),
            command_line(vm.install.all_command.as_str()),
            hint("`--local` writes into the project above only; the CLI skips Claude Desktop because its skills are global only."),
            all,
        ]
        .spacing(spacing::SMALL + 2.0)
        .into(),
    )
}

fn refresh_card(vm: &SkillsVm) -> Element<'_, UiAction> {
    let mut actions = row![].spacing(8).align_y(Center);

    let primary: Element<'_, UiAction> = if vm.job_running {
        hint("A job is running.")
    } else {
        button(vm.refresh_label.as_str(), ButtonKind::Primary)
            .on_press(UiAction::StartSkillRefresh)
            .into()
    };
    actions = actions.push(primary);

    if vm.refresh_label != "Preview refresh (dry run)" && !vm.job_running {
        actions = actions.push(
            button("Preview again", ButtonKind::Ghost).on_press(UiAction::PreviewSkillRefresh),
        );
    }

    card(
        "Refresh tracked installs",
        column![
            hint(vm.refresh_hint.as_str()),
            hint("A refresh renders the skill for every copy the CLI tracks and reports one row per copy. It never installs into a client that has no copy."),
            command_line(vm.refresh_command.as_str()),
            actions,
        ]
        .spacing(spacing::SMALL + 2.0)
        .into(),
    )
}

fn result_card(vm: &SkillsVm) -> Element<'_, UiAction> {
    let Some(result) = &vm.result else {
        return empty(
            "No skill run yet",
            "Install a client or run a refresh; the CLI's outcome rows appear here.",
        );
    };

    let trailing = if result.dry_run {
        badge("dry run · changes nothing", Tone::Info)
    } else {
        badge("applied", Tone::Neutral)
    };

    let mut body = column![].spacing(spacing::SMALL + 2.0);

    if result.rows.is_empty() {
        body = body.push(empty(
            "The CLI reported no install rows",
            "A refresh is guaranteed to answer with one row per tracked install. If this run reported none, check the job card for the raw result.",
        ));
    } else {
        body = body.push(divided(
            result.rows.iter().map(outcome_row).collect::<Vec<_>>(),
        ));
    }

    if let Some(force) = &result.force {
        body = body.push(force_offer(
            force.command.as_str(),
            force.explanation.as_str(),
            vm.job_running,
        ));
    }

    card_with(
        if result.dry_run {
            "Last run (dry run)"
        } else {
            "Last run"
        },
        trailing,
        body.into(),
    )
}

fn force_offer<'a>(
    command: &'a str,
    explanation: &'a str,
    job_running: bool,
) -> Element<'a, UiAction> {
    let action: Element<'a, UiAction> = if job_running {
        hint("A job is running; wait before taking ownership.")
    } else {
        button("Take ownership with `--force`", ButtonKind::Danger)
            .on_press(UiAction::ForceSkillLastRun)
            .into()
    };

    crate::components::notice(
        Tone::Danger,
        icons::TRIANGLE_ALERT,
        column![
            text("A conflict needs your decision")
                .size(font::BODY_LARGE)
                .font(fonts::SEMIBOLD),
            text(explanation).size(font::BODY),
            command_line(command),
            action,
        ]
        .spacing(8),
    )
}

fn client_rows(clients: &[SkillClientVm], job_running: bool) -> Element<'_, UiAction> {
    if clients.is_empty() {
        return empty(
            "No supported clients reported",
            "`unity skill install --list` returned an empty list.",
        );
    }

    divided(
        clients
            .iter()
            .map(|client| client_row(client, job_running))
            .collect::<Vec<_>>(),
    )
    .into()
}

fn client_row(client: &SkillClientVm, job_running: bool) -> Element<'_, UiAction> {
    let mut label = row![text(client.label.as_str())
        .size(font::BODY_LARGE)
        .font(fonts::MEDIUM)]
    .spacing(8)
    .align_y(Center);

    if client.client != client.label {
        label = label.push(mono(client.client.as_str()));
    }

    let action: Element<'_, UiAction> = if job_running {
        hint("Running")
    } else {
        button("Install", ButtonKind::Secondary)
            .on_press(UiAction::InstallSkill(client.client.clone()))
            .padding(COMPACT)
            .into()
    };

    list_row(
        row![
            column![label, mono(path_or_dash(client.path.as_str()))]
                .spacing(3)
                .width(Fill),
            installed_badge(client.installed),
            action,
        ]
        .spacing(16)
        .align_y(Center),
    )
}

fn tracked_rows(installs: &[SkillInstallVm]) -> Element<'_, UiAction> {
    if installs.is_empty() {
        return empty(
            "The CLI tracks no skill installs",
            "Install the skill into a client above; the CLI will report the copy here.",
        );
    }

    divided(
        installs
            .iter()
            .map(|install| {
                list_row(
                    row![
                        column![
                            row![
                                text(if install.skill.is_empty() {
                                    "skill"
                                } else {
                                    install.skill.as_str()
                                })
                                .size(font::BODY_LARGE)
                                .font(fonts::MEDIUM),
                                badge(install.client.as_str(), Tone::Neutral),
                            ]
                            .spacing(8)
                            .align_y(Center),
                            mono(path_or_dash(install.path.as_str())),
                        ]
                        .spacing(3)
                        .width(Fill),
                        badge(
                            install.status.as_str(),
                            install_status_tone(install.status.as_str()),
                        ),
                    ]
                    .spacing(16)
                    .align_y(Center),
                )
            })
            .collect::<Vec<_>>(),
    )
    .into()
}

fn outcome_row(row: &SkillOutcomeRowVm) -> Element<'_, UiAction> {
    let mut body = column![
        row![
            text(if row.client.is_empty() {
                "unknown client"
            } else {
                row.client.as_str()
            })
            .size(font::BODY_LARGE)
            .font(fonts::MEDIUM),
            badge(
                if row.skill.is_empty() {
                    "skill"
                } else {
                    row.skill.as_str()
                },
                Tone::Neutral,
            ),
        ]
        .spacing(8)
        .align_y(Center),
        mono(path_or_dash(row.path.as_str())),
    ]
    .spacing(3);

    if let Some(reason) = &row.skip_reason {
        body = body.push(hint(format!("skipReason: {reason}")));
    } else if matches!(
        &row.outcome,
        unity_hub_model::SkillOutcomeVm::Skipped
            | unity_hub_model::SkillOutcomeVm::WouldSkip
            | unity_hub_model::SkillOutcomeVm::Conflict
    ) {
        body = body.push(hint("The CLI gave no reason for this row."));
    }

    list_row(
        row![
            body.width(Fill),
            badge(row.outcome.label(), row.outcome.tone()),
        ]
        .spacing(16)
        .align_y(Center),
    )
}

fn installed_badge(installed: bool) -> Element<'static, UiAction> {
    if installed {
        badge("installed", Tone::Success)
    } else {
        badge("absent", Tone::Neutral)
    }
}

fn install_status_tone(status: &str) -> Tone {
    match status.trim().to_ascii_lowercase().as_str() {
        "installed" | "current" | "up-to-date" => Tone::Success,
        "absent" | "missing" => Tone::Warning,
        "outdated" => Tone::Accent,
        _ => Tone::Neutral,
    }
}

fn section_body<'a, T>(
    state: &'a LoadState<T>,
    loading_label: &'a str,
    empty_title: &'a str,
    empty_description: &'a str,
    render: impl Fn(&'a T) -> Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    match state {
        LoadState::Idle => empty(empty_title, empty_description),
        LoadState::Loading { previous } => match previous {
            Some(data) => column![loading(loading_label), render(data)]
                .spacing(spacing::SMALL + 2.0)
                .into(),
            None => loading(loading_label),
        },
        LoadState::Ready(data) => render(data),
        LoadState::Failed { error, previous } => {
            let mut body = column![error_text(error)].spacing(spacing::SMALL + 2.0);
            if let Some(data) = previous {
                body = body.push(render(data));
            }
            body.into()
        }
    }
}

fn command_line<'a>(command: &'a str) -> Element<'a, UiAction> {
    if command.trim().is_empty() {
        return hint("The command is not available yet.");
    }

    container(
        row![
            icon(icons::TERMINAL, font::ICON_SMALL).style(theme::faint_text),
            mono(command),
        ]
        .spacing(8)
        .align_y(Center),
    )
    .padding([10.0, 12.0])
    .width(Fill)
    .style(theme::inset)
    .into()
}

fn detail<'a>(label: &'a str, value: &'a str) -> Element<'a, UiAction> {
    column![
        text(label).size(font::SMALL).style(theme::faint_text),
        mono(value),
    ]
    .spacing(3)
    .into()
}

fn path_or_dash(path: &str) -> &str {
    if path.trim().is_empty() {
        "—"
    } else {
        path
    }
}
