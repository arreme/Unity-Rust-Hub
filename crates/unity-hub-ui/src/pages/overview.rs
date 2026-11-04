use iced::widget::{button, column, container, row, rule, space};
use iced::{Element, Fill};

use unity_hub_model::{
    AccountCardVm, AuthVm, CliCardVm, CliDetailsVm, LicenseVm, LoadState, OverviewVm, Tone,
    UiAction, UpdateVm,
};

use crate::components::{badge, card, detail_column};
use crate::theme;
use crate::ui::text;

pub fn view(vm: &OverviewVm) -> Element<'_, UiAction> {
    let mut page = column![hero()].spacing(16);

    page = page.push(
        row![cli_card(vm), account_card(&vm.account)]
            .spacing(16)
            .align_y(iced::Alignment::Start),
    );

    page = page.push(running_card(&vm.running));

    page.into()
}

fn hero() -> Element<'static, UiAction> {
    container(
        row![
            column![
                text("LOCAL UNITY TOOLCHAIN")
                    .size(9)
                    .style(theme::accent_text),
                text("One desk for the Unity CLI.").size(28),
                text("Check the CLI, manage Editors and modules, upgrade project versions, and read release notes — natively.")
                    .size(12)
                    .style(theme::muted_text),
            ]
            .spacing(5)
            .width(Fill),
            container(text("U").size(34).style(theme::accent_text))
                .width(76)
                .height(76)
                .center(76)
                .style(theme::badge(Tone::Accent)),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center),
    )
    .padding(24)
    .width(Fill)
    .style(theme::hero)
    .into()
}

fn cli_card(vm: &OverviewVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(11);

    match &vm.cli {
        CliCardVm::Idle => {}
        CliCardVm::Loading => {
            body = body.push(
                text("Resolving the Unity CLI…")
                    .size(11)
                    .style(theme::muted_text),
            );
        }
        CliCardVm::Missing => {
            body = body.push(install_callout());
        }
        CliCardVm::Failed(error) => {
            body = body.push(text(error.text()).size(10).style(theme::danger_text));
        }
        CliCardVm::Ready { details, error } => {
            body = body.push(cli_details(details, &vm.update));

            if let Some(update) = vm.update.data() {
                if update.available {
                    body = body.push(update_callout(update));
                }
            }

            if let Some(error) = error {
                body = body.push(text(error.text()).size(10).style(theme::danger_text));
            }
            if let Some(error) = vm.update.error() {
                body = body.push(
                    text(format!("Update check failed: {}", error.text()))
                        .size(10)
                        .style(theme::danger_text),
                );
            }

            body = body.push(
                row![
                    button(text("Re-check").size(10))
                        .on_press(UiAction::Refresh)
                        .padding([5.0, 9.0])
                        .style(button::secondary),
                    button(text("Open Settings").size(10))
                        .on_press(UiAction::Navigate(unity_hub_model::Page::Settings))
                        .padding([5.0, 9.0])
                        .style(button::text),
                ]
                .spacing(6),
            );
        }
    }

    card("Unity CLI", body.into())
}

fn cli_details<'a>(
    details: &'a CliDetailsVm,
    update: &'a LoadState<UpdateVm>,
) -> Element<'a, UiAction> {
    column![
        row![
            text(details.version.as_str())
                .size(24)
                .style(theme::accent_text),
            badge(details.channel.clone(), Tone::Accent),
            update_badge(update),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        row![
            detail_column("Platform", details.platform.as_str()),
            detail_column("Runtime", details.runtime.as_str()),
        ]
        .spacing(16),
        row![
            detail_column("Executable", details.executable.as_str()),
            detail_column("Resolved via", details.resolved_via.as_str()),
        ]
        .spacing(16),
        row![
            detail_column("Editor install path", details.install_path.as_str()),
            detail_column("Build date", details.build_date.as_str()),
        ]
        .spacing(16),
    ]
    .spacing(11)
    .into()
}

fn update_badge(update: &LoadState<UpdateVm>) -> Element<'static, UiAction> {
    if update.is_loading() {
        badge("checking updates", Tone::Info)
    } else if update.error().is_some() {
        badge("update check failed", Tone::Neutral)
    } else if let Some(update) = update.data() {
        if update.available {
            badge("update available", Tone::Warning)
        } else {
            badge("up to date", Tone::Success)
        }
    } else {
        badge("unknown", Tone::Neutral)
    }
}

fn update_callout(update: &UpdateVm) -> Element<'static, UiAction> {
    container(
        row![
            column![
                text("A newer CLI is available.").size(11),
                text(format!(
                    "{} → {}",
                    update.current_version, update.latest_version
                ))
                .size(10)
                .style(theme::muted_text),
            ]
            .spacing(2)
            .width(Fill),
            button(text("Update CLI").size(10))
                .on_press(UiAction::UpdateCli)
                .padding([5.0, 9.0])
                .style(button::primary),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    )
    .padding(10)
    .width(Fill)
    .style(theme::notice_card(Tone::Warning))
    .into()
}

