use iced::widget::{column, container, row, space};
use iced::{Center, Element, Fill, FillPortion, Length};

use unity_hub_model::{
    EditorRowVm, EditorsVm, InstallEditorVm, LoadState, ModuleOptionVm, ReleaseRowVm, Tone,
    UiAction,
};

use crate::components::{
    self, badge, button, caption, card, cell, cell_centered, checkbox, detail_column, divided,
    empty, error_text, hint, icon_button, list_row, loading, mono, page_header, refresh_button,
    scroll, surface, text_input, ButtonKind,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::{self, text};

/// Room for "Manage modules" at 100% text; it grows with the text preference.
const MANAGE_WIDTH: f32 = 166.0;

pub fn view(vm: &EditorsVm) -> Element<'_, UiAction> {
    column![
        page_header(
            "Editors",
            "What's installed, add modules, or install a new version.",
            Some(refresh_button()),
        ),
        installed_card(&vm.installed),
        row![
            container(install_card(&vm.install)).width(FillPortion(3)),
            container(releases_card(&vm.releases)).width(FillPortion(2)),
        ]
        .spacing(spacing::SECTION)
        .align_y(iced::Alignment::Start),
    ]
    .spacing(spacing::SECTION)
    .into()
}

fn editor_row(editor: &EditorRowVm) -> Element<'_, UiAction> {
    let mut version = column![text(editor.version.as_str())
        .size(font::ROW_TITLE)
        .font(fonts::semibold())]
    .spacing(4)
    .width(Fill);
    if editor.is_default {
        version = version.push(
            row![
                icon(icons::STAR, 12.0).style(theme::muted_text),
                text("Default Editor")
                    .size(font::SMALL)
                    .style(theme::muted_text),
            ]
            .spacing(6)
            .align_y(Center),
        );
    }

    let modules: Element<'_, UiAction> = if editor.modules.is_empty() {
        hint("No modules reported")
    } else {
        row(module_badges(&editor.modules)).spacing(6).into()
    };

    list_row(editor_columns(
        version.into(),
        mono(editor.architecture.as_str()),
        modules,
        button("Manage modules", ButtonKind::Secondary)
            .on_press(UiAction::OpenEditorModules(editor.version.clone()))
            .padding(components::COMPACT)
            .into(),
    ))
}

fn editor_columns<'a>(
    editor: Element<'a, UiAction>,
    architecture: Element<'a, UiAction>,
    modules: Element<'a, UiAction>,
    action: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    row![
        cell(editor, FillPortion(3)),
        cell(architecture, FillPortion(2)),
        cell_centered(modules, FillPortion(5)),
        cell(action, Length::Fixed(ui::scaled_pixels(MANAGE_WIDTH))),
    ]
    .spacing(16)
    .align_y(Center)
    .into()
}

/// Keeps the module strip inside its column: a couple of names, then a `+N`
/// chip. The full list stays available in the Manage modules dialog.
fn module_badges(modules: &[String]) -> Vec<Element<'static, UiAction>> {
    let budget = (42.0 / ui::scale()).max(12.0) as usize;
    let mut used = 0;
    let mut shown = Vec::new();

    for module in modules {
        let chars = module.chars().count() + 4;
        if shown.len() >= 2 || used + chars > budget {
            break;
        }
        used += chars;
        shown.push(badge(module.clone(), Tone::Neutral));
    }

    if shown.len() < modules.len() {
        shown.push(badge(
            format!("+{}", modules.len() - shown.len()),
            Tone::Neutral,
        ));
    }

    shown
}

fn installed_card(installed: &LoadState<Vec<EditorRowVm>>) -> Element<'_, UiAction> {
    let body: Element<'_, UiAction> = if let Some(error) = installed.error() {
        container(error_text(error)).padding([20.0, 0.0]).into()
    } else if let Some(editors) = installed.data() {
        if editors.is_empty() {
            empty(
                "No Editors installed",
                "Use the installer below. LTS is the safest default.",
            )
        } else {
            let header = list_row(editor_columns(
                caption("Editor"),
                caption("Architecture"),
                container(caption("Modules"))
                    .width(Fill)
                    .align_x(Center)
                    .into(),
                space().width(0).into(),
            ));
            let mut rows = vec![header];
            rows.extend(editors.iter().map(editor_row));
            divided(rows).into()
        }
    } else if installed.is_loading() {
        container(loading("Reading installed Editors…"))
            .padding([20.0, 0.0])
            .into()
    } else {
        space().height(0).into()
    };

    container(body)
        .padding([4.0, spacing::CARD])
        .width(Fill)
        .style(theme::card)
        .into()
}

