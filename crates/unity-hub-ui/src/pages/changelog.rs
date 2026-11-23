use iced::widget::{column, container, pick_list, row, space};
use iced::{Center, Element, Fill};

use unity_hub_model::{
    ChangelogDataVm, ChangelogEntryVm, ChangelogSectionVm, ChangelogVm, DocsScopeVm, EntryDocsVm,
    SectionKind, Tone, UiAction,
};

use crate::components::{
    badge, button, card_with, checkbox, empty, error_text, hint, icon_button, loading, notice,
    page_header, scroll, stat_strip, status_dot, surface, text_input, ButtonKind, COMPACT,
    PICK_LIST_PADDING,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::{self, text};

const MAX_SECTION_ENTRIES: usize = 250;

pub fn view(vm: &ChangelogVm) -> Element<'_, UiAction> {
    let controls = row![
        pick_list(
            vm.versions.clone(),
            vm.selected_target.clone(),
            UiAction::ChangelogTargetChanged,
        )
        .text_size(ui::font_size(font::BODY))
        .padding(PICK_LIST_PADDING)
        .handle(icons::pick_list_handle(font::ICON_SMALL))
        .style(theme::pick_list)
        .menu_style(theme::menu)
        .width(200)
        .placeholder("Installed CLI"),
        button(
            if vm.loaded {
                "Refresh notes"
            } else {
                "Show changelog"
            },
            ButtonKind::Primary,
        )
        .on_press(UiAction::LoadChangelog),
    ]
    .spacing(8)
    .align_y(Center);

    column![
        page_header(
            "Changelog",
            "Release notes from the installed CLI, with docs links and a filter for the commands, flags and error codes this app uses.",
            Some(controls.into()),
        ),
        changelog_body(vm),
    ]
    .spacing(spacing::SECTION)
    .into()
}

fn entries_of(data: &ChangelogDataVm, kind: SectionKind) -> usize {
    data.sections
        .iter()
        .filter(|section| section.kind == kind)
        .map(|section| section.entries.len())
        .sum()
}

fn changelog_body(vm: &ChangelogVm) -> Element<'_, UiAction> {
    if !vm.loaded {
        return surface(empty(
            "Changelog not loaded yet",
            "Press “Show changelog” to read the installed CLI's release notes and flag entries that may break workflows.",
        ));
    }

    if let Some(error) = vm.state.error() {
        return surface(error_text(error));
    }

    let Some(data) = vm.state.data() else {
        if vm.state.is_loading() {
            return surface(loading("Reading release notes…"));
        }
        return surface(empty(
            "No changelog data",
            "The CLI returned no release notes.",
        ));
    };

    let breaking = data.breaking_changes.len();
    let mut body = column![
        filter_bar(vm, data),
        row![
            column![
                text(format!("Unity CLI {}", data.version))
                    .size(font::MODAL_TITLE)
                    .font(fonts::semibold()),
                hint(data.source_detail.as_str()),
            ]
            .spacing(3)
            .width(Fill),
            badge(data.source.clone(), Tone::Neutral),
        ]
        .align_y(Center),
        stat_strip(vec![
            (
                data.entry_count.to_string(),
                "Entries parsed".to_string(),
                None
            ),
            (
                breaking.to_string(),
                "Possible breaking".to_string(),
                (breaking > 0).then_some(Tone::Danger),
            ),
            (
                data.app_impact_count.to_string(),
                "Affects this app".to_string(),
                (data.app_impact_count > 0).then_some(Tone::Accent),
            ),
            (
                entries_of(data, SectionKind::Added).to_string(),
                "Added".to_string(),
                None,
            ),
        ]),
        breaking_changes(data),
    ]
    .spacing(spacing::SECTION);

    let sections = data
        .sections
        .iter()
        .filter(|section| {
            !vm.app_impact_only || section.entries.iter().any(|entry| entry.app_impact)
        })
        .collect::<Vec<&ChangelogSectionVm>>();

    if sections.is_empty() {
        let empty = if vm.app_impact_only {
            column![
                text("No entry names a command, flag, error code or exit code this app uses.")
                    .size(font::BODY_LARGE)
                    .font(fonts::medium()),
                hint("Turn off “Affects this app” to read the full release notes."),
            ]
            .spacing(3)
        } else {
            column![
                text("The CLI returned no release-note entries.")
                    .size(font::BODY_LARGE)
                    .font(fonts::medium()),
                hint("Try another CLI version, or use the raw Markdown view."),
            ]
            .spacing(3)
        };
        body = body.push(notice(Tone::Info, icons::INFO, empty));
    } else {
        // Sections sit two per row.
        for pair in sections.chunks(2) {
            let mut line = row![]
                .spacing(spacing::SECTION)
                .align_y(iced::Alignment::Start);
            for section in pair {
                line = line.push(container(section_card(section, vm.app_impact_only)).width(Fill));
            }
            if pair.len() == 1 {
                line = line.push(space().width(Fill));
            }
            body = body.push(line);
        }
    }

    body = body.push(
        row![
            button(
                if vm.show_raw {
                    "Hide raw Markdown"
                } else {
                    "Show raw Markdown"
                },
                ButtonKind::Ghost,
            )
            .on_press(UiAction::ToggleRawChangelog),
            hint("Provider: unity-changelog"),
        ]
        .spacing(8)
        .align_y(Center),
    );

    if vm.show_raw {
        let raw: Element<'_, UiAction> = if let Some(error) = vm.raw.error() {
            error_text(error)
        } else if let Some(markdown) = vm.raw.data() {
            scroll(
                text(markdown.as_str())
                    .size(font::MONO)
                    .font(fonts::mono())
                    .line_height(1.55)
                    .style(theme::muted_text),
            )
            .height(320)
            .into()
        } else if vm.raw.is_loading() {
            loading("Loading raw Markdown…")
        } else {
            hint("Raw Markdown is not loaded.")
        };

        body = body.push(
            container(raw)
                .padding([14.0, 16.0])
                .width(Fill)
                .style(theme::inset),
        );
    }

    body.into()
}

