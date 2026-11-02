use iced::widget::{button, column, container, pick_list, row, scrollable, space};
use iced::{Element, Fill};

use unity_hub_model::{ChangelogDataVm, ChangelogVm, SectionKind, Tone, UiAction};

use crate::components::{badge, card, empty, page_header};
use crate::theme;
use crate::ui::{self, text};

const MAX_SECTION_ENTRIES: usize = 250;

pub fn view(vm: &ChangelogVm) -> Element<'_, UiAction> {
    let mut page = column![page_header(
        "BETA WATCH",
        "Changelog & breaking changes",
        "Release notes come from the installed CLI. Breaking-change flags are a heuristic pass, ready for a remote provider.",
    )]
    .spacing(16);

    page = page.push(controls(vm));
    page = page.push(changelog_body(vm));
    page.into()
}

fn controls(vm: &ChangelogVm) -> Element<'_, UiAction> {
    container(
        row![
            column![
                text("Version").size(9).style(theme::faint_text),
                pick_list(
                    vm.versions.clone(),
                    vm.selected_target.clone(),
                    UiAction::ChangelogTargetChanged,
                )
                .text_size(ui::font_size(12.0))
                .width(Fill)
                .placeholder("Installed CLI"),
            ]
            .spacing(4)
            .width(Fill),
            column![
                text("Provider").size(9).style(theme::faint_text),
                text("unity-changelog").size(10).style(theme::muted_text),
            ]
            .spacing(4),
            space().width(Fill),
            button(
                text(if vm.loaded {
                    "Refresh notes"
                } else {
                    "Show changelog"
                })
                .size(10)
            )
            .on_press(UiAction::LoadChangelog)
            .padding([6.0, 10.0])
            .style(button::primary),
        ]
        .spacing(16)
        .align_y(iced::Alignment::End),
    )
    .padding(14)
    .width(Fill)
    .style(theme::card)
    .into()
}

fn changelog_body(vm: &ChangelogVm) -> Element<'_, UiAction> {
    if !vm.loaded {
        return empty(
            "Changelog not loaded yet",
            "Press “Show changelog” to read the installed CLI's release notes and flag entries that may break workflows.",
        );
    }

    if let Some(error) = vm.state.error() {
        return text(error.text()).size(10).style(theme::danger_text).into();
    }

    let Some(data) = vm.state.data() else {
        if vm.state.is_loading() {
            return text("Reading release notes…")
                .size(11)
                .style(theme::muted_text)
                .into();
        }
        return empty("No changelog data", "The CLI returned no release notes.");
    };

    let mut body = column![].spacing(14);

    body = body.push(
        row![
            column![
                text(format!("Unity CLI {}", data.version)).size(20),
                text(format!(
                    "{} · {} entries parsed",
                    data.source_detail, data.entry_count
                ))
                .size(10)
                .style(theme::faint_text),
            ]
            .spacing(2)
            .width(Fill),
            badge(data.source.clone(), Tone::Neutral),
        ]
        .align_y(iced::Alignment::Center),
    );

    body = body.push(breaking_changes(data));

    for section in &data.sections {
        let tone = section_tone(section.kind);
        let mut entries = column(
            section
                .entries
                .iter()
                .take(MAX_SECTION_ENTRIES)
                .map(|entry| entry_row(entry, tone))
                .collect::<Vec<Element<'_, UiAction>>>(),
        )
        .spacing(9);

        if section.entries.len() > MAX_SECTION_ENTRIES {
            entries = entries.push(
                text(format!(
                    "… {} more entries. Use the raw Markdown view for the full list.",
                    section.entries.len() - MAX_SECTION_ENTRIES
                ))
                .size(9)
                .style(theme::faint_text),
            );
        }

        body = body.push(
            container(
                column![
                    row![
                        text(section.title.as_str()).size(13),
                        badge(format!("{}", section.entries.len()), tone),
                    ]
                    .spacing(7)
                    .align_y(iced::Alignment::Center),
                    entries,
                ]
                .spacing(10)
                .width(Fill),
            )
            .padding(16)
            .width(Fill)
            .style(theme::card),
        );
    }

    body = body.push(
        row![
            button(text(if vm.show_raw {
                "Hide raw Markdown"
            } else {
                "Show raw Markdown"
            })
            .size(10))
            .on_press(UiAction::ToggleRawChangelog)
            .padding([5.0, 9.0])
            .style(button::text),
            space().width(Fill),
            text("Provider: unity-changelog · a remote JSON API can replace it behind the same shape.")
                .size(9)
                .style(theme::faint_text),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    );

    if vm.show_raw {
        let raw: Element<'_, UiAction> = if let Some(error) = vm.raw.error() {
            text(error.text()).size(10).style(theme::danger_text).into()
        } else if let Some(markdown) = vm.raw.data() {
            scrollable(
                text(markdown.as_str())
                    .size(9)
                    .font(iced::Font::MONOSPACE)
                    .style(theme::muted_text),
            )
            .height(320)
            .into()
        } else if vm.raw.is_loading() {
            text("Loading raw Markdown…")
                .size(10)
                .style(theme::muted_text)
                .into()
        } else {
            text("Raw Markdown is not loaded.")
                .size(10)
                .style(theme::faint_text)
                .into()
        };

        body = body.push(container(raw).padding(10).width(Fill).style(theme::card));
    }

    body.into()
}

fn breaking_changes(data: &ChangelogDataVm) -> Element<'_, UiAction> {
    if data.breaking_changes.is_empty() {
        return container(
            column![
                text("No likely breaking changes detected.").size(12),
                text("This is a text heuristic, not a guarantee. Read the Changed and Removed sections before updating.")
                    .size(10)
                    .style(theme::muted_text),
            ]
            .spacing(3),
        )
        .padding(14)
        .width(Fill)
        .style(theme::notice_card(Tone::Success))
        .into();
    }

    let items = column(
        data.breaking_changes
            .iter()
            .enumerate()
            .map(|(index, change)| {
                container(
                    column![
                        row![
                            text(format!("{:02}", index + 1))
                                .size(10)
                                .style(theme::faint_text)
                                .width(24),
                            text(change.text.as_str())
                                .size(12)
                                .line_height(1.45)
                                .width(Fill),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Start),
                        row![
                            space().width(30),
                            badge(change.section.clone(), Tone::Danger),
                            badge(change.reason.clone(), Tone::Warning),
                        ]
                        .spacing(5),
                    ]
                    .spacing(7),
                )
                .padding(10)
                .width(Fill)
                .style(theme::row_card)
                .into()
            })
            .collect::<Vec<Element<'_, UiAction>>>(),
    )
    .spacing(8);

    card(
        "Possible breaking changes",
        scrollable(items).height(300).into(),
    )
}

fn entry_row<'a>(entry: &'a str, tone: Tone) -> Element<'a, UiAction> {
    row![
        text("•")
            .size(14)
            .line_height(1.2)
            .style(theme::tone_text(tone)),
        text(entry)
            .size(12)
            .line_height(1.45)
            .style(theme::muted_text)
            .width(Fill),
    ]
    .spacing(9)
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
