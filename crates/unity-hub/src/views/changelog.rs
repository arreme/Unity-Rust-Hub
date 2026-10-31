use iced::widget::{button, column, container, pick_list, row, scrollable, space, text};
use iced::{Element, Fill};

use crate::app::{badge, App, Message};
use crate::theme;
use crate::util::error_text;
use crate::views::{card, empty, page_header};

pub fn view(app: &App) -> Element<'_, Message> {
    let mut page = column![page_header(
        "BETA WATCH",
        "Changelog & breaking changes",
        "Release notes come from the installed CLI. Breaking-change flags are a heuristic pass, ready for a remote provider.",
    )]
    .spacing(16);

    page = page.push(controls(app));
    page = page.push(changelog_body(app));
    page.into()
}

fn controls(app: &App) -> Element<'_, Message> {
    let versions = app.changelog_versions();

    container(
        row![
            column![
                text("Version").size(9).style(theme::faint_text),
                pick_list(
                    versions,
                    app.changelog_target.clone(),
                    Message::ChangelogTargetChanged,
                )
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
                text(if app.changelog_loaded {
                    "Refresh notes"
                } else {
                    "Show changelog"
                })
                .size(10)
            )
            .on_press(Message::LoadChangelog)
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

fn changelog_body(app: &App) -> Element<'_, Message> {
    if !app.changelog_loaded {
        return empty(
            "Changelog not loaded yet",
            "Press “Show changelog” to read the installed CLI's release notes and flag entries that may break workflows.",
        );
    }

    if app.changelog.loading && app.changelog.data.is_none() {
        return text("Reading release notes…")
            .size(11)
            .style(theme::muted_text)
            .into();
    }

    if let Some(error) = &app.changelog.error {
        return text(error_text(error))
            .size(10)
            .style(theme::danger_text)
            .into();
    }

    let Some(view) = &app.changelog.data else {
        return empty("No changelog data", "The CLI returned no release notes.");
    };

    let mut body = column![].spacing(14);

    body = body.push(
        row![
            column![
                text(format!("Unity CLI {}", view.version)).size(20),
                text(format!(
                    "{} · {} entries parsed",
                    view.source_detail, view.entry_count
                ))
                .size(10)
                .style(theme::faint_text),
            ]
            .spacing(2)
            .width(Fill),
            badge(view.source.clone(), theme::Tone::Neutral),
        ]
        .align_y(iced::Alignment::Center),
    );

    let breaking = if view.breaking_changes.is_empty() {
        container(
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
        .style(theme::notice_card(theme::Tone::Green))
        .into()
    } else {
        let items = column(
            view.breaking_changes
                .iter()
                .map(|change| {
                    column![
                        text(&change.text).size(11),
                        row![
                            badge(change.section.clone(), theme::Tone::Red),
                            badge(change.reason.clone(), theme::Tone::Amber),
                        ]
                        .spacing(5),
                    ]
                    .spacing(4)
                    .width(Fill)
                    .into()
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(10);

        card(
            "Possible breaking changes",
            scrollable(items).height(260).into(),
        )
    };

    body = body.push(breaking);

    for section in &view.sections {
        let tone = section_tone(&section.title);
        let entries = column(
            section
                .entries
                .iter()
                .map(|entry| {
                    text(entry)
                        .size(11)
                        .style(theme::muted_text)
                        .width(Fill)
                        .into()
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(6);

        body = body.push(
            container(
                column![
                    row![
                        text(&section.title).size(13),
                        badge(format!("{}", section.entries.len()), tone),
                    ]
                    .spacing(7)
                    .align_y(iced::Alignment::Center),
                    entries,
                ]
                .spacing(8)
                .width(Fill),
            )
            .padding(16)
            .width(Fill)
            .style(theme::card),
        );
    }

    body = body.push(
        row![
            button(text(if app.show_raw_changelog { "Hide raw Markdown" } else { "Show raw Markdown" }).size(10))
                .on_press(Message::ToggleRawChangelog)
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

    if app.show_raw_changelog {
        body = body.push(
            container(
                scrollable(
                    text(&view.markdown)
                        .size(9)
                        .font(iced::Font::MONOSPACE)
                        .style(theme::muted_text),
                )
                .height(320),
            )
            .padding(10)
            .width(Fill)
            .style(theme::card),
        );
    }

    body.into()
}

fn section_tone(title: &str) -> theme::Tone {
    let title = title.to_lowercase();
    if title.contains("removed") {
        theme::Tone::Red
    } else if title.contains("changed") || title.contains("deprecated") {
        theme::Tone::Amber
    } else if title.contains("added") {
        theme::Tone::Green
    } else if title.contains("security") {
        theme::Tone::Violet
    } else if title.contains("fixed") {
        theme::Tone::Blue
    } else {
        theme::Tone::Neutral
    }
}
