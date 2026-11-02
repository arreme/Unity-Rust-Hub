use iced::widget::text::IntoFragment;
use iced::widget::{column, container, row};
use iced::{Element, Fill, Length};

use unity_hub_model::{Tone, UiAction};

use crate::theme;
use crate::tokens::{font, space};
use crate::ui::text;

pub fn page_header<'a>(
    eyebrow: &'a str,
    title: &'a str,
    subtitle: &'a str,
) -> Element<'a, UiAction> {
    column![
        text(eyebrow).size(font::EYEBROW).style(theme::accent_text),
        text(title).size(font::PAGE_TITLE),
        text(subtitle).size(font::SUBTITLE).style(theme::muted_text),
    ]
    .spacing(space::SMALL)
    .width(Fill)
    .into()
}

pub fn card<'a>(title: &'a str, body: Element<'a, UiAction>) -> Element<'a, UiAction> {
    container(
        column![text(title).size(font::CARD_TITLE), body]
            .spacing(space::CARD_GAP)
            .width(Fill),
    )
    .padding(space::CARD)
    .width(Fill)
    .style(theme::card)
    .into()
}

pub fn cell<'a>(content: impl Into<Element<'a, UiAction>>, portion: u16) -> Element<'a, UiAction> {
    container(content.into())
        .width(Length::FillPortion(portion))
        .into()
}

pub fn header_cell<'a>(label: &'a str, portion: u16) -> Element<'a, UiAction> {
    cell(
        text(label).size(font::SMALL).style(theme::faint_text),
        portion,
    )
}

pub fn row_header<'a>(cells: Vec<Element<'a, UiAction>>) -> Element<'a, UiAction> {
    container(
        row(cells)
            .spacing(space::ROW)
            .align_y(iced::Alignment::Center),
    )
    .padding(6)
    .width(Fill)
    .style(theme::row_header)
    .into()
}

pub fn row_card<'a>(cells: Vec<Element<'a, UiAction>>) -> Element<'a, UiAction> {
    container(
        row(cells)
            .spacing(space::ROW)
            .align_y(iced::Alignment::Center),
    )
    .padding(7)
    .width(Fill)
    .style(theme::row_card)
    .into()
}

pub fn empty<'a>(title: &'a str, description: &'a str) -> Element<'a, UiAction> {
    container(
        column![
            text(title).size(font::SECTION_TITLE),
            text(description).size(font::BADGE).style(theme::muted_text),
        ]
        .spacing(space::SMALL)
        .align_x(iced::Alignment::Center),
    )
    .padding(28)
    .width(Fill)
    .center_x(Fill)
    .into()
}

pub fn detail_column<'a>(label: &'a str, value: impl IntoFragment<'a>) -> Element<'a, UiAction> {
    column![
        text(label).size(font::SMALL).style(theme::faint_text),
        text(value).size(font::BODY).style(theme::muted_text),
    ]
    .spacing(2)
    .width(Fill)
    .into()
}

pub fn badge(label: impl Into<String>, tone: Tone) -> Element<'static, UiAction> {
    badge_with_label(label.into(), tone)
}

pub fn badge_with_label(label: String, tone: Tone) -> Element<'static, UiAction> {
    container(text(label).size(font::BADGE))
        .padding([3.0, 8.0])
        .style(theme::badge(tone))
        .into()
}
