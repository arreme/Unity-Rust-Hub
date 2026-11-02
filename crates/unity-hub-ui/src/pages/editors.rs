use iced::widget::{button, checkbox, column, container, row, scrollable, space, text_input};
use iced::{Element, Fill};

use unity_hub_model::{
    EditorRowVm, EditorsVm, InstallEditorVm, LoadState, ModuleOptionVm, ReleaseRowVm, Tone,
    UiAction,
};

use crate::components::{
    badge, card, cell, detail_column, empty, header_cell, page_header, row_card, row_header,
};
use crate::theme;
use crate::ui::{self, text};

pub fn view(vm: &EditorsVm) -> Element<'_, UiAction> {
    let mut page = column![page_header(
        "TOOLCHAIN",
        "Editors & modules",
        "See what is installed, add modules, and install a new Editor version.",
    )]
    .spacing(16);

    page = page.push(installed_card(&vm.installed));
    page = page.push(
        row![install_card(&vm.install), releases_card(&vm.releases)]
            .spacing(16)
            .align_y(iced::Alignment::Start),
    );

    page.into()
}

fn installed_card(installed: &LoadState<Vec<EditorRowVm>>) -> Element<'_, UiAction> {
    let mut body = column![].spacing(10);

    if let Some(error) = installed.error() {
        body = body.push(text(error.text()).size(10).style(theme::danger_text));
    } else if let Some(editors) = installed.data() {
        if editors.is_empty() {
            body = body.push(empty(
                "No Editors installed",
                "Use the installer on this page. LTS is the safest default.",
            ));
        } else {
            body = body.push(row_header(vec![
                header_cell("Version", 2),
                header_cell("Arch", 2),
                header_cell("Default", 2),
                header_cell("Modules", 5),
                header_cell("", 2),
            ]));

            for editor in editors {
                let default = if editor.is_default {
                    badge("default", Tone::Success)
                } else {
                    text("—").size(10).style(theme::faint_text).into()
                };

                body = body.push(row_card(vec![
                    cell(text(editor.version.as_str()).size(11), 2),
                    cell(text(editor.architecture.as_str()).size(10), 2),
                    cell(default, 2),
                    cell(
                        text(editor.modules.as_str())
                            .size(10)
                            .style(theme::muted_text),
                        5,
                    ),
                    cell(
                        button(text("Manage").size(10))
                            .on_press(UiAction::OpenEditorModules(editor.version.clone()))
                            .padding([4.0, 8.0])
                            .style(button::secondary),
                        2,
                    ),
                ]));
            }
        }
    } else if installed.is_loading() {
        body = body.push(
            text("Reading installed Editors…")
                .size(11)
                .style(theme::muted_text),
        );
    }

    card("Installed Editors", body.into())
}

