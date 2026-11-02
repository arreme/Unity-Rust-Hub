use iced::widget::{button, column, container, row, rule, space};
use iced::{Element, Fill};

use crate::app::{badge, detail_column, App, Message};
use crate::theme;
use crate::ui::text;
use crate::util::{error_text, format_iso};

pub fn view(app: &App) -> Element<'_, Message> {
    let mut page = column![hero()].spacing(16);

    page = page.push(
        row![cli_card(app), account_card(app)]
            .spacing(16)
            .align_y(iced::Alignment::Start),
    );

    page = page.push(running_card(app));

    page.into()
}

fn hero() -> Element<'static, Message> {
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
                .style(theme::badge(theme::Tone::Accent)),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center),
    )
    .padding(24)
    .width(Fill)
    .style(theme::hero)
    .into()
}

fn cli_card(app: &App) -> Element<'_, Message> {
    let mut body = column![].spacing(11);

    if app.cli_status.loading && app.cli_status.data.is_none() {
        body = body.push(
            text("Resolving the Unity CLI…")
                .size(11)
                .style(theme::muted_text),
        );
    } else if let Some(status) = &app.cli_status.data {
        if !status.installed {
            body = body.push(install_callout());
        } else {
            let version = status
                .version
                .as_ref()
                .map(|version| version.version.as_str())
                .unwrap_or("unknown");

            body = body.push(
                row![
                    text(version).size(24).style(theme::accent_text),
                    badge(
                        status
                            .version
                            .as_ref()
                            .map(|version| version.channel.as_str())
                            .unwrap_or("unknown"),
                        theme::Tone::Accent,
                    ),
                    update_badge(app),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );

            body = body.push(
                row![
                    detail_column(
                        "Platform",
                        format!(
                            "{} {}",
                            status
                                .version
                                .as_ref()
                                .map(|v| v.platform.as_str())
                                .unwrap_or("—"),
                            status
                                .version
                                .as_ref()
                                .map(|v| v.arch.as_str())
                                .unwrap_or("")
                        ),
                    ),
                    detail_column(
                        "Runtime",
                        status
                            .version
                            .as_ref()
                            .and_then(|v| v.runtime.as_deref())
                            .unwrap_or("—"),
                    ),
                ]
                .spacing(16),
            );
            body = body.push(
                row![
                    detail_column("Executable", status.path.as_deref().unwrap_or("—")),
                    detail_column("Resolved via", status.path_source.as_deref().unwrap_or("—"),),
                ]
                .spacing(16),
            );
            body = body.push(
                row![
                    detail_column(
                        "Editor install path",
                        status.install_path.as_deref().unwrap_or("—"),
                    ),
                    detail_column(
                        "Build date",
                        format_iso(
                            status
                                .version
                                .as_ref()
                                .and_then(|v| v.build_date.as_deref())
                        ),
                    ),
                ]
                .spacing(16),
            );

            if let Some(update) = &app.update.data {
                if update.update_available {
                    body = body.push(
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
                                    .on_press(Message::UpdateCli)
                                    .padding([5.0, 9.0])
                                    .style(button::primary),
                            ]
                            .spacing(10)
                            .align_y(iced::Alignment::Center),
                        )
                        .padding(10)
                        .width(Fill)
                        .style(theme::notice_card(theme::Tone::Amber)),
                    );
                }
            }

            if let Some(error) = &app.cli_status.error {
                body = body.push(text(error_text(error)).size(10).style(theme::danger_text));
            }
            if let Some(error) = &app.update.error {
                body = body.push(
                    text(format!("Update check failed: {}", error_text(error)))
                        .size(10)
                        .style(theme::danger_text),
                );
            }

            body = body.push(
                row![
                    button(text("Re-check").size(10))
                        .on_press(Message::Refresh)
                        .padding([5.0, 9.0])
                        .style(button::secondary),
                    button(text("Open Settings").size(10))
                        .on_press(Message::Navigate(crate::app::Page::Settings))
                        .padding([5.0, 9.0])
                        .style(button::text),
                ]
                .spacing(6),
            );
        }
    } else if let Some(error) = &app.cli_status.error {
        body = body.push(text(error_text(error)).size(10).style(theme::danger_text));
    }

    card("Unity CLI", body.into())
}

fn update_badge(app: &App) -> Element<'_, Message> {
    if app.update.loading {
        badge("checking updates", theme::Tone::Blue)
    } else if app.update.error.is_some() {
        badge("update check failed", theme::Tone::Neutral)
    } else if app
        .update
        .data
        .as_ref()
        .map(|update| update.update_available)
        .unwrap_or(false)
    {
        badge("update available", theme::Tone::Amber)
    } else if app.update.data.is_some() {
        badge("up to date", theme::Tone::Green)
    } else {
        badge("unknown", theme::Tone::Neutral)
    }
}

