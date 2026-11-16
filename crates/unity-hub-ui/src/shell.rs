use iced::widget::{button, column, container, row, space};
use iced::{Center, Element, Fill, Padding};

use unity_hub_model::{
    CliConnectionVm, JobPhase, JobVm, NotificationActionState, NotificationVm, Page, ToastVm, Tone,
    UiAction, ViewModel,
};

use crate::components::{self, badge, icon_tile, mono_wrapping, scroll, status_dot, ButtonKind};
use crate::fonts;
use crate::icons::{self, icon};
use crate::pages;
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::text;

const SIDEBAR_WIDTH: f32 = 228.0;
const RAIL_WIDTH: f32 = 320.0;

/// Sidebar plus a scrolling page, with the notification rail on the right when
/// it is open. Each page draws its own title, so there is no top bar.
///
/// The rail is a third column and the toast sits inline above the page: the
/// tiny-skia fallback miscomposites stacked layers, so nothing is overlaid.
pub fn view(vm: &ViewModel) -> Element<'_, UiAction> {
    let mut content = column![].spacing(spacing::SECTION).width(Fill);

    content = content.push(pages::view(&vm.page));

    if let Some(job) = &vm.shell.job {
        content = content.push(job_view(job));
    }

    let mut body = column![].spacing(spacing::SECTION).width(Fill).height(Fill);

    if let Some(toast) = &vm.shell.toast {
        body = body.push(toast_view(toast));
    }

    body = body.push(scroll(content).height(Fill).width(Fill));

    let main = container(body)
        .width(Fill)
        .height(Fill)
        .padding(Padding {
            top: spacing::PAGE_TOP,
            right: spacing::PAGE_X,
            bottom: 48.0,
            left: spacing::PAGE_X,
        })
        .style(theme::page);

    let mut root = row![sidebar(vm), vertical_line(), main]
        .height(Fill)
        .width(Fill);

    if vm.shell.rail_open {
        root = root.push(vertical_line()).push(rail(vm));
    }

    root.into()
}

fn vertical_line<'a>() -> Element<'a, UiAction> {
    container(space())
        .width(1)
        .height(Fill)
        .style(theme::vertical_line)
        .into()
}

fn sidebar(vm: &ViewModel) -> Element<'_, UiAction> {
    let brand = row![
        icon_tile(icons::BOX, Tone::Neutral, 36.0),
        column![
            text("Unity Rust Hub")
                .size(font::BODY_LARGE)
                .font(fonts::semibold()),
            text(concat!("v", env!("CARGO_PKG_VERSION")))
                .size(font::SMALL)
                .style(theme::faint_text),
        ]
        .spacing(1),
    ]
    .spacing(12)
    .align_y(Center);

    let nav_item = |page: Page| -> Element<'_, UiAction> {
        let active = vm.shell.page == page;

        button(
            row![
                icon(page_icon(page), font::ICON),
                text(page_label(page))
                    .size(font::BODY)
                    .font(fonts::regular()),
            ]
            .spacing(12)
            .align_y(Center),
        )
        .width(Fill)
        .padding([10.0, 12.0])
        .style(theme::nav_button(active))
        .on_press(UiAction::Navigate(page))
        .into()
    };

    let primary_nav = column(
        Page::ALL
            .iter()
            .filter(|page| **page != Page::Settings)
            .map(|page| nav_item(*page))
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    let mut notifications_label = row![
        icon(icons::CIRCLE_DOT, font::ICON),
        text("Notifications")
            .size(font::BODY)
            .font(fonts::regular()),
        space().width(Fill),
    ]
    .spacing(12)
    .align_y(Center);

    if vm.shell.unread > 0 {
        notifications_label =
            notifications_label.push(badge(vm.shell.unread.to_string(), Tone::Danger));
    }

    let notifications_item = button(notifications_label)
        .width(Fill)
        .padding([10.0, 12.0])
        .style(theme::nav_button(vm.shell.rail_open))
        .on_press(UiAction::ToggleNotificationRail);

    let (tone, status, version): (Tone, &str, String) = match &vm.shell.cli {
        CliConnectionVm::Checking => (Tone::Neutral, "Checking CLI…", "—".to_string()),
        CliConnectionVm::Connected { version } => (
            Tone::Success,
            "CLI connected",
            version
                .clone()
                .unwrap_or_else(|| "unknown version".to_string()),
        ),
        CliConnectionVm::Missing => (Tone::Danger, "CLI missing", "not detected".to_string()),
    };

    let footer = container(
        row![
            status_dot(tone),
            column![
                text(status).size(font::SMALL).font(fonts::medium()),
                text(version)
                    .size(font::CAPTION)
                    .font(fonts::mono())
                    .style(theme::faint_text),
            ]
            .spacing(1),
        ]
        .spacing(10)
        .align_y(Center),
    )
    .padding([12.0, 14.0])
    .width(Fill)
    .style(theme::outline);

    container(
        column![
            container(brand).padding(Padding {
                top: 0.0,
                right: 8.0,
                bottom: 20.0,
                left: 8.0,
            }),
            primary_nav,
            space().height(Fill),
            notifications_item,
            nav_item(Page::Settings),
            space().height(8),
            footer,
        ]
        .spacing(4)
        .width(Fill)
        .height(Fill),
    )
    .width(SIDEBAR_WIDTH)
    .height(Fill)
    .padding([24.0, 16.0])
    .style(theme::sidebar)
    .into()
}

