use iced::widget::{button, column, container, row, scrollable, space};
use iced::{Center, Element, Fill};

use unity_hub_model::{CliConnectionVm, JobPhase, JobVm, NoticeVm, Page, UiAction, ViewModel};

use crate::components::badge;
use crate::pages;
use crate::theme;
use crate::ui::text;

pub fn view(vm: &ViewModel) -> Element<'_, UiAction> {
    let mut content = column![].spacing(16).width(Fill).padding(24);

    if let Some(notice) = &vm.shell.notice {
        content = content.push(notice_view(notice));
    }

    content = content.push(pages::view(&vm.page));

    if let Some(job) = &vm.shell.job {
        content = content.push(job_view(job));
    }

    let main = column![topbar(vm), scrollable(content).height(Fill).width(Fill)]
        .width(Fill)
        .height(Fill);

    row![sidebar(vm), main].height(Fill).width(Fill).into()
}

fn sidebar(vm: &ViewModel) -> Element<'_, UiAction> {
    let brand = row![
        container(text("U").size(18).style(theme::accent_text))
            .width(38)
            .height(38)
            .center(38)
            .style(theme::badge(unity_hub_model::Tone::Accent)),
        column![
            text("Unity Rust Hub").size(14),
            text("Native · iced").size(10).style(theme::faint_text),
        ]
        .spacing(1),
    ]
    .spacing(10)
    .align_y(Center);

    let nav = column(
        Page::ALL
            .iter()
            .map(|page| {
                button(text(page_label(*page)).size(13))
                    .width(Fill)
                    .padding([10.0, 12.0])
                    .style(theme::nav_button(vm.shell.page == *page))
                    .on_press(UiAction::Navigate(*page))
                    .into()
            })
            .collect::<Vec<Element<'_, UiAction>>>(),
    )
    .spacing(4);

    let (dot_style, status, version): (
        fn(&iced::Theme) -> iced::widget::text::Style,
        &str,
        String,
    ) = match &vm.shell.cli {
        CliConnectionVm::Checking => (
            theme::muted_text,
            "Checking CLI…",
            "not detected".to_string(),
        ),
        CliConnectionVm::Connected { version } => (
            theme::success_text,
            "CLI connected",
            version
                .clone()
                .unwrap_or_else(|| "not detected".to_string()),
        ),
        CliConnectionVm::Missing => (
            theme::danger_text,
            "CLI missing",
            "not detected".to_string(),
        ),
    };

    let footer = container(
        column![
            row![
                container(text("●").size(9).style(dot_style)),
                text(status).size(11),
            ]
            .spacing(6)
            .align_y(Center),
            text(version).size(10).style(theme::faint_text),
        ]
        .spacing(3),
    )
    .padding(10)
    .width(Fill)
    .style(theme::card);

    container(
        column![
            brand,
            nav,
            space().height(Fill),
            footer,
            text("All mutations go through the official Unity CLI.")
                .size(9)
                .style(theme::faint_text),
        ]
        .spacing(18)
        .width(Fill)
        .height(Fill),
    )
    .width(232)
    .height(Fill)
    .padding(16)
    .style(theme::sidebar)
    .into()
}

fn topbar(vm: &ViewModel) -> Element<'_, UiAction> {
    let cli_badge = match &vm.shell.cli {
        CliConnectionVm::Checking => badge("checking CLI…", unity_hub_model::Tone::Neutral),
        CliConnectionVm::Connected {
            version: Some(version),
        } => badge(version.clone(), unity_hub_model::Tone::Success),
        CliConnectionVm::Connected { version: None } | CliConnectionVm::Missing => {
            badge("CLI not found", unity_hub_model::Tone::Danger)
        }
    };

    container(
        row![
            text(page_title(vm.shell.page)).size(17),
            space().width(Fill),
            cli_badge,
            button(text("Refresh").size(11))
                .on_press(UiAction::Refresh)
                .padding([6.0, 10.0])
                .style(button::text),
        ]
        .spacing(10)
        .align_y(Center),
    )
    .padding([12.0, 22.0])
    .width(Fill)
    .style(theme::topbar)
    .into()
}

fn notice_view(notice: &NoticeVm) -> Element<'_, UiAction> {
    container(
        row![
            text(notice.text.as_str()).size(12).width(Fill),
            button(text("Dismiss").size(10))
                .on_press(UiAction::DismissNotice)
                .padding([4.0, 8.0])
                .style(button::text),
        ]
        .spacing(10)
        .align_y(Center),
    )
    .padding([10.0, 12.0])
    .width(Fill)
    .style(theme::notice_card(notice.tone))
    .into()
}

fn job_view(job: &JobVm) -> Element<'_, UiAction> {
    let state_tone = match job.phase {
        JobPhase::Succeeded => unity_hub_model::Tone::Success,
        JobPhase::Failed => unity_hub_model::Tone::Danger,
        JobPhase::Cancelled => unity_hub_model::Tone::Warning,
        JobPhase::Running => unity_hub_model::Tone::Info,
    };
    let running = job.phase == JobPhase::Running;

    let header = row![
        text(job.kind.as_str()).size(13).width(Fill),
        badge(job_phase_label(job.phase), state_tone),
        if running {
            Element::from(
                button(text("Cancel").size(10))
                    .on_press(UiAction::CancelJob)
                    .padding([4.0, 8.0])
                    .style(button::danger),
            )
        } else {
            Element::from(
                button(text("Dismiss").size(10))
                    .on_press(UiAction::DismissJob)
                    .padding([4.0, 8.0])
                    .style(button::text),
            )
        },
    ]
    .spacing(8)
    .align_y(Center);

    let mut body = column![header].spacing(8);

    if running {
        body = body.push(
            iced::widget::progress_bar(0.0..=100.0, job.progress)
                .length(Fill)
                .girth(6.0),
        );
    }

    if let Some(message) = &job.message {
        body = body.push(text(message.as_str()).size(11).style(theme::muted_text));
    }

    if let Some(error) = &job.error {
        body = body.push(text(error.text()).size(11).style(theme::danger_text));
    }

    body = body.push(
        text(job.command.as_str())
            .size(9)
            .font(iced::Font::MONOSPACE)
            .style(theme::faint_text),
    );

    if !job.logs.is_empty() {
        let mut log_text = String::new();
        for line in &job.logs {
            if line.is_stderr {
                log_text.push_str("! ");
            }
            log_text.push_str(&line.text);
            log_text.push('\n');
        }

        body = body.push(
            container(
                scrollable(
                    text(log_text)
                        .size(9)
                        .font(iced::Font::MONOSPACE)
                        .style(theme::muted_text),
                )
                .height(150),
            )
            .padding(8)
            .width(Fill)
            .style(theme::card),
        );
    }

    container(body)
        .padding(14)
        .width(Fill)
        .style(theme::job_card)
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

pub fn page_label(page: Page) -> &'static str {
    match page {
        Page::Overview => "Overview",
        Page::Editors => "Editors",
        Page::Projects => "Projects",
        Page::Changelog => "Changelog",
        Page::Settings => "Settings",
    }
}

pub fn page_title(page: Page) -> &'static str {
    match page {
        Page::Overview => "Overview",
        Page::Editors => "Editors & modules",
        Page::Projects => "Projects",
        Page::Changelog => "Changelog & breaking changes",
        Page::Settings => "Settings",
    }
}
