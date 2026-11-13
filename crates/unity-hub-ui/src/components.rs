use iced::widget::text::IntoFragment;
use iced::widget::{
    column, container, row, rule, space, Button, Checkbox, Column, Scrollable, TextInput,
};
use iced::{Center, Element, Fill, Length};

use unity_hub_model::{Tone, UiAction, UiError};

use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::{self, text};

/// Padding for buttons inside list rows and toolbars (about 36 px tall).
pub const COMPACT: [f32; 2] = [8.0, 14.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// White pill. Use one per card or dialog.
    Primary,
    /// Outlined pill.
    Secondary,
    /// Text-only pill.
    Ghost,
    Danger,
}

impl ButtonKind {
    fn style(
        self,
    ) -> fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style {
        match self {
            Self::Primary => theme::button_primary,
            Self::Secondary => theme::button_secondary,
            Self::Ghost => theme::button_ghost,
            Self::Danger => theme::button_danger,
        }
    }
}

pub fn button<'a>(label: impl IntoFragment<'a>, kind: ButtonKind) -> Button<'a, UiAction> {
    iced::widget::button(text(label).size(font::BUTTON).font(fonts::medium()))
        .padding([11.0, 18.0])
        .style(kind.style())
}

pub fn icon_button<'a>(
    glyph: char,
    label: impl IntoFragment<'a>,
    kind: ButtonKind,
) -> Button<'a, UiAction> {
    iced::widget::button(
        row![
            icon(glyph, font::ICON_SMALL),
            text(label).size(font::BUTTON).font(fonts::medium()),
        ]
        .spacing(8)
        .align_y(Center),
    )
    .padding([11.0, 18.0])
    .style(kind.style())
}

/// A square button holding only an icon. Callers add `.on_press` and may
/// override the padding for tighter rows.
pub fn icon_only<'a>(glyph: char, kind: ButtonKind) -> Button<'a, UiAction> {
    iced::widget::button(icon(glyph, font::ICON_SMALL))
        .padding([8.0, 9.0])
        .style(kind.style())
}

/// A row of pills where the selected option is filled.
pub fn segmented<'a>(options: Vec<(String, bool, UiAction)>) -> Element<'a, UiAction> {
    container(
        row(options
            .into_iter()
            .map(|(label, selected, action)| {
                iced::widget::button(text(label).size(font::SMALL).font(fonts::medium()))
                    .padding([7.0, 14.0])
                    .style(theme::segment(selected))
                    .on_press(action)
                    .into()
            })
            .collect::<Vec<_>>())
        .spacing(2),
    )
    .padding(4)
    .style(theme::segmented)
    .into()
}

pub fn text_input<'a>(placeholder: &str, value: &str) -> TextInput<'a, UiAction> {
    iced::widget::text_input(placeholder, value)
        .padding([11.0, 14.0])
        .size(ui::font_size(font::BODY_LARGE))
        .font(fonts::regular())
        .style(theme::text_input)
}

pub fn search_input<'a>(placeholder: &str, value: &str) -> TextInput<'a, UiAction> {
    iced::widget::text_input(placeholder, value)
        .padding([12.0, 18.0])
        .size(ui::font_size(font::BODY_LARGE))
        .font(fonts::regular())
        .icon(icons::input_icon(icons::SEARCH, font::ICON_SMALL))
        .style(theme::search_input)
}

pub fn checkbox<'a>(
    label: impl Into<String>,
    checked: bool,
    on_toggle: impl Fn(bool) -> UiAction + 'a,
) -> Checkbox<'a, UiAction> {
    iced::widget::checkbox(checked)
        .label(label.into())
        .on_toggle(on_toggle)
        .size(16)
        .spacing(12)
        .text_size(ui::font_size(font::BODY))
        .font(fonts::regular())
        .style(theme::checkbox)
}

pub fn scroll<'a>(content: impl Into<Element<'a, UiAction>>) -> Scrollable<'a, UiAction> {
    iced::widget::scrollable(content)
        .direction(iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(2),
        ))
        .style(theme::scrollable)
}

/// A horizontally scrolling viewport for tables that cannot fit their columns.
pub fn scroll_horizontal<'a>(
    content: impl Into<Element<'a, UiAction>>,
) -> Scrollable<'a, UiAction> {
    iced::widget::scrollable(content)
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(2),
        ))
        .width(Fill)
        .style(theme::scrollable)
}

/// Large page title with a subtitle and optional actions on the right.
pub fn page_header<'a>(
    title: impl IntoFragment<'a>,
    subtitle: impl IntoFragment<'a>,
    actions: Option<Element<'a, UiAction>>,
) -> Element<'a, UiAction> {
    let heading = column![
        text(title).size(font::PAGE_TITLE).font(fonts::semibold()),
        text(subtitle)
            .size(font::BODY_LARGE)
            .style(theme::muted_text),
    ]
    .spacing(6)
    .width(Fill);

    match actions {
        Some(actions) => row![heading, actions]
            .spacing(16)
            .align_y(iced::Alignment::End)
            .into(),
        None => heading.into(),
    }
}

