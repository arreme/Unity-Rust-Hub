use iced::widget::{column, container, row, space};
use iced::{Center, Element, Fill, FillPortion};

use unity_hub_model::{
    AccountCardVm, AuthVm, CliCardVm, CliDetailsVm, LicenseVm, LoadState, OverviewVm,
    RunningEditorVm, Tone, UiAction, UpdateVm,
};

use crate::components::{
    badge, button, caption, card, card_with, detail_column, detail_mono, divided, divider,
    error_text, hint, icon_badge, icon_button, icon_tile, list_row, loading, mono, notice,
    page_header, refresh_button, stat_strip, status_label, ButtonKind,
};
use crate::fonts;
use crate::icons;
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::text;

pub fn view(vm: &OverviewVm) -> Element<'_, UiAction> {
    let title = match &vm.account.auth {
        AuthVm::SignedIn { name, .. } => match name.split_whitespace().next() {
            Some(first) => format!("Welcome back, {first}"),
            None => "Overview".to_string(),
        },
        _ => "Overview".to_string(),
    };

    column![
        page_header(
            title,
            "Your local Unity toolchain at a glance.",
            Some(refresh_button()),
        ),
        summary(vm),
        row![
            container(cli_card(vm)).width(FillPortion(2)),
            container(account_card(&vm.account)).width(FillPortion(1)),
        ]
        .spacing(spacing::SECTION)
        .align_y(iced::Alignment::Start),
        running_card(&vm.running),
    ]
    .spacing(spacing::SECTION)
    .into()
}

fn count(value: Option<usize>) -> String {
    value.map_or_else(|| "—".to_string(), |value| value.to_string())
}

fn summary(vm: &OverviewVm) -> Element<'_, UiAction> {
    let (cli_value, cli_label) = match &vm.cli {
        CliCardVm::Ready { details, .. } => {
            let state = match vm.update.data() {
                Some(update) if update.available => "update available",
                Some(_) => "up to date",
                None => "installed",
            };
            (details.version.clone(), format!("Unity CLI · {state}"))
        }
        CliCardVm::Missing => ("—".to_string(), "Unity CLI · not installed".to_string()),
        CliCardVm::Failed(_) => ("—".to_string(), "Unity CLI · unavailable".to_string()),
        CliCardVm::Idle | CliCardVm::Loading => ("…".to_string(), "Unity CLI".to_string()),
    };

    stat_strip(vec![
        (cli_value, cli_label, None),
        (
            count(vm.editor_count),
            "Editors installed".to_string(),
            None,
        ),
        (count(vm.project_count), "Projects".to_string(), None),
        (
            count(vm.running.data().map(Vec::len)),
            "Editors running".to_string(),
            None,
        ),
    ])
}

fn cli_card(vm: &OverviewVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(spacing::CARD_GAP);
    let mut trailing = row![].spacing(8).align_y(Center);

    match &vm.cli {
        CliCardVm::Idle => {}
        CliCardVm::Loading => {
            body = body.push(loading("Resolving the Unity CLI…"));
        }
        CliCardVm::Missing => {
            body = body.push(install_callout());
        }
        CliCardVm::Failed(error) => {
            body = body.push(error_text(error));
        }
        CliCardVm::Ready { details, error } => {
            trailing = trailing
                .push(badge(details.channel.clone(), Tone::Neutral))
                .push(update_badge(&vm.update));

            body = body.push(cli_details(details));

            if let Some(update) = vm.update.data() {
                if update.available {
                    body = body.push(update_callout(update));
                }
            }

            if let Some(error) = error {
                body = body.push(error_text(error));
            }
            if let Some(error) = vm.update.error() {
                body = body.push(
                    text(format!("Update check failed: {}", error.text()))
                        .size(font::SMALL)
                        .style(theme::danger_text),
                );
            }

            body = body.push(divider());
            body = body.push(
                row![
                    button("Re-check", ButtonKind::Secondary).on_press(UiAction::Refresh),
                    button("Open settings", ButtonKind::Ghost)
                        .on_press(UiAction::Navigate(unity_hub_model::Page::Settings)),
                ]
                .spacing(8),
            );
        }
    }

    card_with("Unity CLI", trailing.into(), body.into())
}

