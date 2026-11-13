use std::sync::OnceLock;

use iced::border::Radius;
use iced::theme::Palette;
use iced::widget::{self as widget, button, container, overlay, rule, text};
use iced::{Background, Border, Color, Shadow, Theme};

use crate::tokens::{radius, TOKENS};

pub use unity_hub_model::Tone;

pub fn tone_color(tone: Tone) -> Color {
    let palette = &TOKENS.palette;
    match tone {
        Tone::Neutral => palette.muted,
        Tone::Accent => palette.text,
        Tone::Success => palette.success,
        Tone::Warning => palette.warning,
        Tone::Danger => palette.danger,
        Tone::Info => palette.info,
        Tone::Highlight => palette.highlight,
    }
}

fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

fn border(color: Color, width: f32, radius: impl Into<Radius>) -> Border {
    Border {
        color,
        width,
        radius: radius.into(),
    }
}

fn surface(background: Color, border_color: Color, corner: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: border(
            border_color,
            if border_color.a > 0.0 { 1.0 } else { 0.0 },
            corner,
        ),
        ..Default::default()
    }
}

pub fn app_theme() -> Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME
        .get_or_init(|| {
            let palette = &TOKENS.palette;
            Theme::custom(
                "Unity Rust Hub",
                Palette {
                    background: palette.background,
                    text: palette.text,
                    primary: palette.accent,
                    success: palette.success,
                    warning: palette.warning,
                    danger: palette.danger,
                },
            )
        })
        .clone()
}

// ---------------------------------------------------------------------------
// Surfaces

pub fn page(_theme: &Theme) -> container::Style {
    surface(TOKENS.palette.background, Color::TRANSPARENT, 0.0)
}

pub fn sidebar(_theme: &Theme) -> container::Style {
    surface(TOKENS.sidebar_background, Color::TRANSPARENT, 0.0)
}

/// The 1 px column between the sidebar and the page.
pub fn vertical_line(_theme: &Theme) -> container::Style {
    surface(TOKENS.palette.border, Color::TRANSPARENT, 0.0)
}

pub fn card(_theme: &Theme) -> container::Style {
    surface(TOKENS.palette.panel, TOKENS.palette.border, radius::CARD)
}

/// A raised block inside a card: breaking-change items, version strips.
pub fn raised(_theme: &Theme) -> container::Style {
    surface(
        TOKENS.palette.panel_raised,
        Color::TRANSPARENT,
        radius::TILE,
    )
}

/// A recessed surface inside a card: logs, previews, code.
pub fn inset(_theme: &Theme) -> container::Style {
    surface(
        TOKENS.palette.sunken,
        TOKENS.palette.border,
        radius::CONTROL,
    )
}

/// A hairline-bordered box without fill: module lists, the sidebar status.
pub fn outline(_theme: &Theme) -> container::Style {
    surface(Color::TRANSPARENT, TOKENS.palette.border, radius::CONTROL)
}

/// A pill-shaped outline holding a segmented control.
pub fn segmented(_theme: &Theme) -> container::Style {
    surface(
        Color::TRANSPARENT,
        TOKENS.palette.border_strong,
        radius::PILL,
    )
}

pub fn modal(_theme: &Theme) -> container::Style {
    // No drop shadow: tiny-skia's damage tracking repaints it over the card on
    // partial redraws, which progressively darkens the dialog. The software
    // fallback must look identical to the GPU path.
    surface(TOKENS.palette.panel, TOKENS.palette.border, radius::MODAL)
}

/// Full-window background for dialog screens. Dialogs replace the shell instead
/// of being stacked over it.
pub fn modal_screen(_theme: &Theme) -> container::Style {
    surface(TOKENS.backdrop, Color::TRANSPARENT, 0.0)
}

pub fn notice_card(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        let color = tone_color(tone);
        surface(alpha(color, 0.08), alpha(color, 0.35), radius::NOTICE)
    }
}

pub fn badge(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        let (background, text_color) = match tone {
            Tone::Neutral | Tone::Accent => (TOKENS.palette.panel_raised, TOKENS.palette.muted),
            _ => {
                let color = tone_color(tone);
                (alpha(color, 0.14), color)
            }
        };
        container::Style {
            background: Some(Background::Color(background)),
            border: border(Color::TRANSPARENT, 0.0, radius::PILL),
            text_color: Some(text_color),
            ..Default::default()
        }
    }
}

/// A rounded square behind an icon or initial. The icon takes the tone color;
/// the tile stays grey.
pub fn icon_tile(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(TOKENS.palette.panel_raised)),
        border: border(Color::TRANSPARENT, 0.0, radius::TILE),
        text_color: Some(match tone {
            Tone::Neutral | Tone::Accent => TOKENS.palette.text,
            _ => tone_color(tone),
        }),
        ..Default::default()
    }
}

