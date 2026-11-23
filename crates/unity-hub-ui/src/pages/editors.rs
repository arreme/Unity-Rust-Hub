use iced::widget::{column, container, row, space};
use iced::{Center, Element, Fill, FillPortion};

use unity_hub_model::{
    EditorRowVm, EditorsVm, InstallAdvancedVm, InstallEditorVm, LoadState, ModuleOptionVm,
    PruneReportVm, ReleaseRowVm, Tone, UiAction,
};

use crate::components::{
    self, badge, button, caption, card, cell, checkbox, detail_column, divided, empty, error_text,
    field_label, hint, icon_button, list_row, loading, mono, notice, page_header, refresh_button,
    scroll, segmented, surface, text_input, ButtonKind,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::{self, text};

pub fn view(vm: &EditorsVm) -> Element<'_, UiAction> {
    column![
        page_header(
            "Editors",
            "What's installed, add modules, or install a new version.",
            Some(refresh_button()),
        ),
        installed_card(&vm.installed),
        prune_card(&vm.prune),
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

    list_row(
        row![
            cell(version, FillPortion(3)),
            cell(
                mono(if editor.architecture.trim().is_empty() {
                    "host default"
                } else {
                    editor.architecture.as_str()
                }),
                FillPortion(2),
            ),
            cell(modules, FillPortion(3)),
            cell(
                row![
                    button("Modules", ButtonKind::Secondary)
                        .on_press(UiAction::OpenEditorModules(
                            editor.version.clone(),
                            editor.architecture.clone(),
                        ))
                        .padding(components::COMPACT),
                    button("Verify", ButtonKind::Ghost)
                        .on_press(UiAction::OpenVerify(
                            editor.version.clone(),
                            editor.architecture.clone(),
                        ))
                        .padding(components::COMPACT),
                    default_button(editor),
                ]
                .spacing(8)
                .align_y(Center),
                FillPortion(4),
            ),
        ]
        .spacing(16)
        .align_y(Center),
    )
}

/// Keeps the module strip inside its column: a couple of names, then a `+N`
/// chip. The full list stays available in the modules dialog.
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

/// Sets or clears the stored default for the clicked build. A version with two
/// builds still yields an unambiguous action because the row carries the build.
fn default_button(editor: &EditorRowVm) -> Element<'_, UiAction> {
    if editor.is_default {
        button("Clear default", ButtonKind::Ghost)
            .on_press(UiAction::ClearDefaultEditor)
            .padding(components::COMPACT)
            .into()
    } else {
        button("Set default", ButtonKind::Secondary)
            .on_press(UiAction::SetDefaultEditor(
                editor.version.clone(),
                editor.architecture.clone(),
            ))
            .padding(components::COMPACT)
            .into()
    }
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
            let header = list_row(
                row![
                    cell(caption("Editor"), FillPortion(3)),
                    cell(caption("Architecture"), FillPortion(2)),
                    cell(caption("Modules"), FillPortion(3)),
                    cell(caption("Actions"), FillPortion(4)),
                ]
                .spacing(16)
                .align_y(Center),
            );
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

/// Maintenance entry point. The scan is report-only; the destructive actions
/// live behind the dialog's explicit confirmations.
fn prune_card(prune: &LoadState<PruneReportVm>) -> Element<'_, UiAction> {
    let summary = match prune {
        LoadState::Idle => "Not scanned yet.".to_string(),
        LoadState::Loading { previous } => match previous {
            Some(report) => format!(
                "Rescanning… last report: {} unused · {} reclaimable",
                report.unused.len(),
                report.reclaimable
            ),
            None => "Scanning installed Editors…".to_string(),
        },
        LoadState::Ready(report) => format!(
            "{} unused Editor{} · {} reclaimable",
            report.unused.len(),
            if report.unused.len() == 1 { "" } else { "s" },
            report.reclaimable
        ),
        LoadState::Failed { .. } => "The last scan failed.".to_string(),
    };

    card(
        "Maintenance",
        column![
            text("Find installed Editors no registered project uses, or drop registry entries whose folder is already gone. Every action starts with a report and needs a confirmation.")
                .size(font::BODY)
                .line_height(1.5)
                .style(theme::muted_text),
            row![
                hint(summary),
                space().width(Fill),
                button("Review unused Editors", ButtonKind::Secondary)
                    .on_press(UiAction::OpenPrune),
            ]
            .spacing(12)
            .align_y(Center),
        ]
        .spacing(12)
        .into(),
    )
}

