use iced::widget::{button, checkbox, column, container, row, scrollable, space, text, text_input};
use iced::{Element, Fill};

use crate::app::{badge, detail_column, App, Message};
use crate::theme;
use crate::util::{error_text, format_bytes};
use crate::views::{card, cell, empty, header_cell, page_header, row_card, row_header};

pub fn view(app: &App) -> Element<'_, Message> {
    let mut page = column![page_header(
        "TOOLCHAIN",
        "Editors & modules",
        "See what is installed, add modules, and install a new Editor version.",
    )]
    .spacing(16);

    page = page.push(installed_card(app));
    page = page.push(
        row![install_card(app), releases_card(app)]
            .spacing(16)
            .align_y(iced::Alignment::Start),
    );

    page.into()
}

fn installed_card(app: &App) -> Element<'_, Message> {
    let mut body = column![].spacing(10);

    if app.editors.loading && app.editors.data.is_none() {
        body = body.push(
            text("Reading installed Editors…")
                .size(11)
                .style(theme::muted_text),
        );
    } else if let Some(error) = &app.editors.error {
        body = body.push(text(error_text(error)).size(10).style(theme::danger_text));
    } else if let Some(editors) = &app.editors.data {
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
                let modules = if editor.module_names.is_empty() {
                    "No modules reported".to_string()
                } else {
                    editor.module_names.join(", ")
                };
                let default = if editor.default {
                    badge("default", theme::Tone::Green)
                } else {
                    text("—").size(10).style(theme::faint_text).into()
                };

                body = body.push(row_card(vec![
                    cell(text(editor.version.clone()).size(11), 2),
                    cell(text(editor.architecture.clone()).size(10), 2),
                    cell(default, 2),
                    cell(text(modules).size(10).style(theme::muted_text), 5),
                    cell(
                        button(text("Manage").size(10))
                            .on_press(Message::OpenModules(editor.clone()))
                            .padding([4.0, 8.0])
                            .style(button::secondary),
                        2,
                    ),
                ]));
            }
        }
    }

    card("Installed Editors", body.into())
}

fn install_card(app: &App) -> Element<'_, Message> {
    let mut body = column![].spacing(10);

    body = body.push(
        row![
            text_input(
                "lts, latest, 6000.3, or an exact version",
                &app.install_version
            )
            .on_input(Message::InstallVersionChanged)
            .padding(8)
            .size(12)
            .width(Fill),
            button(text("Load modules").size(10))
                .on_press(Message::LoadModules)
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

    if app.modules.loading {
        body = body.push(
            text("Loading module catalog…")
                .size(10)
                .style(theme::muted_text),
        );
    } else if let Some(error) = &app.modules.error {
        body = body.push(text(error_text(error)).size(10).style(theme::danger_text));
    } else if let Some(modules) = &app.modules.data {
        let available = modules.iter().filter(|module| !module.is_installed).count();
        body = body.push(
            row![
                text(format!("{available} modules available"))
                    .size(9)
                    .style(theme::faint_text),
                space().width(Fill),
                button(text("Select all").size(9))
                    .on_press(Message::SelectAllModules(true))
                    .padding([3.0, 6.0])
                    .style(button::text),
                button(text("Clear").size(9))
                    .on_press(Message::SelectAllModules(false))
                    .padding([3.0, 6.0])
                    .style(button::text),
            ]
            .align_y(iced::Alignment::Center),
        );

        let list = column(
            modules
                .iter()
                .filter(|module| !module.is_installed)
                .map(|module| {
                    checkbox(app.selected_modules.contains(&module.id))
                        .label(format!(
                            "{}  ·  {}",
                            module.name,
                            format_bytes(module.download_bytes)
                        ))
                        .on_toggle({
                            let id = module.id.clone();
                            move |value| Message::ToggleModule(id.clone(), value)
                        })
                        .size(14)
                        .text_size(10)
                        .into()
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(5);

        body = body.push(scrollable(list).height(230));
    } else {
        body = body.push(
            text("Load a version to choose modules. Installing without modules is allowed.")
                .size(9)
                .style(theme::faint_text),
        );
    }

    body = body.push(
        checkbox(app.accept_eula)
            .label("Accept the module license agreements for this install (--accept-eula).")
            .on_toggle(Message::AcceptEulaToggled)
            .size(14)
            .text_size(10),
    );

    let preview_status: Element<'_, Message> = if app.preview_loading {
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
                .on_press(Message::PreviewInstall)
                .padding([6.0, 9.0])
                .style(button::secondary),
            button(text("Install Editor").size(10))
                .on_press_maybe(
                    (app.selected_modules.is_empty() || app.accept_eula)
                        .then_some(Message::StartInstallEditor),
                )
                .padding([6.0, 9.0])
                .style(button::primary),
            space().width(Fill),
            preview_status,
        ]
        .spacing(7)
        .align_y(iced::Alignment::Center),
    );

    if let Some(preview) = &app.preview {
        body = body.push(
            container(
                row![
                    detail_column(
                        "Editor",
                        preview
                            .editor
                            .as_ref()
                            .map(|editor| editor.version.clone())
                            .unwrap_or_else(|| app.install_version.clone()),
                    ),
                    detail_column(
                        "Download",
                        format_bytes(
                            preview.total_download_size.max(
                                preview
                                    .editor
                                    .as_ref()
                                    .map(|editor| editor.download_size)
                                    .unwrap_or(0),
                            )
                        ),
                    ),
                    detail_column(
                        "Modules",
                        format!("{}", preview.modules.len().max(app.selected_modules.len())),
                    ),
                ]
                .spacing(16),
            )
            .padding(10)
            .width(Fill)
            .style(theme::card),
        );
    } else if let Some(raw) = &app.preview_raw {
        body = body.push(
            text(format!("Dry-run returned: {raw}"))
                .size(9)
                .style(theme::muted_text),
        );
    }

    card("Install a new Editor", body.into())
}

fn releases_card(app: &App) -> Element<'_, Message> {
    let mut body = column![].spacing(8);

    if app.releases.loading {
        body = body.push(text("Fetching releases…").size(10).style(theme::muted_text));
    } else if let Some(error) = &app.releases.error {
        body = body.push(text(error_text(error)).size(10).style(theme::danger_text));
    } else if let Some(releases) = &app.releases.data {
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

            let installed = app.installed_versions();
            for release in releases.iter().take(14) {
                let tone = stream_tone(&release.stream);
                let is_installed = release.installed || installed.contains(&release.version);
                let action: Element<'_, Message> = if is_installed {
                    badge("installed", theme::Tone::Blue)
                } else {
                    button(text("Use").size(9))
                        .on_press(Message::UseRelease(release.version.clone()))
                        .padding([3.0, 7.0])
                        .style(button::text)
                        .into()
                };

                body = body.push(row_card(vec![
                    cell(text(release.version.clone()).size(10), 4),
                    cell(badge(release.stream.clone(), tone), 3),
                    cell(action, 3),
                ]));
            }
        }
    }

    card("Available releases", body.into())
}

fn stream_tone(stream: &str) -> theme::Tone {
    let stream = stream.to_lowercase();
    if stream.contains("lts") || stream.contains("supported") {
        theme::Tone::Green
    } else if stream.contains("beta") {
        theme::Tone::Amber
    } else if stream.contains("alpha") {
        theme::Tone::Red
    } else {
        theme::Tone::Neutral
    }
}
