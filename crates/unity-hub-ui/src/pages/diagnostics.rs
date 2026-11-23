use iced::widget::{column, container, row};
use iced::{Center, Element, Fill};

use unity_hub_model::{
    CheckStatus, DiagnoseReportVm, DiagnoseTopic, DiagnosticsVm, DoctorCheckVm, DoctorVm,
    EnvironmentVm, LoadState, ProxyVm, Tone, UiAction,
};

use crate::components::{
    badge, button, caption, card_with, detail_column, detail_mono, empty, error_text, hint,
    icon_button, loading, mono, page_header, refresh_button, scroll, ButtonKind, COMPACT,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::text;

pub fn view(vm: &DiagnosticsVm) -> Element<'_, UiAction> {
    let copy = if vm.can_copy {
        icon_button(icons::COPY, "Copy report", ButtonKind::Secondary)
            .on_press(UiAction::CopyDiagnosticsReport)
    } else {
        icon_button(icons::COPY, "Copy report", ButtonKind::Secondary)
    };

    let actions = row![refresh_button(), copy]
        .spacing(8)
        .align_y(Center)
        .into();

    column![
        page_header(
            "Diagnostics",
            "Read-only checks for this machine: CLI, environment, proxy, accelerator, and updates.",
            Some(actions),
        ),
        hint("Nothing on this page installs, updates, or changes the machine."),
        doctor_section(&vm.doctor),
        environment_section(&vm.environment),
        report_section(&vm.proxy_report, DiagnoseTopic::Proxy),
        report_section(&vm.accelerator_report, DiagnoseTopic::Accelerator),
        report_section(&vm.update_report, DiagnoseTopic::Update),
    ]
    .spacing(spacing::SECTION)
    .into()
}

fn doctor_section(state: &LoadState<DoctorVm>) -> Element<'_, UiAction> {
    let summary = state
        .data()
        .map(check_summary)
        .unwrap_or_else(|| hint("Not run"));

    let mut header = row![summary].spacing(8).align_y(Center);
    match state {
        LoadState::Idle => {
            header = header
                .push(button("Run doctor", ButtonKind::Secondary).on_press(UiAction::LoadDoctor));
        }
        LoadState::Ready(_) => {
            header = header.push(
                button("Refresh doctor", ButtonKind::Secondary).on_press(UiAction::LoadDoctor),
            );
        }
        _ => {}
    }

    let body = if matches!(state, LoadState::Idle) {
        empty(
            "CLI checks are not run",
            "`unity doctor` probes the credential store and can trigger a polkit password prompt. Run it only when you need the full report.",
        )
    } else {
        section_body(
            state,
            "Reading `unity doctor`…",
            "CLI checks are not run",
            "`unity doctor` probes the credential store and can trigger a polkit password prompt. Run it only when you need the full report.",
            doctor_body,
        )
    };

    card_with("CLI checks", header.into(), body)
}

fn environment_section(state: &LoadState<EnvironmentVm>) -> Element<'_, UiAction> {
    card_with(
        "Environment",
        badge("unity env", Tone::Neutral),
        section_body(
            state,
            "Reading `unity env`…",
            "Environment is not loaded",
            "Open this page to read the CLI's resolved paths and proxy.",
            environment_body,
        ),
    )
}

fn report_section(
    state: &LoadState<DiagnoseReportVm>,
    topic: DiagnoseTopic,
) -> Element<'_, UiAction> {
    let copy = icon_button(icons::COPY, "Copy", ButtonKind::Ghost).padding(COMPACT);
    let copy = if state.data().is_some() {
        copy.on_press(UiAction::CopyDiagnoseReport(topic))
    } else {
        copy
    };

    card_with(
        report_title(topic),
        row![badge(topic.label(), Tone::Neutral), copy]
            .spacing(8)
            .align_y(Center)
            .into(),
        section_body(
            state,
            report_loading_label(topic),
            report_empty_title(topic),
            report_empty_description(topic),
            report_body,
        ),
    )
}

/// One section's state, never the page's: a failed report leaves the doctor
/// checks and the environment paths exactly as they were.
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

fn check_summary(doctor: &DoctorVm) -> Element<'static, UiAction> {
    let count = |status: CheckStatus| {
        doctor
            .checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    };

    let mut summary = row![].spacing(6).align_y(Center);
    for (status, label) in [
        (CheckStatus::Pass, "pass"),
        (CheckStatus::Warn, "warn"),
        (CheckStatus::Fail, "fail"),
        (CheckStatus::Info, "info"),
        (CheckStatus::Unknown, "unknown"),
    ] {
        let count = count(status);
        if count > 0 {
            let (tone, _, _) = status_visual(status);
            summary = summary.push(badge(format!("{count} {label}"), tone));
        }
    }
    if doctor.checks.is_empty() {
        summary = summary.push(hint("No checks reported"));
    }
    summary.into()
}