fn cli_details(details: &CliDetailsVm) -> Element<'_, UiAction> {
    column![
        text(details.version.as_str())
            .size(font::DISPLAY)
            .font(fonts::SEMIBOLD),
        row![
            detail_column("Platform", details.platform.as_str()),
            detail_column("Runtime", details.runtime.as_str()),
        ]
        .spacing(24),
        row![
            detail_mono("Executable", details.executable.as_str()),
            detail_column("Resolved via", details.resolved_via.as_str()),
        ]
        .spacing(24),
        row![
            detail_mono("Editor install path", details.install_path.as_str()),
            detail_column("Build date", details.build_date.as_str()),
        ]
        .spacing(24),
    ]
    .spacing(16)
    .into()
}

fn update_badge(update: &LoadState<UpdateVm>) -> Element<'static, UiAction> {
    if update.is_loading() {
        icon_badge(icons::LOADER, "checking updates", Tone::Neutral)
    } else if update.error().is_some() {
        badge("update check failed", Tone::Neutral)
    } else if let Some(update) = update.data() {
        if update.available {
            icon_badge(icons::CIRCLE_ARROW_UP, "update available", Tone::Warning)
        } else {
            icon_badge(icons::CHECK, "up to date", Tone::Success)
        }
    } else {
        badge("unknown", Tone::Neutral)
    }
}

fn update_callout(update: &UpdateVm) -> Element<'static, UiAction> {
    notice(
        Tone::Warning,
        icons::CIRCLE_ARROW_UP,
        row![
            column![
                text("A newer CLI is available.")
                    .size(font::BODY_LARGE)
                    .font(fonts::MEDIUM),
                text(format!(
                    "{} → {}",
                    update.current_version, update.latest_version
                ))
                .size(font::MONO)
                .font(fonts::MONO)
                .style(theme::muted_text),
            ]
            .spacing(3)
            .width(Fill),
            icon_button(icons::DOWNLOAD, "Update CLI", ButtonKind::Primary)
                .on_press(UiAction::UpdateCli),
        ]
        .spacing(12)
        .align_y(Center),
    )
}

fn install_callout() -> Element<'static, UiAction> {
    notice(
        Tone::Warning,
        icons::TRIANGLE_ALERT,
        row![
            column![
                text("The Unity CLI is not installed.")
                    .size(font::BODY_LARGE)
                    .font(fonts::MEDIUM),
                text("This app is a native GUI over the official unity binary. Install it to manage Editors, projects, and release notes.")
                    .size(font::BODY)
                    .style(theme::muted_text),
            ]
            .spacing(4)
            .width(Fill),
            icon_button(icons::DOWNLOAD, "Install Unity CLI", ButtonKind::Primary)
                .on_press(UiAction::OpenInstallCli),
        ]
        .spacing(14)
        .align_y(Center),
    )
}

fn avatar(name: &str) -> Element<'static, UiAction> {
    let initials = name
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();

    let content: Element<'static, UiAction> = if initials.is_empty() {
        crate::icons::icon(icons::USER, font::ICON).into()
    } else {
        text(initials)
            .size(font::BODY_LARGE)
            .font(fonts::SEMIBOLD)
            .into()
    };

    container(content)
        .width(44)
        .height(44)
        .center(44)
        .style(theme::avatar)
        .into()
}

