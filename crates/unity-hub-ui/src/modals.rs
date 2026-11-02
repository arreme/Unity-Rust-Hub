use iced::widget::{button, checkbox, column, container, pick_list, row, scrollable, space};
use iced::{Element, Fill};

use unity_hub_model::{
    EditorModulesVm, InstallCliVm, ModalVm, ModuleOptionVm, Tone, UiAction, UpgradeProjectVm,
};

use crate::theme;
use crate::ui::{self, text};

pub fn view(modal: &ModalVm) -> Element<'_, UiAction> {
    let card = match modal {
        ModalVm::InstallCli(vm) => install_cli_modal(vm),
        ModalVm::EditorModules(vm) => editor_modules_modal(vm),
        ModalVm::UpgradeProject(vm) => upgrade_modal(vm),
    };

    container(iced::widget::center(card))
        .width(Fill)
        .height(Fill)
        .style(theme::modal_screen)
        .into()
}

fn install_cli_modal(vm: &InstallCliVm) -> Element<'_, UiAction> {
    let content = column![
        text("Install the Unity CLI").size(20),
        text("Unity's official installer runs locally. This app does not bundle the CLI.")
            .size(11)
            .style(theme::muted_text),
        text("Release channel").size(10).style(theme::faint_text),
        pick_list(
            vm.channels.clone(),
            Some(vm.selected_channel.clone()),
            UiAction::CliChannelChanged,
        )
        .text_size(ui::font_size(12.0))
        .width(Fill),
        container(
            column![
                text("Official installer command")
                    .size(10)
                    .style(theme::faint_text),
                text(vm.command_preview.as_str())
                    .size(11)
                    .font(iced::Font::MONOSPACE)
                    .style(theme::accent_text),
                text("UNITY_CLI_SKIP_SETUP=1 is set so the installer does not block on its interactive setup.")
                    .size(9)
                    .style(theme::faint_text),
            ]
            .spacing(5),
        )
        .padding(10)
        .width(Fill)
        .style(theme::card),
        checkbox(vm.consent)
            .label("I understand this downloads and executes Unity's installer script from unity.com.")
            .on_toggle(UiAction::CliConsentToggled)
            .size(14)
            .text_size(ui::font_size(11.0)),
        row![
            space().width(Fill),
            button(text("Cancel").size(11))
                .on_press(UiAction::CloseModal)
                .padding([7.0, 12.0])
                .style(button::text),
            button(text("Run installer").size(11))
                .on_press_maybe(vm.can_install.then_some(UiAction::InstallCli))
                .padding([7.0, 12.0])
                .style(button::primary),
        ]
        .spacing(8),
    ]
    .spacing(12);

    container(content)
        .width(560)
        .padding(22)
        .style(theme::modal)
        .into()
}

fn editor_modules_modal(vm: &EditorModulesVm) -> Element<'_, UiAction> {
    let installed: Element<'_, UiAction> = if vm.installed_modules.is_empty() {
        text("No modules reported.")
            .size(10)
            .style(theme::faint_text)
            .into()
    } else {
        text(vm.installed_modules.join(" · "))
            .size(10)
            .style(theme::success_text)
            .into()
    };

    let module_list: Element<'_, UiAction> = if let Some(error) = vm.modules.error() {
        text(error.text()).size(10).style(theme::danger_text).into()
    } else if let Some(modules) = vm.modules.data() {
        scrollable(column(modules.iter().map(module_checkbox).collect::<Vec<_>>()).spacing(6))
            .height(280)
            .into()
    } else if vm.modules.is_loading() {
        text("Loading modules…")
            .size(11)
            .style(theme::muted_text)
            .into()
    } else {
        text("Load modules for this Editor to add them.")
            .size(10)
            .style(theme::faint_text)
            .into()
    };

    let content = column![
        text(format!("Modules for Unity {}", vm.editor_version)).size(20),
        text("Only modules that are not already installed are listed.")
            .size(11)
            .style(theme::muted_text),
        text("Installed").size(10).style(theme::faint_text),
        installed,
        module_list,
        checkbox(vm.accept_eula)
            .label("I accept the module license agreements (--accept-eula).")
            .on_toggle(UiAction::DialogEulaToggled)
            .size(14)
            .text_size(ui::font_size(11.0)),
        row![
            space().width(Fill),
            button(text("Cancel").size(11))
                .on_press(UiAction::CloseModal)
                .padding([7.0, 12.0])
                .style(button::text),
            button(text(format!("Install {} module(s)", vm.selected_count)).size(11))
                .on_press_maybe(vm.can_install.then_some(UiAction::StartInstallModules))
                .padding([7.0, 12.0])
                .style(button::primary),
        ]
        .spacing(8),
    ]
    .spacing(12);

    container(content)
        .width(620)
        .padding(22)
        .style(theme::modal)
        .into()
}

fn module_checkbox(module: &ModuleOptionVm) -> Element<'static, UiAction> {
    let id = module.id.clone();
    checkbox(module.selected)
        .label(module.label.clone())
        .on_toggle(move |value| UiAction::ToggleDialogModule(id.clone(), value))
        .size(14)
        .text_size(ui::font_size(10.0))
        .into()
}

fn upgrade_modal(vm: &UpgradeProjectVm) -> Element<'_, UiAction> {
    let picker: Element<'_, UiAction> = if vm.target_options.is_empty() {
        text("No other Editor version is installed. Install one on the Editors page first.")
            .size(11)
            .style(theme::warning_text)
            .into()
    } else {
        pick_list(
            vm.target_options.clone(),
            vm.selected_target.clone(),
            UiAction::UpgradeTargetChanged,
        )
        .text_size(ui::font_size(12.0))
        .width(Fill)
        .placeholder("Select an installed Editor")
        .into()
    };

    let content = column![
        text("Change project Editor version").size(20),
        text(vm.project_title.as_str())
            .size(12)
            .style(theme::muted_text),
        row![
            column![
                text("Current version").size(10).style(theme::faint_text),
                text(vm.current_version.as_str()).size(12),
            ]
            .spacing(2)
            .width(Fill),
            column![
                text("Project path").size(10).style(theme::faint_text),
                text(vm.project_path_short.as_str())
                    .size(10)
                    .font(iced::Font::MONOSPACE)
                    .style(theme::muted_text),
            ]
            .spacing(2)
            .width(Fill),
        ]
        .spacing(14),
        text("Target Editor version").size(10).style(theme::faint_text),
        picker,
        container(
            text("This runs `unity projects upgrade <path> --to <version> --yes`. It can update ProjectVersion.txt, open the Editor, and run API upgrades. Commit or back up first.")
                .size(10)
                .style(theme::warning_text),
        )
        .padding(10)
        .width(Fill)
        .style(theme::notice_card(Tone::Warning)),
        checkbox(vm.confirmed)
            .label("I understand and want to upgrade this project now.")
            .on_toggle(UiAction::UpgradeConfirmed)
            .size(14)
            .text_size(ui::font_size(11.0)),
        row![
            space().width(Fill),
            button(text("Cancel").size(11))
                .on_press(UiAction::CloseModal)
                .padding([7.0, 12.0])
                .style(button::text),
            button(text("Upgrade project").size(11))
                .on_press_maybe(vm.can_upgrade.then_some(UiAction::StartUpgrade))
                .padding([7.0, 12.0])
                .style(button::primary),
        ]
        .spacing(8),
    ]
    .spacing(12);

    container(content)
        .width(600)
        .padding(22)
        .style(theme::modal)
        .into()
}