fn install_callout() -> Element<'static, Message> {
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
                .on_press(Message::OpenInstallCli)
                .padding([6.0, 10.0])
                .style(button::primary),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
    )
    .padding(12)
    .width(Fill)
    .style(theme::notice_card(theme::Tone::Amber))
    .into()
}

fn account_card(app: &App) -> Element<'_, Message> {
    let mut body = column![].spacing(8);

    if app.auth.loading && app.auth.data.is_none() {
        body = body.push(text("Checking sign-in…").size(10).style(theme::muted_text));
    } else if let Some(auth) = &app.auth.data {
        if auth.logged_in {
            body = body.push(
                row![
                    container(text("U").size(16).style(theme::accent_text))
                        .width(38)
                        .height(38)
                        .center(38)
                        .style(theme::badge(theme::Tone::Accent)),
                    column![
                        text(
                            auth.user
                                .as_ref()
                                .map(|u| u.name.as_str())
                                .unwrap_or("Signed in")
                        )
                        .size(12),
                        text(
                            auth.user
                                .as_ref()
                                .map(|u| u.email.as_str())
                                .unwrap_or_default()
                        )
                        .size(10)
                        .style(theme::muted_text),
                    ]
                    .spacing(1),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
            );
        } else {
            let message = if auth.session_state.as_deref() == Some("SESSION_EXPIRED") {
                "Your session expired. Sign in again."
            } else {
                "Not signed in."
            };

            body = body.push(
                row![
                    text(message).size(11).style(theme::muted_text).width(Fill),
                    button(text("Sign in").size(10))
                        .on_press(Message::SignIn)
                        .padding([5.0, 9.0])
                        .style(button::primary),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
    } else if let Some(error) = &app.auth.error {
        body = body.push(
            text(format!("Sign-in unavailable: {}", error.message))
                .size(10)
                .style(theme::muted_text),
        );
    }

    body = body.push(rule::horizontal(1));

    if let Some(license) = &app.license.data {
        body = body.push(
            row![
                text("License").size(10).style(theme::faint_text),
                space().width(Fill),
                badge(
                    if license.active { "active" } else { "inactive" },
                    if license.active {
                        theme::Tone::Green
                    } else {
                        theme::Tone::Red
                    },
                ),
            ]
            .align_y(iced::Alignment::Center),
        );

        if license.licenses.is_empty() {
            body = body.push(
                text("No licenses reported.")
                    .size(10)
                    .style(theme::muted_text),
            );
        } else {
            for license in license.licenses.iter().take(4) {
                body = body.push(
                    row![
                        text(&license.product).size(11).width(Fill),
                        badge(license.kind.as_str(), theme::Tone::Neutral),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
            }
        }
    } else if let Some(error) = &app.license.error {
        body = body.push(
            text(format!("License unavailable: {}", error.message))
                .size(10)
                .style(theme::muted_text),
        );
    }

    card("Account", body.into())
}

fn running_card(app: &App) -> Element<'_, Message> {
    let mut body = column![].spacing(8);

    if app.running.loading && app.running.data.is_none() {
        body = body.push(
            text("Checking running Editors…")
                .size(10)
                .style(theme::muted_text),
        );
    } else if let Some(running) = &app.running.data {
        if running.count == 0 {
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
            for instance in &running.instances {
                body = body.push(
                    container(
                        column![
                            text(instance.project_path.as_deref().unwrap_or("Unity Editor"))
                                .size(11),
                            text(format!(
                                "{} · pid {}",
                                instance.editor_version.as_deref().unwrap_or("unknown"),
                                instance
                                    .pid
                                    .map(|pid| pid.to_string())
                                    .unwrap_or_else(|| "—".to_string())
                            ))
                            .size(9)
                            .style(theme::faint_text),
                        ]
                        .spacing(2),
                    )
                    .padding(8)
                    .width(Fill)
                    .style(theme::card),
                );
            }
        }
    } else if let Some(error) = &app.running.error {
        body = body.push(
            text(error.message.as_str())
                .size(10)
                .style(theme::muted_text),
        );
    }

    card("Running Editors", body.into())
}

fn card<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(column![text(title).size(14), body].spacing(10).width(Fill))
        .padding(18)
        .width(Fill)
        .style(theme::card)
        .into()
}