fn filter_bar<'a>(vm: &'a ChangelogVm, data: &'a ChangelogDataVm) -> Element<'a, UiAction> {
    let mut controls = row![
        checkbox(
            format!("Affects this app ({})", data.app_impact_count),
            vm.app_impact_only,
            UiAction::ToggleChangelogImpact,
        ),
        space().width(Fill),
        hint("Docs scope"),
        pick_list(
            DocsScopeVm::ALL.to_vec(),
            Some(vm.docs_scope),
            UiAction::ChangelogDocsScopeChanged,
        )
        .text_size(ui::font_size(font::BODY))
        .padding(PICK_LIST_PADDING)
        .handle(icons::pick_list_handle(font::ICON_SMALL))
        .style(theme::pick_list)
        .menu_style(theme::menu)
        .width(190),
    ]
    .spacing(10)
    .align_y(Center);

    if vm.docs_scope == DocsScopeVm::Package {
        controls = controls.push(
            text_input("Package name", &vm.docs_package)
                .on_input(UiAction::ChangelogDocsPackageChanged)
                .width(220),
        );
    }

    let mut content = column![controls].spacing(6).width(Fill);
    if let Some(note) = &vm.docs_scope_note {
        content = content.push(hint(note.as_str()));
    }
    container(content)
        .padding([12.0, 14.0])
        .width(Fill)
        .style(theme::raised)
        .into()
}

fn breaking_changes(data: &ChangelogDataVm) -> Element<'_, UiAction> {
    if data.breaking_changes.is_empty() {
        return notice(
            Tone::Success,
            icons::CIRCLE_CHECK,
            column![
                text("No likely breaking changes detected.")
                    .size(font::BODY_LARGE)
                    .font(fonts::medium()),
                text("This is a text heuristic, not a guarantee. Read the Changed and Removed sections before updating.")
                    .size(font::BODY)
                    .style(theme::muted_text),
            ]
            .spacing(3),
        );
    }

    let items = column(
        data.breaking_changes
            .iter()
            .enumerate()
            .map(|(index, change)| {
                container(
                    row![
                        text(format!("{:02}", index + 1))
                            .size(font::MONO)
                            .font(fonts::mono())
                            .style(theme::faint_text)
                            .width(28),
                        column![
                            text(change.text.as_str())
                                .size(font::BODY_LARGE)
                                .line_height(1.5)
                                .width(Fill),
                            row![
                                badge(change.section.clone(), Tone::Danger),
                                badge(format!("matched “{}”", change.reason), Tone::Neutral),
                            ]
                            .spacing(6),
                        ]
                        .spacing(10)
                        .width(Fill),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Start),
                )
                .padding(16)
                .width(Fill)
                .style(theme::raised)
                .into()
            })
            .collect::<Vec<Element<'_, UiAction>>>(),
    )
    .spacing(8);

    surface(
        column![
            row![
                icon(icons::TRIANGLE_ALERT, font::ICON).style(theme::danger_text),
                crate::components::card_title("Possible breaking changes"),
                space().width(Fill),
                hint("Heuristic, not a guarantee"),
            ]
            .spacing(10)
            .align_y(Center),
            container(scroll(items)).max_height(340),
        ]
        .spacing(16),
    )
}

