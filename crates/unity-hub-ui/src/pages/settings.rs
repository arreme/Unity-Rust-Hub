use iced::widget::{button, column, container, pick_list, row, space, text_input};
use iced::{Element, Fill};

use unity_hub_model::{SettingsVm, TextScaleOptionVm, Tone, UiAction};

use crate::components::{badge, card, detail_column, page_header};
use crate::theme;
use crate::ui::{self, text};

pub fn view(vm: &SettingsVm) -> Element<'_, UiAction> {
    let mut page = column![page_header(
        "CONFIGURATION",
        "Settings",
        "Point the app at a specific CLI binary and adjust text readability.",
    )]
    .spacing(16);

    page = page.push(
        row![cli_location_card(vm), app_data_card(vm)]
            .spacing(16)
            .align_y(iced::Alignment::Start),
    );
    page = page.push(appearance_card(vm));
    page = page.push(about_card(vm));

    page.into()
}

fn cli_location_card(vm: &SettingsVm) -> Element<'_, UiAction> {
    let body =
        column![
        row![
            detail_column("Resolved executable", vm.resolved_executable.as_str()),
            detail_column("Resolved via", vm.resolved_via.as_str()),
        ]
        .spacing(16),
        text("Manual CLI path override")
            .size(9)
            .style(theme::faint_text),
        text_input("/home/you/.local/bin/unity", &vm.cli_path_input)
            .on_input(UiAction::CliPathChanged)
            .padding(8)
            .size(ui::font_size(11.0))
            .width(Fill),
        text("Leave empty to use the install receipt, UNITY_CLI_HOME, platform defaults, and PATH.")
            .size(9)
            .style(theme::faint_text),
        row![
            button(text("Save path").size(10))
                .on_press(UiAction::SaveCliPath)
                .padding([6.0, 10.0])
                .style(button::primary),
            button(text("Clear override").size(10))
                .on_press(UiAction::ClearCliPath)
                .padding([6.0, 10.0])
                .style(button::text),
            space().width(Fill),
            button(text("Re-check").size(10))
                .on_press(UiAction::Refresh)
                .padding([6.0, 10.0])
                .style(button::secondary),
        ]
        .spacing(7)
        .align_y(iced::Alignment::Center),
    ]
        .spacing(10);

    card("Unity CLI location", body.into())
}

fn app_data_card(vm: &SettingsVm) -> Element<'_, UiAction> {
    let body = column![
        detail_column("Config directory", vm.config_dir.as_str()),
        detail_column("Settings file", vm.settings_file.as_str()),
        text("Nothing is stored except the optional CLI path and the text-size preference. Credentials and Unity account state stay in the CLI and the OS keyring.")
            .size(10)
            .style(theme::muted_text),
    ]
    .spacing(10);

    card("App data", body.into())
}

fn appearance_card(vm: &SettingsVm) -> Element<'_, UiAction> {
    let selected = vm
        .text_scale_options
        .iter()
        .find(|option| option.percent == vm.text_scale_percent)
        .cloned();

    let body = column![
        text("Text size").size(9).style(theme::faint_text),
        pick_list(
            vm.text_scale_options.clone(),
            selected,
            |option: TextScaleOptionVm| UiAction::TextScaleChanged(option.percent),
        )
        .text_size(ui::font_size(12.0))
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

fn about_card(vm: &SettingsVm) -> Element<'_, UiAction> {
    let body = column![
        row![
            detail_column("App version", env!("CARGO_PKG_VERSION")),
            detail_column("UI", "iced 0.14 · native Rust"),
            detail_column("CLI adapter", "unity-core"),
        ]
        .spacing(16),
        text("Engine").size(9).style(theme::faint_text),
        text(vm.engine_info.as_str())
            .size(11)
            .style(theme::muted_text),
        text("Linux builds use iced's software renderer (tiny-skia), so there is no webview or GPU API requirement.")
            .size(10)
            .style(theme::faint_text),
        text("CLI calls opt out of telemetry (DO_NOT_TRACK=1) and run one at a time to keep memory low.")
            .size(10)
            .style(theme::faint_text),
        row![
            badge("Unity CLI docs", Tone::Accent),
            badge("Unity CLI reference", Tone::Info),
            badge("Release notes", Tone::Highlight),
        ]
        .spacing(5),
    ]
    .spacing(9);

    card("About", body.into())
}