fn doctor_body(doctor: &DoctorVm) -> Element<'_, UiAction> {
    let details = column![
        row![
            detail_column("Platform", doctor.platform.as_str()),
            detail_column("Architecture", doctor.arch.as_str()),
        ]
        .spacing(16),
        row![
            detail_column("Runtime", doctor.runtime_version.as_str()),
            detail_column("Node", doctor.node_version.as_str()),
        ]
        .spacing(16),
        row![
            detail_column("CLI version", doctor.cli_version.as_str()),
            detail_column("Install path", doctor.install_path.as_str()),
        ]
        .spacing(16),
        detail_mono("Log directory", doctor.log_directory.as_str()),
    ]
    .spacing(12);

    let mut body = column![details].spacing(spacing::SECTION);

    if doctor.auth.is_some() || doctor.license.is_some() {
        let mut account = row![].spacing(16);
        if let Some(auth) = &doctor.auth {
            account = account.push(detail_column("Auth", auth.as_str()));
        }
        if let Some(license) = &doctor.license {
            account = account.push(detail_column("License", license.as_str()));
        }
        body = body.push(
            container(account)
                .padding([12.0, 14.0])
                .width(Fill)
                .style(theme::inset),
        );
    }

    if doctor.checks.is_empty() {
        body = body.push(hint("The CLI reported no environment checks."));
    } else {
        let rows = doctor
            .checks
            .iter()
            .map(check_row)
            .collect::<Vec<Element<'_, UiAction>>>();
        body = body.push(crate::components::divided(rows));
    }

    body = body.push(
        column![
            caption("Raw report"),
            container(
                scroll(
                    text(doctor.raw.as_str())
                        .size(font::MONO)
                        .font(fonts::mono())
                        .line_height(1.55)
                        .style(theme::muted_text),
                )
                .height(220),
            )
            .padding([12.0, 14.0])
            .width(Fill)
            .style(theme::inset),
        ]
        .spacing(8),
    );

    body.into()
}

fn check_row(check: &DoctorCheckVm) -> Element<'_, UiAction> {
    let (tone, glyph, label) = status_visual(check.status);

    let mut detail = column![row![
        text(check.label.as_str())
            .size(font::BODY)
            .font(fonts::medium())
            .width(Fill),
        badge(label, tone),
    ]
    .spacing(10)
    .align_y(Center),]
    .spacing(4);

    if let Some(text) = &check.detail {
        detail = detail.push(mono(text.as_str()));
    }

    row![
        container(icon(glyph, font::ICON_SMALL).style(theme::tone_text(tone))).padding([2.0, 0.0]),
        detail.width(Fill),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Start)
    .into()
}

fn status_visual(status: CheckStatus) -> (Tone, char, &'static str) {
    match status {
        CheckStatus::Pass => (Tone::Success, icons::CIRCLE_CHECK, "pass"),
        CheckStatus::Warn => (Tone::Warning, icons::TRIANGLE_ALERT, "warn"),
        CheckStatus::Fail => (Tone::Danger, icons::CIRCLE_X, "fail"),
        CheckStatus::Info => (Tone::Info, icons::INFO, "info"),
        CheckStatus::Unknown => (Tone::Neutral, icons::CIRCLE_DOT, "unknown"),
    }
}

fn environment_body(environment: &EnvironmentVm) -> Element<'_, UiAction> {
    let paths = column![
        row![
            detail_mono("User data", environment.user_data_path.as_str()),
            detail_mono("Editor installs", environment.editor_install_path.as_str()),
        ]
        .spacing(16),
        row![
            detail_mono("Download cache", environment.download_cache_path.as_str()),
            detail_mono("Config", environment.config_path.as_str()),
        ]
        .spacing(16),
        row![
            detail_column("CLI version", environment.cli_version.as_str()),
            detail_column("Hub version", environment.hub_version.as_str()),
        ]
        .spacing(16),
    ]
    .spacing(12);

    let proxy = match &environment.proxy {
        Some(proxy) => proxy_body(proxy),
        None => hint("The CLI reported no proxy section."),
    };

    column![
        paths,
        container(proxy)
            .padding([12.0, 14.0])
            .width(Fill)
            .style(theme::inset),
    ]
    .spacing(spacing::SMALL + 2.0)
    .into()
}

fn proxy_body(proxy: &ProxyVm) -> Element<'_, UiAction> {
    let no_proxy = if proxy.no_proxy.is_empty() {
        "none".to_string()
    } else {
        proxy.no_proxy.join(", ")
    };

    column![
        caption("Resolved proxy"),
        row![
            detail_column("URL", proxy.url.as_str()),
            detail_column("Source", proxy.source.as_str()),
            detail_column("Auth source", proxy.auth_source.as_str()),
        ]
        .spacing(16),
        detail_mono("No proxy", no_proxy),
    ]
    .spacing(12)
    .into()
}

fn report_body(report: &DiagnoseReportVm) -> Element<'_, UiAction> {
    if report.text.trim().is_empty() {
        return hint("The CLI returned an empty report.");
    }

    container(
        scroll(
            text(report.text.as_str())
                .size(font::MONO)
                .font(fonts::mono())
                .line_height(1.55)
                .style(theme::muted_text),
        )
        .height(220),
    )
    .padding([12.0, 14.0])
    .width(Fill)
    .style(theme::inset)
    .into()
}

fn report_title(topic: DiagnoseTopic) -> &'static str {
    match topic {
        DiagnoseTopic::Proxy => "Proxy report",
        DiagnoseTopic::Accelerator => "Accelerator report",
        DiagnoseTopic::Update => "Update report",
    }
}

fn report_loading_label(topic: DiagnoseTopic) -> &'static str {
    match topic {
        DiagnoseTopic::Proxy => "Reading `unity diagnose proxy`…",
        DiagnoseTopic::Accelerator => "Reading `unity diagnose accelerator`…",
        DiagnoseTopic::Update => "Reading `unity diagnose update`…",
    }
}

fn report_empty_title(topic: DiagnoseTopic) -> &'static str {
    match topic {
        DiagnoseTopic::Proxy => "Proxy report is not loaded",
        DiagnoseTopic::Accelerator => "Accelerator report is not loaded",
        DiagnoseTopic::Update => "Update report is not loaded",
    }
}

fn report_empty_description(topic: DiagnoseTopic) -> &'static str {
    match topic {
        DiagnoseTopic::Proxy => "Open this page to read the redacted proxy diagnostic.",
        DiagnoseTopic::Accelerator => "Open this page to check the resolved Accelerator endpoint.",
        DiagnoseTopic::Update => "Open this page to see why this install is or is not updating.",
    }
}
