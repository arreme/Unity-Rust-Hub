use iced::widget::{column, container, pick_list, row, space};
use iced::{Center, Element, Fill};

use unity_hub_model::{
    ChangelogDataVm, ChangelogSectionVm, ChangelogVm, SectionKind, Tone, UiAction,
};

use crate::components::{
    badge, button, card_with, empty, error_text, hint, loading, notice, page_header, scroll,
    stat_strip, status_dot, surface, ButtonKind, PICK_LIST_PADDING,
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
            "Release notes from the installed CLI, with a heuristic pass for breaking changes.",
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
                entries_of(data, SectionKind::Added).to_string(),
                "Added".to_string(),
                None,
            ),
            (
                entries_of(data, SectionKind::Changed).to_string(),
                "Changed".to_string(),
                None,
            ),
        ]),
        breaking_changes(data),
    ]
    .spacing(spacing::SECTION);

    // Sections sit two per row.
    for pair in data.sections.chunks(2) {
        let mut line = row![]
            .spacing(spacing::SECTION)
            .align_y(iced::Alignment::Start);
        for section in pair {
            line = line.push(container(section_card(section)).width(Fill));
        }
        if pair.len() == 1 {
            line = line.push(space().width(Fill));
        }
        body = body.push(line);
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

fn section_card(section: &ChangelogSectionVm) -> Element<'_, UiAction> {
    let tone = section_tone(section.kind);
    let mut entries = column(
        section
            .entries
            .iter()
            .take(MAX_SECTION_ENTRIES)
            .map(|entry| entry_row(entry, tone))
            .collect::<Vec<Element<'_, UiAction>>>(),
    )
    .spacing(12);

    if section.entries.len() > MAX_SECTION_ENTRIES {
        entries = entries.push(hint(format!(
            "… {} more entries. Use the raw Markdown view for the full list.",
            section.entries.len() - MAX_SECTION_ENTRIES
        )));
    }

    card_with(
        section.title.as_str(),
        hint(section.entries.len().to_string()),
        entries.into(),
    )
}

fn entry_row<'a>(entry: &'a str, tone: Tone) -> Element<'a, UiAction> {
    row![
        container(status_dot(tone)).padding([7.0, 0.0]),
        text(entry)
            .size(font::BODY_LARGE)
            .line_height(1.55)
            .style(theme::muted_text)
            .width(Fill),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Start)
    .into()
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