fn rail(vm: &ViewModel) -> Element<'_, UiAction> {
    let shell = &vm.shell;

    let mut header = row![
        text("Notifications")
            .size(font::CARD_TITLE)
            .font(fonts::semibold()),
        space().width(Fill),
    ]
    .spacing(spacing::SMALL)
    .align_y(Center);

    if shell.unread > 0 {
        header = header.push(badge(shell.unread.to_string(), Tone::Danger));
    }

    if !shell.notifications.is_empty() {
        header = header.push(
            components::button("Clear all", ButtonKind::Ghost)
                .on_press(UiAction::ClearNotifications)
                .padding(components::COMPACT),
        );
    }

    let body: Element<'_, UiAction> = if shell.notifications.is_empty() {
        components::empty(
            "No notifications.",
            "CLI advisories and app events appear here.",
        )
    } else {
        let mut list = column![].spacing(spacing::SMALL).width(Fill);

        for notification in &shell.notifications {
            list = list.push(notification_card(notification));
        }

        scroll(list).height(Fill).width(Fill).into()
    };

    container(
        column![header, body]
            .spacing(spacing::CARD_GAP)
            .width(Fill)
            .height(Fill),
    )
    .width(RAIL_WIDTH)
    .height(Fill)
    .padding([spacing::SECTION, spacing::SECTION])
    .style(theme::sidebar)
    .into()
}

fn notification_card(notification: &NotificationVm) -> Element<'_, UiAction> {
    let tone = notification.tone;

    let header = row![
        icon(tone_glyph(tone), font::ICON_SMALL).style(theme::tone_text(tone)),
        container(mono_wrapping(notification.label.as_str())).width(Fill),
        components::icon_only(icons::X, ButtonKind::Ghost)
            .on_press(UiAction::DismissNotification(notification.id))
            .padding([2.0, 4.0]),
    ]
    .spacing(spacing::SMALL)
    .align_y(Center);

    let mut body = column![
        header,
        text(notification.message.as_str())
            .size(font::BODY)
            .width(Fill),
    ]
    .spacing(spacing::SMALL)
    .width(Fill);

    if let Some(command) = &notification.command {
        body = body.push(
            container(
                text(command.as_str())
                    .size(font::MONO)
                    .font(fonts::mono())
                    .style(theme::muted_text)
                    .width(Fill),
            )
            .padding([spacing::SMALL, 10.0])
            .width(Fill)
            .style(theme::inset),
        );

        if let Some(actions) = notification_actions(notification) {
            body = body.push(actions);
        }
    }

    container(body)
        .padding(spacing::SMALL)
        .width(Fill)
        .style(theme::notice_card(tone))
        .into()
}

/// The action row for an entry that carries a command, or `None` when there is
/// nothing the user may do beyond the (absent) command itself.
fn notification_actions(notification: &NotificationVm) -> Option<Element<'_, UiAction>> {
    notification.command.as_ref()?;

    let mut actions = row![
        components::icon_button(icons::COPY, "Copy", ButtonKind::Secondary)
            .on_press(UiAction::CopyNotificationCommand(notification.id))
            .padding(components::COMPACT),
    ]
    .spacing(spacing::SMALL)
    .align_y(Center);

    // The confirmation row replaces `run` under the primary one: three pills do
    // not fit the rail width at every text scale.
    let mut confirmation = row![].spacing(spacing::SMALL).align_y(Center);
    let mut confirming = false;

    match notification.state {
        NotificationActionState::Idle => {
            if notification.can_run {
                actions = actions.push(
                    components::button("Run", ButtonKind::Primary)
                        .on_press(UiAction::RunNotificationCommand(notification.id))
                        .padding(components::COMPACT),
                );
            }
        }
        NotificationActionState::Confirming => {
            actions = actions.push(
                text("Run this command?")
                    .size(font::SMALL)
                    .style(theme::muted_text)
                    .width(Fill),
            );
            confirmation = confirmation
                .push(
                    components::button("Confirm", ButtonKind::Primary)
                        .on_press(UiAction::ConfirmNotificationCommand(notification.id))
                        .padding(components::COMPACT),
                )
                .push(
                    components::button("Cancel", ButtonKind::Secondary)
                        .on_press(UiAction::CancelNotificationCommand(notification.id))
                        .padding(components::COMPACT),
                );
            confirming = true;
        }
        NotificationActionState::Running => {
            actions = actions.push(components::loading("Running…"));
        }
        NotificationActionState::Failed => {
            actions = actions.push(badge("Failed", Tone::Danger));

            if notification.can_run {
                actions = actions.push(
                    components::button("Run", ButtonKind::Primary)
                        .on_press(UiAction::RunNotificationCommand(notification.id))
                        .padding(components::COMPACT),
                );
            }
        }
    }

    let mut buttons = column![actions].spacing(spacing::SMALL).width(Fill);

    if confirming {
        buttons = buttons.push(confirmation);
    }

    Some(buttons.into())
}

