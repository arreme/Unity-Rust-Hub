use iced::widget::{button, column, container, row, space};
use iced::{Center, Element, Fill, Padding};

use unity_hub_model::{
    CliConnectionVm, JobPhase, JobVm, NoticeVm, Page, Tone, UiAction, ViewModel,
};

use crate::components::{self, badge, icon_tile, notice, scroll, status_dot, ButtonKind};
use crate::fonts;
use crate::icons::{self, icon};
use crate::pages;
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::text;

const SIDEBAR_WIDTH: f32 = 228.0;

/// Sidebar plus a scrolling page. Each page draws its own title, so there is
/// no top bar.
pub fn view(vm: &ViewModel) -> Element<'_, UiAction> {
    let mut content = column![]
        .spacing(spacing::SECTION)
        .width(Fill)
        .padding(Padding {
            top: spacing::PAGE_TOP,
            right: spacing::PAGE_X,
            bottom: 48.0,
            left: spacing::PAGE_X,
        });

    if let Some(notice) = &vm.shell.notice {
        content = content.push(notice_view(notice));
    }

    content = content.push(pages::view(&vm.page));

    if let Some(job) = &vm.shell.job {
        content = content.push(job_view(job));
    }

    let main = container(scroll(content).height(Fill).width(Fill))
        .width(Fill)
        .height(Fill)
        .style(theme::page);

    row![
        sidebar(vm),
        container(space())
            .width(1)
            .height(Fill)
            .style(theme::vertical_line),
        main
    ]
    .height(Fill)
    .width(Fill)
    .into()
}

fn sidebar(vm: &ViewModel) -> Element<'_, UiAction> {
    let brand = row![
        icon_tile(icons::BOX, Tone::Neutral, 36.0),
        column![
            text("Unity Rust Hub")
                .size(font::BODY_LARGE)
                .font(fonts::SEMIBOLD),
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
                text(page_label(page)).size(font::BODY).font(if active {
                    fonts::MEDIUM
                } else {
                    fonts::SANS
                }),
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
                text(status).size(font::SMALL).font(fonts::MEDIUM),
                text(version)
                    .size(font::CAPTION)
                    .font(fonts::MONO)
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

fn notice_view(notice_vm: &NoticeVm) -> Element<'_, UiAction> {
    let glyph = match notice_vm.tone {
        Tone::Success => icons::CIRCLE_CHECK,
        Tone::Danger => icons::CIRCLE_ALERT,
        Tone::Warning => icons::TRIANGLE_ALERT,
        _ => icons::INFO,
    };

    notice(
        notice_vm.tone,
        glyph,
        row![
            text(notice_vm.text.as_str()).size(font::BODY).width(Fill),
            button(icon(icons::X, font::ICON_SMALL))
                .on_press(UiAction::DismissNotice)
                .padding(6)
                .style(theme::button_ghost),
        ]
        .spacing(10)
        .align_y(Center),
    )
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
                .font(fonts::SEMIBOLD),
            text(job.command.as_str())
                .size(font::MONO)
                .font(fonts::MONO)
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
                    .font(fonts::MONO)
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
                        .font(fonts::MONO)
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
        Page::Changelog => icons::NEWSPAPER,
        Page::Settings => icons::SETTINGS,
    }
}

pub fn page_label(page: Page) -> &'static str {
    match page {
        Page::Overview => "Overview",
        Page::Editors => "Editors",
        Page::Projects => "Projects",
        Page::Changelog => "Changelog",
        Page::Settings => "Settings",
    }
}