pub fn avatar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TOKENS.palette.panel_raised)),
        border: border(Color::TRANSPARENT, 0.0, radius::PILL),
        text_color: Some(TOKENS.palette.text),
        ..Default::default()
    }
}

pub fn status_dot(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(tone_color(tone))),
        border: border(Color::TRANSPARENT, 0.0, radius::PILL),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Controls

fn button_style(
    background: Option<Color>,
    text_color: Color,
    border_color: Color,
) -> button::Style {
    button::Style {
        background: background.map(Background::Color),
        text_color,
        border: border(
            border_color,
            if border_color.a > 0.0 { 1.0 } else { 0.0 },
            radius::PILL,
        ),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// White pill with dark text: one per card or dialog.
pub fn button_primary(_theme: &Theme, status: button::Status) -> button::Style {
    let palette = &TOKENS.palette;
    let background = match status {
        button::Status::Active => palette.accent,
        button::Status::Hovered => palette.accent_hover,
        button::Status::Pressed => palette.accent_pressed,
        button::Status::Disabled => alpha(palette.accent, 0.16),
    };
    let text_color = if matches!(status, button::Status::Disabled) {
        palette.faint
    } else {
        palette.on_accent
    };
    button_style(Some(background), text_color, Color::TRANSPARENT)
}

/// Outlined pill.
pub fn button_secondary(_theme: &Theme, status: button::Status) -> button::Style {
    let palette = &TOKENS.palette;
    match status {
        button::Status::Active => button_style(None, palette.text, palette.border_strong),
        button::Status::Hovered => {
            button_style(Some(TOKENS.hover), palette.text, palette.border_strong)
        }
        button::Status::Pressed => {
            button_style(Some(TOKENS.pressed), palette.text, palette.border_strong)
        }
        button::Status::Disabled => button_style(None, palette.faint, palette.border),
    }
}

/// Text-only pill that fills on hover.
pub fn button_ghost(_theme: &Theme, status: button::Status) -> button::Style {
    let palette = &TOKENS.palette;
    match status {
        button::Status::Active => button_style(None, palette.muted, Color::TRANSPARENT),
        button::Status::Hovered => {
            button_style(Some(TOKENS.hover), palette.text, Color::TRANSPARENT)
        }
        button::Status::Pressed => {
            button_style(Some(TOKENS.pressed), palette.text, Color::TRANSPARENT)
        }
        button::Status::Disabled => button_style(None, palette.faint, Color::TRANSPARENT),
    }
}

pub fn button_danger(_theme: &Theme, status: button::Status) -> button::Style {
    let danger = TOKENS.palette.danger;
    match status {
        button::Status::Active => button_style(None, danger, alpha(danger, 0.45)),
        button::Status::Hovered => {
            button_style(Some(alpha(danger, 0.12)), danger, alpha(danger, 0.6))
        }
        button::Status::Pressed => {
            button_style(Some(alpha(danger, 0.2)), danger, alpha(danger, 0.6))
        }
        button::Status::Disabled => button_style(None, alpha(danger, 0.4), alpha(danger, 0.2)),
    }
}

/// Round grey button holding an icon (dialog close).
pub fn button_round(_theme: &Theme, status: button::Status) -> button::Style {
    let palette = &TOKENS.palette;
    let background = match status {
        button::Status::Hovered => palette.border_strong,
        button::Status::Pressed => palette.border,
        _ => palette.panel_raised,
    };
    button_style(Some(background), palette.text, Color::TRANSPARENT)
}

/// One option of a segmented control.
pub fn segment(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        if selected {
            button_primary(theme, button::Status::Active)
        } else {
            button_ghost(theme, status)
        }
    }
}

pub fn nav_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let palette = &TOKENS.palette;
        let (background, text_color) = match status {
            _ if active => (Some(palette.panel_raised), palette.text),
            button::Status::Hovered => (Some(TOKENS.hover), palette.text),
            button::Status::Pressed => (Some(TOKENS.pressed), palette.text),
            button::Status::Disabled => (None, palette.faint),
            button::Status::Active => (None, palette.muted),
        };

        button::Style {
            background: background.map(Background::Color),
            text_color,
            border: border(Color::TRANSPARENT, 0.0, radius::NAV),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

fn input_style(
    status: widget::text_input::Status,
    background: Color,
    rest: Color,
    corner: f32,
) -> widget::text_input::Style {
    let palette = &TOKENS.palette;
    let border_color = match status {
        widget::text_input::Status::Active => rest,
        widget::text_input::Status::Hovered => Color::from_rgb8(0x55, 0x5a, 0x61),
        widget::text_input::Status::Focused { .. } => palette.muted,
        widget::text_input::Status::Disabled => palette.border,
    };

    widget::text_input::Style {
        background: Background::Color(background),
        border: border(border_color, 1.0, corner),
        icon: palette.faint,
        placeholder: palette.faint,
        value: if matches!(status, widget::text_input::Status::Disabled) {
            palette.faint
        } else {
            palette.text
        },
        selection: alpha(palette.text, 0.25),
    }
}

pub fn text_input(_theme: &Theme, status: widget::text_input::Status) -> widget::text_input::Style {
    input_style(
        status,
        TOKENS.palette.sunken,
        TOKENS.palette.border_strong,
        radius::CONTROL,
    )
}

/// Pill-shaped search field on a card surface.
pub fn search_input(
    _theme: &Theme,
    status: widget::text_input::Status,
) -> widget::text_input::Style {
    input_style(
        status,
        TOKENS.palette.panel,
        TOKENS.palette.border,
        radius::PILL,
    )
}

pub fn pick_list(_theme: &Theme, status: widget::pick_list::Status) -> widget::pick_list::Style {
    let palette = &TOKENS.palette;
    let border_color = match status {
        widget::pick_list::Status::Active => palette.border_strong,
        widget::pick_list::Status::Hovered => Color::from_rgb8(0x55, 0x5a, 0x61),
        widget::pick_list::Status::Opened { .. } => palette.muted,
    };

    widget::pick_list::Style {
        text_color: palette.text,
        placeholder_color: palette.faint,
        handle_color: palette.muted,
        background: Background::Color(palette.sunken),
        border: border(border_color, 1.0, radius::CONTROL),
    }
}

pub fn menu(_theme: &Theme) -> overlay::menu::Style {
    let palette = &TOKENS.palette;
    overlay::menu::Style {
        background: Background::Color(palette.panel_raised),
        border: border(palette.border_strong, 1.0, radius::CONTROL),
        text_color: palette.text,
        selected_text_color: palette.on_accent,
        selected_background: Background::Color(palette.accent),
        shadow: Shadow::default(),
    }
}

pub fn checkbox(_theme: &Theme, status: widget::checkbox::Status) -> widget::checkbox::Style {
    let palette = &TOKENS.palette;
    let (is_checked, hovered, disabled) = match status {
        widget::checkbox::Status::Active { is_checked } => (is_checked, false, false),
        widget::checkbox::Status::Hovered { is_checked } => (is_checked, true, false),
        widget::checkbox::Status::Disabled { is_checked } => (is_checked, false, true),
    };

    let (background, border_color) = match (is_checked, hovered) {
        (true, false) => (palette.accent, palette.accent),
        (true, true) => (palette.accent_hover, palette.accent_hover),
        (false, false) => (Color::TRANSPARENT, palette.border_strong),
        (false, true) => (TOKENS.hover, palette.muted),
    };

    widget::checkbox::Style {
        background: Background::Color(if disabled {
            alpha(background, 0.4)
        } else {
            background
        }),
        icon_color: palette.on_accent,
        border: border(border_color, 1.0, 5.0),
        text_color: Some(if disabled {
            palette.faint
        } else {
            palette.text
        }),
    }
}

pub fn scrollable(_theme: &Theme, status: widget::scrollable::Status) -> widget::scrollable::Style {
    let palette = &TOKENS.palette;
    let scroller_color = match status {
        widget::scrollable::Status::Dragged { .. } => alpha(palette.muted, 0.5),
        widget::scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        }
        | widget::scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered: true,
            ..
        } => alpha(palette.muted, 0.38),
        _ => alpha(palette.muted, 0.18),
    };

    let rail = widget::scrollable::Rail {
        background: None,
        border: border(Color::TRANSPARENT, 0.0, radius::PILL),
        scroller: widget::scrollable::Scroller {
            background: Background::Color(scroller_color),
            border: border(Color::TRANSPARENT, 0.0, radius::PILL),
        },
    };

    widget::scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: widget::scrollable::AutoScroll {
            background: Background::Color(palette.panel_raised),
            border: border(palette.border_strong, 1.0, radius::PILL),
            shadow: Shadow::default(),
            icon: palette.text,
        },
    }
}

pub fn progress_bar(_theme: &Theme) -> widget::progress_bar::Style {
    widget::progress_bar::Style {
        background: Background::Color(TOKENS.palette.panel_raised),
        bar: Background::Color(TOKENS.palette.accent),
        border: border(Color::TRANSPARENT, 0.0, radius::PILL),
    }
}

pub fn divider(_theme: &Theme) -> rule::Style {
    rule::Style {
        color: TOKENS.palette.border,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

// ---------------------------------------------------------------------------
// Text

pub fn primary_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TOKENS.palette.text),
    }
}

pub fn muted_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TOKENS.palette.muted),
    }
}

pub fn faint_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TOKENS.palette.faint),
    }
}

pub fn success_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TOKENS.palette.success),
    }
}

pub fn danger_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TOKENS.palette.danger),
    }
}

pub fn warning_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TOKENS.palette.warning),
    }
}

pub fn tone_text(tone: Tone) -> impl Fn(&Theme) -> text::Style {
    move |_theme| text::Style {
        color: Some(tone_color(tone)),
    }
}