pub fn refresh_button<'a>() -> Element<'a, UiAction> {
    icon_button(icons::REFRESH, "Refresh", ButtonKind::Secondary)
        .on_press(UiAction::Refresh)
        .into()
}

pub fn card_title<'a>(title: impl IntoFragment<'a>) -> Element<'a, UiAction> {
    text(title)
        .size(font::CARD_TITLE)
        .font(fonts::semibold())
        .into()
}

pub fn card<'a>(title: &'a str, body: Element<'a, UiAction>) -> Element<'a, UiAction> {
    surface(
        column![card_title(title), body]
            .spacing(spacing::CARD_GAP)
            .width(Fill),
    )
}

/// A card whose title row carries something on the right (a count, a hint).
pub fn card_with<'a>(
    title: &'a str,
    trailing: Element<'a, UiAction>,
    body: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    surface(
        column![
            row![container(card_title(title)).width(Fill), trailing].align_y(Center),
            body
        ]
        .spacing(spacing::CARD_GAP)
        .width(Fill),
    )
}

/// A card surface without a title.
pub fn surface<'a>(content: impl Into<Element<'a, UiAction>>) -> Element<'a, UiAction> {
    container(content)
        .padding(spacing::CARD)
        .width(Fill)
        .style(theme::card)
        .into()
}

/// Big numbers over uppercase captions, separated by hairlines.
pub fn stat_strip<'a>(stats: Vec<(String, String, Option<Tone>)>) -> Element<'a, UiAction> {
    let mut cells = row![];
    for (index, (value, label, tone)) in stats.into_iter().enumerate() {
        if index > 0 {
            // A Fill-height rule stretches to the tallest cell.
            cells = cells.push(rule::vertical(1).style(theme::divider));
        }
        let value = text(value).size(font::STAT).font(fonts::semibold());
        let value = match tone {
            Some(tone) => value.style(theme::tone_text(tone)),
            None => value,
        };
        cells = cells.push(
            container(column![value, caption(label)].spacing(6))
                .padding([20.0, 22.0])
                .width(Fill),
        );
    }

    container(cells).width(Fill).style(theme::card).into()
}

pub fn divider<'a>() -> Element<'a, UiAction> {
    rule::horizontal(1).style(theme::divider).into()
}

/// Stacks `items` with hairlines between them.
pub fn divided<'a>(items: Vec<Element<'a, UiAction>>) -> Column<'a, UiAction> {
    let mut list = column![];
    for (index, item) in items.into_iter().enumerate() {
        if index > 0 {
            list = list.push(divider());
        }
        list = list.push(item);
    }
    list.width(Fill)
}

/// A list row with the standard vertical rhythm.
pub fn list_row<'a>(content: impl Into<Element<'a, UiAction>>) -> Element<'a, UiAction> {
    container(content)
        .padding([spacing::ROW, 0.0])
        .width(Fill)
        .into()
}

pub fn cell<'a>(
    content: impl Into<Element<'a, UiAction>>,
    width: impl Into<Length>,
) -> Element<'a, UiAction> {
    container(content.into())
        .width(width)
        .align_y(iced::Alignment::Center)
        .clip(true)
        .into()
}

/// A table cell whose content is centered in both axes (module tags).
pub fn cell_centered<'a>(
    content: impl Into<Element<'a, UiAction>>,
    width: impl Into<Length>,
) -> Element<'a, UiAction> {
    container(content.into())
        .width(width)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .clip(true)
        .into()
}

/// A titled group of settings-style rows inside one card.
pub fn group<'a>(title: &'a str, rows: Vec<Element<'a, UiAction>>) -> Element<'a, UiAction> {
    column![
        container(caption(title)).padding([0.0, 4.0]),
        container(divided(rows))
            .padding([0.0, 20.0])
            .width(Fill)
            .style(theme::card),
    ]
    .spacing(10)
    .width(Fill)
    .into()
}

/// A settings row: a label (and optional hint) on the left, a value or
/// control on the right.
pub fn setting_row<'a>(
    label: &'a str,
    hint_text: Option<&'a str>,
    trailing: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    let mut left = column![text(label).size(font::BODY_LARGE)].spacing(2);
    if let Some(hint_text) = hint_text {
        left = left.push(hint(hint_text));
    }

    list_row(row![left.width(Fill), trailing].spacing(16).align_y(Center))
}

pub fn empty<'a>(title: &'a str, description: &'a str) -> Element<'a, UiAction> {
    container(
        column![
            icon_tile(icons::PACKAGE, Tone::Neutral, 44.0),
            text(title).size(font::BODY_LARGE).font(fonts::medium()),
            text(description).size(font::SMALL).style(theme::muted_text),
        ]
        .spacing(spacing::SMALL + 2.0)
        .align_x(Center),
    )
    .padding(32)
    .width(Fill)
    .center_x(Fill)
    .into()
}