fn install_card(install: &InstallEditorVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(16);

    let architecture_picker = segmented(
        install
            .architecture_options
            .iter()
            .map(|option| {
                (
                    option.label.clone(),
                    option.selected,
                    UiAction::InstallArchitectureChanged(option.architecture.clone()),
                )
            })
            .collect(),
    );

    body = body.push(
        row![
            column![
                field_label("Editor version"),
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
            .spacing(8)
            .width(FillPortion(3)),
            column![
                field_label("Architecture"),
                architecture_picker,
                hint("Host default leaves --architecture out and lets the CLI choose this machine's build."),
            ]
            .spacing(8)
            .width(FillPortion(2)),
        ]
        .spacing(16)
        .align_y(iced::Alignment::Start),
    );

    if let Some(warning) = &install.second_build_warning {
        body = body.push(notice(
            Tone::Warning,
            icons::TRIANGLE_ALERT,
            text(warning.as_str())
                .size(font::BODY)
                .line_height(1.5)
                .style(theme::muted_text),
        ));
    }

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

    if install.headless_available {
        body = body.push(
            column![
                checkbox(
                    "Install the headless Editor (--headless, runs without a display)",
                    install.headless,
                    UiAction::HeadlessToggled,
                ),
                hint(
                    "Headless builds are Linux-only and version-dependent: the CLI reports INSTALL_HEADLESS_UNAVAILABLE when a version has none. Modules it cannot use, such as iOS, are skipped.",
                ),
            ]
            .spacing(6),
        );
    }

    body = body.push(checkbox(
        "Accept the module license agreements for this install (--accept-eula)",
        install.accept_eula,
        UiAction::AcceptEulaToggled,
    ));

    body = body.push(install_advanced(&install.advanced));

    if let Some(resume) = &install.resume_offer {
        body = body.push(notice(
            Tone::Warning,
            icons::REFRESH,
            row![
                column![
                    text(format!("Resume Unity {}?", resume.version))
                        .size(font::BODY_LARGE)
                        .font(fonts::medium()),
                    text(resume.reason.as_str())
                        .size(font::BODY)
                        .style(theme::muted_text),
                    hint(format!(
                        "Reuses the cache for the {} build instead of downloading again.",
                        resume.architecture
                    )),
                ]
                .spacing(4)
                .width(Fill),
                button("Resume install", ButtonKind::Primary)
                    .on_press_maybe(resume.can_resume.then_some(UiAction::ResumeInstallEditor)),
            ]
            .spacing(12)
            .align_y(Center),
        ));
    }

    if let Some(report) = &install.download_report {
        body = body.push(notice(
            Tone::Success,
            icons::DOWNLOAD,
            text(report.as_str())
                .size(font::BODY)
                .line_height(1.5)
                .style(theme::muted_text),
        ));
    }

    if let Some(error) = &install.preview_error {
        body = body.push(error_text(error));
    }

    if let Some(preview) = &install.preview {
        body = body.push(
            container(
                row![
                    detail_column("Editor", preview.editor_version.as_str()),
                    detail_column(
                        "Architecture",
                        preview.architecture.as_deref().unwrap_or("Host default")
                    ),
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
    body = body.push(hint("Exact command"));
    body = body.push(mono(install.command_preview.as_str()));
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

fn install_advanced(advanced: &InstallAdvancedVm) -> Element<'_, UiAction> {
    let toggle = if advanced.expanded {
        "Hide advanced options"
    } else {
        "Advanced options"
    };
    let summary = if advanced.expanded {
        "Optional install flags. The default command stays unchanged while these are untouched."
            .to_string()
    } else {
        "Resume, download-only, lock wait, timeout, set-default, download retries.".to_string()
    };

    let mut body = column![row![
        button(toggle, ButtonKind::Ghost)
            .on_press(UiAction::InstallAdvancedToggled(!advanced.expanded))
            .padding(components::COMPACT),
        hint(summary),
    ]
    .spacing(10)
    .align_y(Center),]
    .spacing(10);

    if advanced.expanded {
        let mut first_row = row![
            checkbox(
                "Resume an interrupted download (--resume)",
                advanced.resume,
                UiAction::InstallResumeToggled,
            )
            .width(Fill),
            checkbox(
                "Download and verify only (--download-only)",
                advanced.download_only,
                UiAction::InstallDownloadOnlyToggled,
            )
            .width(Fill),
        ]
        .spacing(16);

        if advanced.download_only {
            first_row = first_row.push(hint("No Editor is installed or registered."));
        }

        body = body.push(first_row);
        body = body.push(
            row![
                checkbox(
                    "Fail at once when another install holds the lock (--no-wait)",
                    advanced.no_wait,
                    UiAction::InstallNoWaitToggled,
                )
                .width(Fill),
                checkbox(
                    "Make this Editor the default after success (--set-default)",
                    advanced.set_default,
                    UiAction::InstallSetDefaultToggled,
                )
                .width(Fill),
            ]
            .spacing(16),
        );

        let mut timeout_column = column![
            field_label("Lock wait timeout (seconds)"),
            text_input("600", &advanced.timeout_input).on_input(UiAction::InstallTimeoutChanged),
        ]
        .spacing(6);
        if let Some(error) = &advanced.timeout_error {
            timeout_column = timeout_column.push(hint(error.as_str()));
        } else {
            timeout_column = timeout_column.push(hint(
                "Whole seconds. No-wait and timeout are mutually exclusive, so setting one clears the other.",
            ));
        }

        let mut retries_column = column![
            field_label("Download retries (0 disables)"),
            text_input("2", &advanced.retries_input).on_input(UiAction::InstallRetriesChanged),
        ]
        .spacing(6);
        if let Some(error) = &advanced.retries_error {
            retries_column = retries_column.push(hint(error.as_str()));
        } else {
            retries_column = retries_column.push(hint(
                "Set through UNITY_INSTALL_RETRIES; `unity install` has no --retries flag.",
            ));
        }

        body = body.push(
            row![
                container(timeout_column).width(Fill),
                container(retries_column).width(Fill)
            ]
            .spacing(16),
        );

        if let Some(note) = &advanced.conflict_note {
            body = body.push(notice(
                Tone::Info,
                icons::INFO,
                text(note.as_str())
                    .size(font::BODY)
                    .style(theme::muted_text),
            ));
        }
    }

    container(body)
        .padding([14.0, 16.0])
        .width(Fill)
        .style(theme::outline)
        .into()
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
            .on_press(UiAction::UseRelease(
                release.version.clone(),
                release.architecture.clone(),
            ))
            .padding([6.0, 14.0])
            .into()
    };

    let version = {
        let mut value = column![text(release.version.as_str())
            .size(font::BODY_LARGE)
            .font(fonts::medium())]
        .spacing(2);
        if !release.architecture.trim().is_empty() {
            value = value.push(
                text(release.architecture.as_str())
                    .size(font::SMALL)
                    .font(fonts::mono())
                    .style(theme::muted_text),
            );
        }
        value.width(Fill)
    };

    list_row(
        row![
            version,
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