fn install_callout() -> Element<'static, UiAction> {
    container(
        row![
            column![
                text("The Unity CLI is not installed.").size(13),
                text("This app is a native GUI over the official unity binary. Install it to unlock Editors, projects, and release notes.")
                    .size(10)
                    .style(theme::muted_text),
            ]
            .spacing(3)
            .width(Fill),
            button(text("Install Unity CLI").size(10))
                .on_press(UiAction::OpenInstallCli)
                .padding([6.0, 10.0])
                .style(button::primary),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
    )
    .padding(12)
    .width(Fill)
    .style(theme::notice_card(Tone::Warning))
    .into()
}

fn account_card(account: &AccountCardVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(8);

    match &account.auth {
        AuthVm::Idle => {}
        AuthVm::Loading => {
            body = body.push(text("Checking sign-in…").size(10).style(theme::muted_text));
        }
        AuthVm::SignedIn { name, email } => {
            body = body.push(
                row![
                    container(text("U").size(16).style(theme::accent_text))
                        .width(38)
                        .height(38)
                        .center(38)
                        .style(theme::badge(Tone::Accent)),
                    column![
                        text(name.as_str()).size(12),
                        text(email.as_str()).size(10).style(theme::muted_text),
                    ]
                    .spacing(1),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
            );
        }
        AuthVm::SignedOut { expired } => {
            let message = if *expired {
                "Your session expired. Sign in again."
            } else {
                "Not signed in."
            };

            body = body.push(
                row![
                    text(message).size(11).style(theme::muted_text).width(Fill),
                    button(text("Sign in").size(10))
                        .on_press(UiAction::SignIn)
                        .padding([5.0, 9.0])
                        .style(button::primary),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        AuthVm::Failed(error) => {
            body = body.push(
                text(format!("Sign-in unavailable: {}", error.message))
                    .size(10)
                    .style(theme::muted_text),
            );
        }
    }

    body = body.push(rule::horizontal(1));

    match &account.license {
        LicenseVm::Idle => {}
        LicenseVm::Failed(error) => {
            body = body.push(
                text(format!("License unavailable: {}", error.message))
                    .size(10)
                    .style(theme::muted_text),
            );
        }
        LicenseVm::Ready { active, licenses } => {
            body = body.push(
                row![
                    text("License").size(10).style(theme::faint_text),
                    space().width(Fill),
                    badge(
                        if *active { "active" } else { "inactive" },
                        if *active { Tone::Success } else { Tone::Danger },
                    ),
                ]
                .align_y(iced::Alignment::Center),
            );

            if licenses.is_empty() {
                body = body.push(
                    text("No licenses reported.")
                        .size(10)
                        .style(theme::muted_text),
                );
            } else {
                for license in licenses {
                    body = body.push(
                        row![
                            text(license.product.as_str()).size(11).width(Fill),
                            badge(license.kind.clone(), Tone::Neutral),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    );
                }
            }
        }
    }

    card("Account", body.into())
}

fn running_card(
    running: &LoadState<Vec<unity_hub_model::RunningEditorVm>>,
) -> Element<'_, UiAction> {
    let mut body = column![].spacing(8);

    if running.is_loading() && running.data().is_none() {
        body = body.push(
            text("Checking running Editors…")
                .size(10)
                .style(theme::muted_text),
        );
    } else if let Some(instances) = running.data() {
        if instances.is_empty() {
            body = body.push(
                column![
                    text("No Editor running.").size(12),
                    text("Open a project with the Pipeline package to see live Editors here.")
                        .size(10)
                        .style(theme::muted_text),
                ]
                .spacing(3),
            );
        } else {
            for instance in instances {
                let mut details = column![
                    text(instance.project_path.as_str()).size(11),
                    text(format!(
                        "{} · pid {}",
                        instance.editor_version, instance.pid
                    ))
                    .size(9)
                    .style(theme::faint_text),
                ]
                .spacing(2);

                if let Some(phase) = &instance.phase {
                    details = details.push(
                        text(format!("Phase: {phase}"))
                            .size(9)
                            .style(theme::muted_text),
                    );
                }

                let mut badges = Vec::new();
                if let Some(version) = &instance.pipeline_version {
                    badges.push(badge(format!("Pipeline {version}"), Tone::Highlight));
                }
                if instance.update_available {
                    badges.push(badge("update available", Tone::Warning));
                }
                if instance.safe_mode {
                    badges.push(badge("Safe mode", Tone::Danger));
                }
                if !badges.is_empty() {
                    details = details.push(row(badges).spacing(6));
                }

                if let Some(dialog) = &instance.dialog {
                    details = details.push(
                        text(format!("Waiting on dialog: {dialog}"))
                            .size(9)
                            .style(theme::warning_text),
                    );
                }

                body = body.push(container(details).padding(8).width(Fill).style(theme::card));
            }
        }
    } else if let Some(error) = running.error() {
        body = body.push(
            text(error.message.as_str())
                .size(10)
                .style(theme::muted_text),
        );
    }

    card("Running Editors", body.into())
}