fn install_card(install: &InstallEditorVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(10);

    body = body.push(
        row![
            text_input(
                "lts, latest, 6000.3, or an exact version",
                &install.version_input
            )
            .on_input(UiAction::InstallVersionChanged)
            .padding(8)
            .size(ui::font_size(12.0))
            .width(Fill),
            button(text("Load modules").size(10))
                .on_press(UiAction::LoadModules)
                .padding([6.0, 9.0])
                .style(button::secondary),
        ]
        .spacing(7)
        .align_y(iced::Alignment::Center),
    );

    body = body.push(
        text("Aliases: lts, latest, or a stream such as 6000.3. Exact versions also work.")
            .size(9)
            .style(theme::faint_text),
    );

    if let Some(error) = install.modules.error() {
        body = body.push(text(error.text()).size(10).style(theme::danger_text));
    } else if let Some(modules) = install.modules.data() {
        body = body.push(
            row![
                text(format!("{} modules available", install.available_count))
                    .size(9)
                    .style(theme::faint_text),
                space().width(Fill),
                button(text("Select all").size(9))
                    .on_press(UiAction::SelectAllModules(true))
                    .padding([3.0, 6.0])
                    .style(button::text),
                button(text("Clear").size(9))
                    .on_press(UiAction::SelectAllModules(false))
                    .padding([3.0, 6.0])
                    .style(button::text),
            ]
            .align_y(iced::Alignment::Center),
        );

        let list = column(modules.iter().map(module_checkbox).collect::<Vec<_>>()).spacing(5);

        body = body.push(scrollable(list).height(230));
    } else if install.modules.is_loading() {
        body = body.push(
            text("Loading module catalog…")
                .size(10)
                .style(theme::muted_text),
        );
    } else {
        body = body.push(
            text("Load a version to choose modules. Installing without modules is allowed.")
                .size(9)
                .style(theme::faint_text),
        );
    }

    body = body.push(
        checkbox(install.accept_eula)
            .label("Accept the module license agreements for this install (--accept-eula).")
            .on_toggle(UiAction::AcceptEulaToggled)
            .size(14)
            .text_size(ui::font_size(10.0)),
    );

    let preview_status: Element<'_, UiAction> = if install.preview_loading {
        text("Preparing preview…")
            .size(9)
            .style(theme::muted_text)
            .into()
    } else {
        space().width(0).into()
    };

    body = body.push(
        row![
            button(text("Preview download").size(10))
                .on_press(UiAction::PreviewInstall)
                .padding([6.0, 9.0])
                .style(button::secondary),
            button(text("Install Editor").size(10))
                .on_press_maybe(install.can_install.then_some(UiAction::StartInstallEditor))
                .padding([6.0, 9.0])
                .style(button::primary),
            space().width(Fill),
            preview_status,
        ]
        .spacing(7)
        .align_y(iced::Alignment::Center),
    );

    if let Some(preview) = &install.preview {
        body = body.push(
            container(
                row![
                    detail_column("Editor", preview.editor_version.as_str()),
                    detail_column("Download", preview.download_size.as_str()),
                    detail_column("Modules", preview.module_count.to_string()),
                ]
                .spacing(16),
            )
            .padding(10)
            .width(Fill)
            .style(theme::card),
        );
    } else if let Some(note) = &install.preview_note {
        body = body.push(text(note.as_str()).size(9).style(theme::muted_text));
    }

    card("Install a new Editor", body.into())
}

fn module_checkbox(module: &ModuleOptionVm) -> Element<'static, UiAction> {
    let id = module.id.clone();
    checkbox(module.selected)
        .label(module.label.clone())
        .on_toggle(move |value| UiAction::ToggleModule(id.clone(), value))
        .size(14)
        .text_size(ui::font_size(10.0))
        .into()
}

fn releases_card(releases: &LoadState<Vec<ReleaseRowVm>>) -> Element<'_, UiAction> {
    let mut body = column![].spacing(8);

    if let Some(error) = releases.error() {
        body = body.push(text(error.text()).size(10).style(theme::danger_text));
    } else if let Some(releases) = releases.data() {
        if releases.is_empty() {
            body = body.push(empty(
                "No releases returned",
                "Check network or proxy settings.",
            ));
        } else {
            body = body.push(row_header(vec![
                header_cell("Version", 4),
                header_cell("Stream", 3),
                header_cell("", 3),
            ]));

            for release in releases.iter().take(14) {
                let action: Element<'_, UiAction> = if release.installed {
                    badge("installed", Tone::Info)
                } else {
                    button(text("Use").size(9))
                        .on_press(UiAction::UseRelease(release.version.clone()))
                        .padding([3.0, 7.0])
                        .style(button::text)
                        .into()
                };

                body = body.push(row_card(vec![
                    cell(text(release.version.as_str()).size(10), 4),
                    cell(badge(release.stream.clone(), release.stream_tone), 3),
                    cell(action, 3),
                ]));
            }
        }
    } else if releases.is_loading() {
        body = body.push(text("Fetching releases…").size(10).style(theme::muted_text));
    }

    card("Available releases", body.into())
}
