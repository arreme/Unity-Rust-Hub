use iced::widget::{column, container, row, space};
use iced::{Center, Element, Fill};

use unity_hub_model::{ProcessRowVm, ProcessesVm, Tone, UiAction};

use crate::components::{
    badge, checkbox, divided, empty, error_text, hint, icon_button, icon_only, icon_tile, list_row,
    loading, mono, page_header, refresh_button, ButtonKind,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::text;

pub fn view(vm: &ProcessesVm) -> Element<'_, UiAction> {
    let actions = row![
        checkbox(
            "Show CPU and memory",
            vm.metrics_enabled,
            UiAction::ProcessMetricsToggled,
        ),
        refresh_button(),
    ]
    .spacing(14)
    .align_y(Center);

    let subtitle = match &vm.last_refreshed {
        Some(time) => format!("Read-only view of connected Editors. Last refresh at {time}."),
        None => "Read-only view of connected Editors.".to_string(),
    };

    column![
        page_header("Processes", subtitle, Some(actions.into())),
        processes_card(vm),
    ]
    .spacing(spacing::SECTION)
    .into()
}

fn processes_card(vm: &ProcessesVm) -> Element<'_, UiAction> {
    let body: Element<'_, UiAction> = if vm.rows.is_loading() && vm.rows.data().is_none() {
        loading("Reading running Editors…")
    } else if let Some(error) = vm.rows.error() {
        column![
            error_text(error),
            row![
                icon_button(icons::REFRESH, "Retry", ButtonKind::Secondary)
                    .on_press(UiAction::RefreshProcesses),
                hint("`unity status` needs the Pipeline package for live state.")
            ]
            .spacing(12)
            .align_y(Center),
        ]
        .spacing(12)
        .into()
    } else if let Some(rows) = vm.rows.data() {
        if rows.is_empty() {
            empty(
                "No Unity Editor is running",
                "Open a project and this page will list its Editor, PID, port, and live state.",
            )
        } else {
            let mut content = column![];
            if !vm.pipeline_available {
                content = content.push(
                    container(
                        row![
                            icon(icons::INFO, font::ICON_SMALL).style(theme::warning_text),
                            text("The Pipeline package is not reporting. Phase, Play mode, frames, and dialogs may be unavailable.")
                                .size(font::SMALL)
                                .style(theme::muted_text)
                                .width(Fill),
                        ]
                        .spacing(8)
                        .align_y(Center),
                    )
                    .padding([10.0, 14.0])
                    .width(Fill)
                    .style(theme::raised),
                );
            }
            content = content.push(divided(
                rows.iter()
                    .map(|row| process_row(row, vm.metrics_enabled))
                    .collect(),
            ));
            content.into()
        }
    } else {
        space().height(0).into()
    };

    container(body)
        .padding([0.0, spacing::CARD])
        .width(Fill)
        .style(theme::card)
        .into()
}

fn process_row(instance: &ProcessRowVm, metrics_enabled: bool) -> Element<'_, UiAction> {
    let (glyph, tone) = if instance.safe_mode || instance.dialog.is_some() {
        (icons::TRIANGLE_ALERT, Tone::Warning)
    } else if instance.state.eq_ignore_ascii_case("unreachable") {
        (icons::SHIELD_ALERT, Tone::Danger)
    } else if instance.play_mode {
        (icons::PLAY, Tone::Success)
    } else {
        (icons::CPU, Tone::Neutral)
    };

    let title = row![
        text(instance.project_name.as_str())
            .size(font::BODY_LARGE)
            .font(fonts::semibold()),
        badge(instance.state.clone(), instance.state_tone),
        if instance.play_mode {
            badge("Play mode", Tone::Success)
        } else {
            badge("Edit mode", Tone::Neutral)
        },
    ]
    .spacing(8)
    .align_y(Center);

    let mut meta = row![mono(instance.project_path.as_str())]
        .spacing(10)
        .align_y(Center);
    meta = meta.push(
        text(format!("Unity {}", instance.editor_version))
            .size(font::SMALL)
            .style(theme::faint_text),
    );

    let mut details = column![title, meta].spacing(4).width(Fill);

    let mut state_line = row![].spacing(12).align_y(Center);
    if let Some(phase) = &instance.phase {
        state_line = state_line.push(
            text(format!("Phase: {phase}"))
                .size(font::SMALL)
                .style(theme::muted_text),
        );
    }
    if let Some(frames) = &instance.frame_count {
        state_line = state_line.push(
            text(format!("Frames: {frames}"))
                .size(font::SMALL)
                .style(theme::muted_text),
        );
    }
    if let Some(pipeline) = &instance.pipeline_version {
        state_line = state_line.push(badge(format!("Pipeline {pipeline}"), Tone::Highlight));
    }
    if instance.safe_mode {
        state_line = state_line.push(badge("Safe mode", Tone::Danger));
    }
    details = details.push(state_line);

    if let Some(dialog) = &instance.dialog {
        details = details.push(
            text(format!("Blocking dialog: {dialog}"))
                .size(font::SMALL)
                .style(theme::warning_text),
        );
    }

    if metrics_enabled {
        let metrics_view: Element<'_, UiAction> = match &instance.metrics {
            Some(metrics) => row![
                metric("CPU", metrics.cpu.as_str()),
                metric("Memory", metrics.memory.as_str()),
                metric("Uptime", metrics.uptime.as_str()),
            ]
            .spacing(18)
            .into(),
            None => row![hint(
                "CPU, memory, and uptime are unknown for this process."
            )]
            .into(),
        };
        details = details.push(metrics_view);
    }

    let mut actions = row![space().width(Fill)].spacing(8).align_y(Center);
    if let Some(pid) = &instance.pid {
        actions = actions
            .push(
                icon_only(icons::COPY, ButtonKind::Ghost).on_press(UiAction::CopyText(pid.clone())),
            )
            .push(
                text(format!("pid {pid}"))
                    .size(font::MONO)
                    .font(fonts::mono())
                    .style(theme::faint_text),
            );
    }
    if let Some(port) = &instance.port {
        actions = actions
            .push(
                icon_only(icons::COPY, ButtonKind::Ghost)
                    .on_press(UiAction::CopyText(port.clone())),
            )
            .push(
                text(format!("port {port}"))
                    .size(font::MONO)
                    .font(fonts::mono())
                    .style(theme::faint_text),
            );
    }
    actions = actions.push(
        icon_button(icons::COPY, "Path", ButtonKind::Ghost)
            .on_press(UiAction::CopyText(instance.project_path.clone()))
            .padding([2.0, 4.0]),
    );
    actions = actions.push(
        icon_button(icons::FOLDER_OPEN, "Open", ButtonKind::Secondary)
            .on_press(UiAction::OpenFolder(instance.project_path.clone()))
            .padding(crate::components::COMPACT),
    );

    list_row(
        row![icon_tile(glyph, tone, 40.0), details, actions,]
            .spacing(16)
            .align_y(Center),
    )
}

fn metric<'a>(label: &'a str, value: &'a str) -> Element<'a, UiAction> {
    row![
        text(label).size(font::CAPTION).style(theme::faint_text),
        text(value)
            .size(font::SMALL)
            .font(fonts::mono())
            .style(theme::muted_text),
    ]
    .spacing(6)
    .align_y(Center)
    .into()
}
