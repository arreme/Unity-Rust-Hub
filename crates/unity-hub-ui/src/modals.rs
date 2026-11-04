use iced::widget::{column, container, pick_list, row, space};
use iced::{Center, Element, Fill};

use unity_hub_model::{
    EditorModulesVm, InstallCliVm, ModalVm, ModuleOptionVm, Tone, UiAction, UpgradeProjectVm,
};

use crate::components::{
    button, caption, checkbox, divided, divider, error_text, field_label, hint, icon_button,
    loading, mono, notice, scroll, ButtonKind, PICK_LIST_PADDING,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::font;
use crate::ui::{self, text};

const DIALOG_WIDTH: f32 = 560.0;

pub fn view(modal: &ModalVm) -> Element<'_, UiAction> {
    let card = match modal {
        ModalVm::InstallCli(vm) => install_cli_modal(vm),
        ModalVm::EditorModules(vm) => editor_modules_modal(vm),
        ModalVm::UpgradeProject(vm) => upgrade_modal(vm),
    };

    container(iced::widget::center(card).padding(24))
        .width(Fill)
        .height(Fill)
        .style(theme::modal_screen)
        .into()
}

/// Shared dialog frame: title and subtitle with a round close button, the
/// body, and the actions right-aligned under a hairline.
fn dialog<'a>(
    title: impl Into<String>,
    subtitle: impl Into<String>,
    body: Element<'a, UiAction>,
    actions: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    let header = row![
        column![
            text(title.into())
                .size(font::MODAL_TITLE)
                .font(fonts::SEMIBOLD),
            text(subtitle.into())
                .size(font::BODY_LARGE)
                .style(theme::muted_text),
        ]
        .spacing(4)
        .width(Fill),
        iced::widget::button(
            container(icon(icons::X, font::ICON_SMALL))
                .width(20)
                .height(20)
                .center(20)
        )
        .on_press(UiAction::CloseModal)
        .padding(8)
        .style(theme::button_round),
    ]
    .spacing(16)
    .align_y(iced::Alignment::Start);

    container(
        column![
            header,
            body,
            divider(),
            row![space().width(Fill), actions].align_y(Center),
        ]
        .spacing(22),
    )
    .width(DIALOG_WIDTH)
    .padding(28)
    .style(theme::modal)
    .into()
}

fn labelled<'a>(label: &'a str, control: Element<'a, UiAction>) -> Element<'a, UiAction> {
    column![field_label(label), control].spacing(8).into()
}

fn styled_pick_list<'a>(
    options: Vec<String>,
    selected: Option<String>,
    on_select: impl Fn(String) -> UiAction + 'a,
    placeholder: &str,
) -> Element<'a, UiAction> {
    pick_list(options, selected, on_select)
        .text_size(ui::font_size(font::BODY))
        .padding(PICK_LIST_PADDING)
        .handle(icons::pick_list_handle(font::ICON_SMALL))
        .style(theme::pick_list)
        .menu_style(theme::menu)
        .width(Fill)
        .placeholder(placeholder.to_string())
        .into()
}

fn install_cli_modal(vm: &InstallCliVm) -> Element<'_, UiAction> {
    let body = column![
        labelled(
            "Release channel",
            styled_pick_list(
                vm.channels.clone(),
                Some(vm.selected_channel.clone()),
                UiAction::CliChannelChanged,
                "Select a channel",
            ),
        ),
        container(
            column![
                caption("Official installer command"),
                text(vm.command_preview.as_str())
                    .size(font::MONO)
                    .font(fonts::MONO)
                    .line_height(1.55),
                hint("UNITY_CLI_SKIP_SETUP=1 is set so the installer does not block on its interactive setup."),
            ]
            .spacing(8),
        )
        .padding([14.0, 16.0])
        .width(Fill)
        .style(theme::raised),
        checkbox(
            "I understand this downloads and executes Unity's installer script from unity.com",
            vm.consent,
            UiAction::CliConsentToggled,
        ),
    ]
    .spacing(20);

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        icon_button(icons::DOWNLOAD, "Run installer", ButtonKind::Primary)
            .on_press_maybe(vm.can_install.then_some(UiAction::InstallCli)),
    ]
    .spacing(8);

    dialog(
        "Install the Unity CLI",
        "Unity's official installer runs locally. This app does not bundle the CLI.",
        body.into(),
        actions.into(),
    )
}