fn install_card(install: &InstallEditorVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(16);

    body = body.push(
        column![
            row![
                text_input(
                    "lts, latest, 6000.3, or an exact version",
                    &install.version_input
                )
                .on_input(UiAction::InstallVersionChanged)
                .icon(icons::input_icon(icons::SEARCH, font::ICON_SMALL))
                .width(Fill),
                button("Load modules", ButtonKind::Secondary).on_press(UiAction::LoadModules),
            ]
            .spacing(8)
            .align_y(Center),
            hint("Aliases: lts, latest, or a stream such as 6000.3. Exact versions also work."),
        ]
        .spacing(8),
    );

    if let Some(error) = install.modules.error() {
        body = body.push(error_text(error));
    } else if let Some(modules) = install.modules.data() {
        body = body.push(
            row![
                container(caption(format!(
                    "{} modules available",
                    install.available_count
                )))
                .width(Fill),
                button("Select all", ButtonKind::Ghost)
                    .on_press(UiAction::SelectAllModules(true))
                    .padding([6.0, 12.0]),
                button("Clear", ButtonKind::Ghost)
                    .on_press(UiAction::SelectAllModules(false))
                    .padding([6.0, 12.0]),
            ]
            .align_y(Center),
        );

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

        body = body.push(
            container(container(scroll(list)).max_height(240))
                .width(Fill)
                .style(theme::outline),
        );
    } else if install.modules.is_loading() {
        body = body.push(loading("Loading module catalog…"));
    } else {
        body = body.push(hint(
            "Load a version to choose modules. Installing without modules is allowed.",
        ));
    }

    body = body.push(checkbox(
        "Accept the module license agreements for this install (--accept-eula)",
        install.accept_eula,
        UiAction::AcceptEulaToggled,
    ));

    if let Some(preview) = &install.preview {
        body = body.push(
            container(
                row![
                    detail_column("Editor", preview.editor_version.as_str()),
                    detail_column("Download", preview.download_size.as_str()),
                    detail_column("Modules", preview.module_count.to_string()),
                ]
                .spacing(20),
            )
            .padding([14.0, 16.0])
            .width(Fill)
            .style(theme::raised),
        );
    } else if let Some(note) = &install.preview_note {
        body = body.push(hint(note.as_str()));
    }

    let preview_status: Element<'_, UiAction> = if install.preview_loading {
        loading("Preparing preview…")
    } else {
        space().width(0).into()
    };

    body = body.push(components::divider());
    body = body.push(
        row![
            button("Preview download", ButtonKind::Secondary).on_press(UiAction::PreviewInstall),
            icon_button(icons::DOWNLOAD, "Install Editor", ButtonKind::Primary)
                .on_press_maybe(install.can_install.then_some(UiAction::StartInstallEditor)),
            space().width(Fill),
            preview_status,
        ]
        .spacing(8)
        .align_y(Center),
    );

    card("Install a new Editor", body.into())
}

fn module_checkbox(module: &ModuleOptionVm) -> Element<'static, UiAction> {
    let id = module.id.clone();
    checkbox(module.label.clone(), module.selected, move |value| {
        UiAction::ToggleModule(id.clone(), value)
    })
    .width(Fill)
    .into()
}

fn release_row(release: &ReleaseRowVm) -> Element<'_, UiAction> {
    let action: Element<'_, UiAction> = if release.installed {
        row![
            icon(icons::CHECK, 12.0).style(theme::faint_text),
            text("Installed").size(font::SMALL).style(theme::faint_text),
        ]
        .spacing(5)
        .align_y(Center)
        .into()
    } else {
        button("Use", ButtonKind::Secondary)
            .on_press(UiAction::UseRelease(release.version.clone()))
            .padding([6.0, 14.0])
            .into()
    };

    list_row(
        row![
            text(release.version.as_str())
                .size(font::BODY_LARGE)
                .font(fonts::medium())
                .width(Fill),
            badge(release.stream.clone(), release.stream_tone),
            container(action).width(84).align_right(84),
        ]
        .spacing(12)
        .align_y(Center),
    )
}

fn releases_card(releases: &LoadState<Vec<ReleaseRowVm>>) -> Element<'_, UiAction> {
    let body: Element<'_, UiAction> = if let Some(error) = releases.error() {
        error_text(error)
    } else if let Some(releases) = releases.data() {
        if releases.is_empty() {
            empty("No releases returned", "Check network or proxy settings.")
        } else {
            divided(releases.iter().take(14).map(release_row).collect()).into()
        }
    } else if releases.is_loading() {
        loading("Fetching releases…")
    } else {
        space().height(0).into()
    };

    surface(
        column![components::card_title("Available releases"), body]
            .spacing(4)
            .width(Fill),
    )
}