fn section_card(section: &ChangelogSectionVm, app_impact_only: bool) -> Element<'_, UiAction> {
    let tone = section_tone(section.kind);
    let visible = section
        .entries
        .iter()
        .filter(|entry| !app_impact_only || entry.app_impact)
        .collect::<Vec<&ChangelogEntryVm>>();
    let mut entries = column(
        visible
            .iter()
            .take(MAX_SECTION_ENTRIES)
            .map(|entry| entry_row(entry, tone))
            .collect::<Vec<Element<'_, UiAction>>>(),
    )
    .spacing(12);

    if visible.len() > MAX_SECTION_ENTRIES {
        entries = entries.push(hint(format!(
            "… {} more entries. Use the raw Markdown view for the full list.",
            visible.len() - MAX_SECTION_ENTRIES
        )));
    }

    card_with(
        section.title.as_str(),
        hint(visible.len().to_string()),
        entries.into(),
    )
}

fn entry_row<'a>(entry: &'a ChangelogEntryVm, tone: Tone) -> Element<'a, UiAction> {
    let mut content = column![text(entry.text.as_str())
        .size(font::BODY_LARGE)
        .line_height(1.55)
        .style(theme::muted_text)
        .width(Fill),]
    .spacing(7)
    .width(Fill);

    if let Some(docs) = docs_line(&entry.docs) {
        content = content.push(docs);
    }

    row![container(status_dot(tone)).padding([7.0, 0.0]), content,]
        .spacing(12)
        .align_y(iced::Alignment::Start)
        .into()
}

fn docs_line<'a>(docs: &'a EntryDocsVm) -> Option<Element<'a, UiAction>> {
    match docs {
        EntryDocsVm::Unavailable => None,
        EntryDocsVm::Idle { topic, scope } => Some(
            row![
                icon_button(icons::SEARCH, "Docs", ButtonKind::Ghost)
                    .padding(COMPACT)
                    .on_press(UiAction::LookupChangelogDocs {
                        topic: topic.clone(),
                        scope: *scope,
                    }),
                hint(format!("Look up `{topic}`")),
            ]
            .spacing(8)
            .align_y(Center)
            .into(),
        ),
        EntryDocsVm::Loading { topic } => Some(loading(format!("Finding docs for `{topic}`…"))),
        EntryDocsVm::Ready { topic, url, search } => {
            let mut line = row![
                icon_button(icons::EXTERNAL_LINK, "Open docs", ButtonKind::Ghost)
                    .padding(COMPACT)
                    .on_press(UiAction::OpenExternalUrl(url.clone())),
            ]
            .spacing(8)
            .align_y(Center);
            line = line.push(if *search {
                badge("Search result", Tone::Warning)
            } else {
                badge(topic.clone(), Tone::Neutral)
            });
            line = line.push(hint(url.as_str()));
            Some(line.into())
        }
        EntryDocsVm::Missing { topic } => Some(hint(format!("No docs page found for `{topic}`."))),
        EntryDocsVm::Failed { message, .. } => Some(hint(format!("Docs: {message}"))),
    }
}

fn section_tone(kind: SectionKind) -> Tone {
    match kind {
        SectionKind::Removed => Tone::Danger,
        SectionKind::Changed => Tone::Warning,
        SectionKind::Added => Tone::Success,
        SectionKind::Fixed => Tone::Info,
        SectionKind::Security => Tone::Highlight,
        SectionKind::Other => Tone::Neutral,
    }
}