/// A grey rounded square holding an icon.
pub fn icon_tile<'a>(glyph: char, tone: Tone, size: f32) -> Element<'a, UiAction> {
    container(icon(glyph, font::ICON_SMALL))
        .width(size)
        .height(size)
        .center(size)
        .style(theme::icon_tile(tone))
        .into()
}

/// A grey rounded square holding a letter.
pub fn initial_tile<'a>(title: &str, size: f32) -> Element<'a, UiAction> {
    let initial = title
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string());

    container(
        text(initial)
            .size(font::BODY_LARGE + 1.0)
            .font(fonts::semibold()),
    )
    .width(size)
    .height(size)
    .center(size)
    .style(theme::icon_tile(Tone::Neutral))
    .into()
}

/// Uppercase caption: stat labels, table headers, group titles.
pub fn caption<'a>(label: impl Into<String>) -> Element<'a, UiAction> {
    text(label.into().to_uppercase())
        .size(font::CAPTION)
        .font(fonts::medium())
        .style(theme::faint_text)
        .into()
}

/// Small faint label above a value or a form control.
pub fn field_label<'a>(label: impl IntoFragment<'a>) -> Element<'a, UiAction> {
    text(label)
        .size(font::SMALL)
        .style(theme::faint_text)
        .into()
}

pub fn detail_column<'a>(label: &'a str, value: impl IntoFragment<'a>) -> Element<'a, UiAction> {
    column![
        field_label(label),
        text(value)
            .size(font::BODY_LARGE)
            .style(theme::primary_text),
    ]
    .spacing(3)
    .width(Fill)
    .into()
}

/// A detail whose value is a path, command, or identifier.
pub fn detail_mono<'a>(label: &'a str, value: impl IntoFragment<'a>) -> Element<'a, UiAction> {
    column![field_label(label), mono(value)]
        .spacing(3)
        .width(Fill)
        .into()
}

pub fn mono<'a>(value: impl IntoFragment<'a>) -> Element<'a, UiAction> {
    text(value)
        .size(font::MONO)
        .font(fonts::mono())
        .style(theme::muted_text)
        .into()
}

pub fn badge(label: impl Into<String>, tone: Tone) -> Element<'static, UiAction> {
    container(text(label.into()).size(font::BADGE).font(fonts::medium()))
        .padding([3.0, 10.0])
        .style(theme::badge(tone))
        .into()
}

pub fn icon_badge(glyph: char, label: impl Into<String>, tone: Tone) -> Element<'static, UiAction> {
    container(
        row![
            icon(glyph, 12.0),
            text(label.into()).size(font::BADGE).font(fonts::medium()),
        ]
        .spacing(5)
        .align_y(Center),
    )
    .padding([3.0, 10.0])
    .style(theme::badge(tone))
    .into()
}

pub fn status_dot(tone: Tone) -> Element<'static, UiAction> {
    let size = ui::font_size(8.0);
    container(space())
        .width(size)
        .height(size)
        .style(theme::status_dot(tone))
        .into()
}

/// A dot followed by a short colored label ("active", "Editor not installed").
pub fn status_label(label: impl Into<String>, tone: Tone) -> Element<'static, UiAction> {
    row![
        status_dot(tone),
        text(label.into())
            .size(font::SMALL)
            .style(theme::tone_text(tone)),
    ]
    .spacing(6)
    .align_y(Center)
    .into()
}

/// A tinted callout with a leading icon.
pub fn notice<'a>(
    tone: Tone,
    glyph: char,
    content: impl Into<Element<'a, UiAction>>,
) -> Element<'a, UiAction> {
    container(
        row![
            icon(glyph, font::ICON).style(theme::tone_text(tone)),
            container(content).width(Fill),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Start),
    )
    .padding([14.0, 16.0])
    .width(Fill)
    .style(theme::notice_card(tone))
    .into()
}

/// Inline error message for a failed read.
pub fn error_text(error: &UiError) -> Element<'_, UiAction> {
    row![
        icon(icons::CIRCLE_ALERT, font::ICON_SMALL).style(theme::danger_text),
        text(error.text())
            .size(font::BODY)
            .style(theme::danger_text)
            .width(Fill),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Start)
    .into()
}

/// Inline "work in progress" status line.
pub fn loading<'a>(label: impl IntoFragment<'a>) -> Element<'a, UiAction> {
    row![
        icon(icons::LOADER, font::ICON_SMALL).style(theme::faint_text),
        text(label).size(font::BODY).style(theme::muted_text),
    ]
    .spacing(8)
    .align_y(Center)
    .into()
}

pub fn hint<'a>(label: impl IntoFragment<'a>) -> Element<'a, UiAction> {
    text(label)
        .size(font::SMALL)
        .style(theme::faint_text)
        .into()
}

/// The padding a pick list needs to match the inputs.
pub const PICK_LIST_PADDING: [f32; 2] = [10.0, 14.0];