fn editor_modules_modal(vm: &EditorModulesVm) -> Element<'_, UiAction> {
    let installed: Element<'_, UiAction> = if vm.installed_modules.is_empty() {
        hint("No modules reported.")
    } else {
        text(vm.installed_modules.join(" · "))
            .size(font::BODY)
            .style(theme::muted_text)
            .into()
    };

    let module_list: Element<'_, UiAction> = if let Some(error) = vm.modules.error() {
        error_text(error)
    } else if let Some(modules) = vm.modules.data() {
        let list = divided(
            modules
                .iter()
                .map(|module| {
                    container(module_checkbox(module))
                        .padding([13.0, 16.0])
                        .width(Fill)
                        .into()
                })
                .collect(),
        );
        container(container(scroll(list)).max_height(280))
            .width(Fill)
            .style(theme::outline)
            .into()
    } else if vm.modules.is_loading() {
        loading("Loading modules…")
    } else {
        hint("Load modules for this Editor to add them.")
    };

    let body = column![
        labelled("Installed", installed),
        labelled("Available", module_list),
        checkbox(
            "I accept the module license agreements (--accept-eula)",
            vm.accept_eula,
            UiAction::DialogEulaToggled,
        ),
    ]
    .spacing(20);

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        icon_button(
            icons::DOWNLOAD,
            format!("Install {} module(s)", vm.selected_count),
            ButtonKind::Primary,
        )
        .on_press_maybe(vm.can_install.then_some(UiAction::StartInstallModules)),
    ]
    .spacing(8);

    dialog(
        format!("Modules for Unity {}", vm.editor_version),
        "Only modules that are not already installed are listed.",
        body.into(),
        actions.into(),
    )
}

fn module_checkbox(module: &ModuleOptionVm) -> Element<'static, UiAction> {
    let id = module.id.clone();
    checkbox(module.label.clone(), module.selected, move |value| {
        UiAction::ToggleDialogModule(id.clone(), value)
    })
    .width(Fill)
    .into()
}

fn version_cell<'a>(label: &'a str, version: String, end: bool) -> Element<'a, UiAction> {
    let content = column![
        caption(label),
        text(version).size(font::BODY_LARGE + 1.0).font(fonts::MONO),
    ]
    .spacing(4);

    if end {
        container(content.align_x(iced::Alignment::End))
            .width(Fill)
            .align_right(Fill)
            .into()
    } else {
        container(content).width(Fill).into()
    }
}

fn upgrade_modal(vm: &UpgradeProjectVm) -> Element<'_, UiAction> {
    let picker: Element<'_, UiAction> = if vm.target_options.is_empty() {
        notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            text("No other Editor version is installed. Install one on the Editors page first.")
                .size(font::BODY)
                .style(theme::muted_text),
        )
    } else {
        styled_pick_list(
            vm.target_options.clone(),
            vm.selected_target.clone(),
            UiAction::UpgradeTargetChanged,
            "Select an installed Editor",
        )
    };

    let strip = container(
        row![
            version_cell("Current", vm.current_version.clone(), false),
            icon(icons::ARROW_RIGHT, font::ICON).style(theme::faint_text),
            version_cell(
                "Target",
                vm.selected_target
                    .clone()
                    .unwrap_or_else(|| "—".to_string()),
                true,
            ),
        ]
        .spacing(16)
        .align_y(Center),
    )
    .padding([18.0, 20.0])
    .width(Fill)
    .style(theme::raised);

    let body = column![
        strip,
        column![
            labelled("Target Editor version", picker),
            mono(vm.project_path_short.as_str()),
        ]
        .spacing(8),
        notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            text("Runs `unity projects upgrade <path> --to <version> --yes`. It can update ProjectVersion.txt, open the Editor, and run API upgrades. Close the project in any running Editor and commit or back up first.")
                .size(font::BODY)
                .line_height(1.55)
                .style(theme::muted_text),
        ),
        checkbox(
            "I understand and want to upgrade this project now",
            vm.confirmed,
            UiAction::UpgradeConfirmed,
        ),
    ]
    .spacing(20);

    let actions = row![
        button("Cancel", ButtonKind::Ghost).on_press(UiAction::CloseModal),
        button("Upgrade project", ButtonKind::Primary)
            .on_press_maybe(vm.can_upgrade.then_some(UiAction::StartUpgrade)),
    ]
    .spacing(8);

    dialog(
        "Change Editor version",
        vm.project_title.as_str(),
        body.into(),
        actions.into(),
    )
}