fn toast_view(toast: &ToastVm) -> Element<'_, UiAction> {
    components::notice(
        toast.tone,
        tone_glyph(toast.tone),
        row![
            text(toast.message.as_str()).size(font::BODY).width(Fill),
            components::icon_only(icons::X, ButtonKind::Ghost)
                .on_press(UiAction::DismissToast(toast.id))
                .padding([2.0, 4.0]),
        ]
        .spacing(spacing::SMALL)
        .align_y(iced::Alignment::Start),
    )
}

/// The glyph a tone uses in the rail and the toast.
fn tone_glyph(tone: Tone) -> char {
    match tone {
        Tone::Success => icons::CIRCLE_CHECK,
        Tone::Warning => icons::TRIANGLE_ALERT,
        Tone::Danger => icons::CIRCLE_ALERT,
        _ => icons::INFO,
    }
}

fn job_view(job: &JobVm) -> Element<'_, UiAction> {
    let (tone, glyph) = match job.phase {
        JobPhase::Succeeded => (Tone::Success, icons::CIRCLE_CHECK),
        JobPhase::Failed => (Tone::Danger, icons::CIRCLE_X),
        JobPhase::Cancelled => (Tone::Warning, icons::BAN),
        JobPhase::Running => (Tone::Neutral, icons::LOADER),
    };
    let running = job.phase == JobPhase::Running;

    let action: Element<'_, UiAction> = if running {
        components::button("Cancel", ButtonKind::Danger)
            .on_press(UiAction::CancelJob)
            .padding(components::COMPACT)
            .into()
    } else {
        components::button("Dismiss", ButtonKind::Secondary)
            .on_press(UiAction::DismissJob)
            .padding(components::COMPACT)
            .into()
    };

    let header = row![
        icon_tile(glyph, tone, 40.0),
        column![
            text(job.kind.as_str())
                .size(font::CARD_TITLE)
                .font(fonts::semibold()),
            text(job.command.as_str())
                .size(font::MONO)
                .font(fonts::mono())
                .style(theme::faint_text),
        ]
        .spacing(2)
        .width(Fill),
        badge(job_phase_label(job.phase), tone),
        action,
    ]
    .spacing(14)
    .align_y(Center);

    let mut body = column![header].spacing(16);

    if running {
        body = body.push(
            row![
                iced::widget::progress_bar(0.0..=100.0, job.progress)
                    .length(Fill)
                    .girth(6.0)
                    .style(theme::progress_bar),
                text(format!("{:.0}%", job.progress))
                    .size(font::SMALL)
                    .font(fonts::mono())
                    .style(theme::muted_text),
            ]
            .spacing(12)
            .align_y(Center),
        );
    }

    if let Some(message) = &job.message {
        body = body.push(
            text(message.as_str())
                .size(font::BODY)
                .style(theme::muted_text),
        );
    }

    if let Some(error) = &job.error {
        body = body.push(components::error_text(error));
    }

    if !job.logs.is_empty() {
        body = body.push(
            container(
                scroll(
                    text(job.logs.as_ref())
                        .size(font::MONO)
                        .font(fonts::mono())
                        .line_height(1.55)
                        .style(theme::muted_text),
                )
                .height(160),
            )
            .padding([12.0, 14.0])
            .width(Fill)
            .style(theme::inset),
        );
    }

    container(body)
        .padding(spacing::CARD)
        .width(Fill)
        .style(theme::card)
        .into()
}

fn job_phase_label(phase: JobPhase) -> &'static str {
    match phase {
        JobPhase::Running => "running",
        JobPhase::Succeeded => "succeeded",
        JobPhase::Failed => "failed",
        JobPhase::Cancelled => "cancelled",
    }
}

fn page_icon(page: Page) -> char {
    match page {
        Page::Overview => icons::DASHBOARD,
        Page::Editors => icons::BOX,
        Page::Projects => icons::FOLDERS,
        Page::Templates => icons::PACKAGE,
        Page::Processes => icons::CPU,
        Page::Changelog => icons::NEWSPAPER,
        Page::Settings => icons::SETTINGS,
    }
}

pub fn page_label(page: Page) -> &'static str {
    match page {
        Page::Overview => "Overview",
        Page::Editors => "Editors",
        Page::Projects => "Projects",
        Page::Templates => "Templates",
        Page::Processes => "Processes",
        Page::Changelog => "Changelog",
        Page::Settings => "Settings",
    }
}
