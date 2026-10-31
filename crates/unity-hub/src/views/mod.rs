use iced::widget::{column, container, row, text};
use iced::{Element, Fill, Length};

use crate::app::Message;
use crate::theme;

pub mod changelog;
pub mod editors;
pub mod overview;
pub mod projects;
pub mod settings;

pub fn page_header<'a>(
    eyebrow: &'a str,
    title: &'a str,
    subtitle: &'a str,
) -> Element<'a, Message> {
    column![
        text(eyebrow).size(9).style(theme::accent_text),
        text(title).size(24),
        text(subtitle).size(11).style(theme::muted_text),
    ]
    .spacing(3)
    .width(Fill)
    .into()
}

pub fn card<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(column![text(title).size(14), body].spacing(10).width(Fill))
        .padding(18)
        .width(Fill)
        .style(theme::card)
        .into()
}

pub fn cell<'a>(content: impl Into<Element<'a, Message>>, portion: u16) -> Element<'a, Message> {
    container(content.into())
        .width(Length::FillPortion(portion))
        .into()
}

pub fn header_cell<'a>(label: &'a str, portion: u16) -> Element<'a, Message> {
    cell(text(label).size(9).style(theme::faint_text), portion)
}

pub fn row_header<'a>(cells: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    container(row(cells).spacing(8).align_y(iced::Alignment::Center))
        .padding(6)
        .width(Fill)
        .style(theme::row_header)
        .into()
}

pub fn row_card<'a>(cells: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    container(row(cells).spacing(8).align_y(iced::Alignment::Center))
        .padding(7)
        .width(Fill)
        .style(theme::row_card)
        .into()
}

pub fn empty<'a>(title: &'a str, description: &'a str) -> Element<'a, Message> {
    container(
        column![
            text(title).size(13),
            text(description).size(10).style(theme::muted_text),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center),
    )
    .padding(28)
    .width(Fill)
    .center_x(Fill)
    .into()
}
