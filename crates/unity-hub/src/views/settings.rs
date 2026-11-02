use iced::widget::{button, column, container, pick_list, row, space, text_input};
use iced::{Element, Fill};

use crate::app::{badge, detail_column, App, Message};
use crate::settings::TextSize;
use crate::theme;
use crate::ui::text;
use crate::views::{card, page_header};

pub fn view(app: &App) -> Element<'_, Message> {
    let mut page = column![page_header(
        "CONFIGURATION",
        "Settings",
        "Point the app at a specific CLI binary and adjust text readability.",
    )]
    .spacing(16);

    page = page.push(
        row![cli_location_card(app), app_data_card(app)]
            .spacing(16)
            .align_y(iced::Alignment::Start),
    );
    page = page.push(appearance_card(app));
    page = page.push(about_card(app));

    page.into()
}

fn cli_location_card(app: &App) -> Element<'_, Message> {
    let resolved = app
        .cli_status
        .data
        .as_ref()
        .and_then(|status| status.path.as_deref())
        .unwrap_or("Not found");
    let source = app
        .cli_status
        .data
        .as_ref()
        .and_then(|status| status.path_source.as_deref())
        .unwrap_or("—");

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
            .size(app.font_size(11.0))
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
    let config = app.config_dir.as_deref().unwrap_or("—");
    let settings_file = if config == "—" {
        "—".to_string()
    } else {
        format!("{config}/settings.json")
    };

    let body = column![
        detail_column("Config directory", config),
        detail_column("Settings file", settings_file),
        text("Nothing is stored except the optional CLI path and the text-size preference. Credentials and Unity account state stay in the CLI and the OS keyring.")
            .size(10)
            .style(theme::muted_text),
    ]
    .spacing(10);

    card("App data", body.into())
}

fn appearance_card(app: &App) -> Element<'_, Message> {
    let body = column![
        text("Text size").size(9).style(theme::faint_text),
        pick_list(
            TextSize::ALL.to_vec(),
            Some(app.app_settings.text_size()),
            Message::TextSizeChanged,
        )
        .text_size(app.font_size(12.0))
        .width(220),
        text("Scales text throughout the app. Controls and layout keep their current size.")
            .size(9)
            .style(theme::faint_text),
        container(
            text("Preview: The quick brown fox jumps over the lazy dog.")
                .size(12)
                .style(theme::muted_text),
        )
        .padding(10)
        .width(Fill)
        .style(theme::card),
    ]
    .spacing(9);

    card("Appearance", body.into())
}

fn about_card(app: &App) -> Element<'_, Message> {
    let body = column![
        row![
            detail_column("App version", env!("CARGO_PKG_VERSION")),
            detail_column("UI", "iced 0.14 · native Rust"),
            detail_column("CLI adapter", "unity-core"),
        ]
        .spacing(16),
        text("Engine").size(9).style(theme::faint_text),
        text(app.engine_info()).size(11).style(theme::muted_text),
        text("Linux builds use iced's software renderer (tiny-skia), so there is no webview or GPU API requirement.")
            .size(10)
            .style(theme::faint_text),
        text("CLI calls opt out of telemetry (DO_NOT_TRACK=1) and run one at a time to keep memory low.")
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
