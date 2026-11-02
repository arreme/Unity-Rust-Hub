use iced::widget::{button, column, row, space, text, text_input};
use iced::{Element, Fill};

use crate::app::{badge, detail_column, App, Message};
use crate::theme;
use crate::views::{card, page_header};

pub fn view(app: &App) -> Element<'_, Message> {
    let mut page = column![page_header(
        "CONFIGURATION",
        "Settings",
        "Point the app at a specific CLI binary when auto-detection is not enough.",
    )]
    .spacing(16);

    page = page.push(
        row![cli_location_card(app), app_data_card(app)]
            .spacing(16)
            .align_y(iced::Alignment::Start),
    );
    page = page.push(about_card(app));

    page.into()
}

fn cli_location_card(app: &App) -> Element<'_, Message> {
    let resolved = app
        .cli_status
        .data
        .as_ref()
        .and_then(|status| status.path.clone())
        .unwrap_or_else(|| "Not found".to_string());
    let source = app
        .cli_status
        .data
        .as_ref()
        .and_then(|status| status.path_source.clone())
        .unwrap_or_else(|| "—".to_string());

    let body =
        column![
        row![
            detail_column("Resolved executable", resolved),
            detail_column("Resolved via", source),
        ]
        .spacing(16),
        text("Manual CLI path override").size(9).style(theme::faint_text),
        text_input("/home/you/.local/bin/unity", &app.cli_path_input)
            .on_input(Message::CliPathChanged)
            .padding(8)
            .size(11)
            .width(Fill),
        text("Leave empty to use the install receipt, UNITY_CLI_HOME, platform defaults, and PATH.")
            .size(9)
            .style(theme::faint_text),
        row![
            button(text("Save path").size(10))
                .on_press(Message::SaveCliPath)
                .padding([6.0, 10.0])
                .style(button::primary),
            button(text("Clear override").size(10))
                .on_press(Message::ClearCliPath)
                .padding([6.0, 10.0])
                .style(button::text),
            space().width(Fill),
            button(text("Re-check").size(10))
                .on_press(Message::Refresh)
                .padding([6.0, 10.0])
                .style(button::secondary),
        ]
        .spacing(7)
        .align_y(iced::Alignment::Center),
    ]
        .spacing(10);

    card("Unity CLI location", body.into())
}

fn app_data_card(app: &App) -> Element<'_, Message> {
    let config = app.config_dir.clone().unwrap_or_else(|| "—".to_string());
    let settings_file = if config == "—" {
        "—".to_string()
    } else {
        format!("{config}/settings.json")
    };

    let body = column![
        detail_column("Config directory", config),
        detail_column("Settings file", settings_file),
        text("Nothing is stored except the optional CLI path. Credentials and Unity account state stay in the CLI and the OS keyring.")
            .size(10)
            .style(theme::muted_text),
    ]
    .spacing(10);

    card("App data", body.into())
}

fn about_card(app: &App) -> Element<'_, Message> {
    let body = column![
        row![
            detail_column("App version", env!("CARGO_PKG_VERSION").to_string()),
            detail_column("UI", "iced 0.14 · native Rust".to_string()),
            detail_column("CLI adapter", "unity-core".to_string()),
        ]
        .spacing(16),
        text("Engine").size(9).style(theme::faint_text),
        text(app.engine_info()).size(11).style(theme::muted_text),
        text("Linux builds use iced's software renderer (tiny-skia), so there is no webview or GPU API requirement.")
            .size(10)
            .style(theme::faint_text),
        row![
            badge("Unity CLI docs", theme::Tone::Accent),
            badge("Unity CLI reference", theme::Tone::Blue),
            badge("Release notes", theme::Tone::Violet),
        ]
        .spacing(5),
    ]
    .spacing(9);

    card("About", body.into())
}