fn account_card(account: &AccountCardVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(16);

    match &account.auth {
        AuthVm::Idle => {}
        AuthVm::Loading => {
            body = body.push(loading("Checking sign-in…"));
        }
        AuthVm::SignedIn { name, email, stale } => {
            body = body.push(
                row![
                    avatar(name),
                    column![
                        text(name.as_str())
                            .size(font::BODY_LARGE)
                            .font(fonts::MEDIUM),
                        text(email.as_str())
                            .size(font::SMALL)
                            .style(theme::faint_text),
                    ]
                    .spacing(2),
                ]
                .spacing(12)
                .align_y(Center),
            );

            if *stale {
                body = body.push(
                    row![
                        text(
                            "Your session is still valid, but it needs a refresh. \
                             Sign in again if a Unity command asks for it."
                        )
                        .size(font::SMALL)
                        .style(theme::warning_text)
                        .width(Fill),
                        icon_button(icons::LOG_IN, "Sign in", ButtonKind::Secondary)
                            .on_press(UiAction::SignIn),
                    ]
                    .spacing(10)
                    .align_y(Center),
                );
            }
        }
        AuthVm::SignedOut { expired } => {
            let message = if *expired {
                "Your session expired. Sign in again."
            } else {
                "Not signed in."
            };

            body = body.push(
                row![
                    text(message)
                        .size(font::BODY)
                        .style(theme::muted_text)
                        .width(Fill),
                    icon_button(icons::LOG_IN, "Sign in", ButtonKind::Primary)
                        .on_press(UiAction::SignIn),
                ]
                .spacing(10)
                .align_y(Center),
            );
        }
        AuthVm::Failed(error) => {
            body = body.push(hint(format!("Sign-in unavailable: {}", error.message)));
        }
    }

    body = body.push(divider());

    match &account.license {
        LicenseVm::Idle => {}
        LicenseVm::Failed(error) => {
            body = body.push(hint(format!("License unavailable: {}", error.message)));
        }
        LicenseVm::Ready { active, licenses } => {
            body = body.push(
                row![
                    container(caption("Licenses")).width(Fill),
                    if *active {
                        status_label("active", Tone::Success)
                    } else {
                        status_label("inactive", Tone::Danger)
                    },
                ]
                .align_y(Center),
            );

            if licenses.is_empty() {
                body = body.push(hint("No licenses reported."));
            } else {
                for license in licenses {
                    body = body.push(
                        row![
                            text(license.product.as_str())
                                .size(font::BODY_LARGE)
                                .font(fonts::MEDIUM)
                                .width(Fill),
                            badge(license.kind.clone(), Tone::Neutral),
                        ]
                        .spacing(8)
                        .align_y(Center),
                    );
                }
            }
        }
    }

    card("Account", body.into())
}

/// The last path component, which is the project's folder name.
fn project_name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
}

fn running_row(instance: &RunningEditorVm) -> Element<'_, UiAction> {
    let (glyph, tone) = if instance.dialog.is_some() || instance.safe_mode {
        (icons::TRIANGLE_ALERT, Tone::Warning)
    } else {
        (icons::PLAY, Tone::Success)
    };

    let mut details = row![column![
        text(project_name(&instance.project_path))
            .size(font::BODY_LARGE)
            .font(fonts::MEDIUM),
        mono(format!(
            "{} · {} · pid {}",
            instance.project_path, instance.editor_version, instance.pid
        )),
    ]
    .spacing(2)
    .width(Fill)]
    .spacing(10)
    .align_y(Center);

    if let Some(dialog) = &instance.dialog {
        details = details.push(
            text(format!("Waiting on dialog: {dialog}"))
                .size(font::SMALL)
                .style(theme::warning_text),
        );
    } else if let Some(phase) = &instance.phase {
        details = details.push(
            text(format!("Phase: {phase}"))
                .size(font::SMALL)
                .style(theme::muted_text),
        );
    }
    if let Some(version) = &instance.pipeline_version {
        details = details.push(badge(format!("Pipeline {version}"), Tone::Highlight));
    }
    if instance.update_available {
        details = details.push(badge("update available", Tone::Warning));
    }
    if instance.safe_mode {
        details = details.push(badge("Safe mode", Tone::Danger));
    }

    list_row(
        row![icon_tile(glyph, tone, 40.0), details]
            .spacing(16)
            .align_y(Center),
    )
}

fn running_card(running: &LoadState<Vec<RunningEditorVm>>) -> Element<'_, UiAction> {
    let body: Element<'_, UiAction> = if running.is_loading() && running.data().is_none() {
        loading("Checking running Editors…")
    } else if let Some(instances) = running.data() {
        if instances.is_empty() {
            row![
                icon_tile(icons::MONITOR, Tone::Neutral, 40.0),
                column![
                    text("No Editor running.")
                        .size(font::BODY_LARGE)
                        .font(fonts::MEDIUM),
                    hint("Open a project with the Pipeline package to see live Editors here."),
                ]
                .spacing(2),
            ]
            .spacing(16)
            .align_y(Center)
            .into()
        } else {
            column![
                divider(),
                divided(instances.iter().map(running_row).collect()),
            ]
            .into()
        }
    } else if let Some(error) = running.error() {
        hint(error.message.as_str())
    } else {
        space().height(0).into()
    };

    card_with("Running Editors", hint("From unity editors running"), body)
}
