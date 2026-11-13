use iced::widget::{column, container, pick_list, row};
use iced::{Center, Element, Fill, Length};

use unity_hub_model::{SettingsVm, Tone, UiAction};

use crate::components::{
    badge, button, group, hint, list_row, mono, page_header, segmented, setting_row, text_input,
    ButtonKind, PICK_LIST_PADDING,
};
use crate::fonts;
use crate::icons;
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::{self, text};

const MAX_WIDTH: f32 = 860.0;

pub fn view(vm: &SettingsVm) -> Element<'_, UiAction> {
    container(
        column![
            page_header(
                "Settings",
                "Point the app at a specific CLI binary and adjust readability.",
                None,
            ),
            cli_group(vm),
            appearance_group(vm),
            app_data_group(vm),
            about_group(vm),
        ]
        .spacing(28),
    )
    .max_width(MAX_WIDTH)
    .into()
}

fn value<'a>(label: &'a str) -> Element<'a, UiAction> {
    text(label)
        .size(font::BODY_LARGE)
        .style(theme::muted_text)
        .into()
}

fn cli_group(vm: &SettingsVm) -> Element<'_, UiAction> {
    let override_row = list_row(
        column![
            column![
                text("Manual path override").size(font::BODY_LARGE),
                hint("Leave empty to use the install receipt, UNITY_CLI_HOME, platform defaults, and PATH."),
            ]
            .spacing(2),
            row![
                text_input("/home/you/.local/bin/unity", &vm.cli_path_input)
                    .on_input(UiAction::CliPathChanged)
                    .icon(icons::input_icon(icons::TERMINAL, font::ICON_SMALL))
                    .width(Fill),
                button("Save path", ButtonKind::Primary).on_press(UiAction::SaveCliPath),
                button("Clear", ButtonKind::Ghost).on_press(UiAction::ClearCliPath),
            ]
            .spacing(8)
            .align_y(Center),
        ]
        .spacing(spacing::SMALL + 2.0),
    );

    group(
        "Unity CLI",
        vec![
            setting_row(
                "Resolved executable",
                None,
                row![
                    mono(vm.resolved_executable.as_str()),
                    badge(vm.resolved_via.clone(), Tone::Neutral),
                ]
                .spacing(10)
                .align_y(Center)
                .into(),
            ),
            override_row,
            setting_row(
                "Check the CLI again",
                None,
                button("Re-check", ButtonKind::Secondary)
                    .on_press(UiAction::Refresh)
                    .padding(crate::components::COMPACT)
                    .into(),
            ),
        ],
    )
}

fn appearance_group(vm: &SettingsVm) -> Element<'_, UiAction> {
    let sizes = vm
        .text_scale_options
        .iter()
        .map(|option| {
            (
                option.label.clone(),
                option.percent == vm.text_scale_percent,
                UiAction::TextScaleChanged(option.percent),
            )
        })
        .collect();

    group(
        "Appearance",
        vec![
            setting_row(
                "Font",
                Some("Uses the font families installed on this PC."),
                font_picker(vm),
            ),
            setting_row(
                "Text size",
                Some("Scales text only; controls keep their size."),
                segmented(sizes),
            ),
            list_row(
                text("The quick brown fox jumps over the lazy dog.")
                    .size(font::BODY_LARGE)
                    .style(theme::muted_text),
            ),
        ],
    )
}

fn font_picker(vm: &SettingsVm) -> Element<'_, UiAction> {
    pick_list(
        vm.font_options.clone(),
        Some(vm.font_family.clone()),
        UiAction::FontFamilyChanged,
    )
    .text_size(ui::font_size(font::BODY))
    .font(fonts::regular())
    .padding(PICK_LIST_PADDING)
    .handle(icons::pick_list_handle(font::ICON_SMALL))
    .style(theme::pick_list)
    .menu_style(theme::menu)
    .width(Length::Fixed(ui::scaled_pixels(240.0)))
    .into()
}

fn app_data_group(vm: &SettingsVm) -> Element<'_, UiAction> {
    group(
        "App data",
        vec![
            setting_row("Config directory", None, mono(vm.config_dir.as_str())),
            setting_row("Settings file", None, mono(vm.settings_file.as_str())),
            list_row(hint(
                "Only the optional CLI path, the text size, and the font are stored. Credentials and account state stay in the CLI and the OS keyring.",
            )),
        ],
    )
}

fn about_group(vm: &SettingsVm) -> Element<'_, UiAction> {
    group(
        "About",
        vec![
            setting_row("App version", None, value(env!("CARGO_PKG_VERSION"))),
            setting_row("UI", None, value("iced 0.14 · native Rust")),
            setting_row("CLI adapter", None, value("unity-core")),
            setting_row("Engine", None, value(vm.engine_info.as_str())),
        ],
    )
}
