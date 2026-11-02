use std::sync::OnceLock;

use iced::theme::Palette;
use iced::widget::{button, container, text};
use iced::{Background, Border, Color, Shadow, Theme};

use crate::tokens::{radius, TOKENS};

pub use unity_hub_model::Tone;

fn tone_color(tone: Tone) -> Color {
    let palette = &TOKENS.palette;
    match tone {
        Tone::Neutral => palette.muted,
        Tone::Accent => palette.accent,
        Tone::Success => palette.success,
        Tone::Warning => palette.warning,
        Tone::Danger => palette.danger,
        Tone::Info => palette.info,
        Tone::Highlight => palette.highlight,
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

pub fn sidebar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TOKENS.sidebar_background)),
        border: Border {
            color: TOKENS.palette.border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn topbar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TOKENS.topbar_background)),
        border: Border {
            color: TOKENS.palette.border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TOKENS.palette.panel)),
        border: Border {
            color: TOKENS.palette.border,
            width: 1.0,
            radius: radius::CARD.into(),
        },
        ..Default::default()
    }
}

pub fn hero(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TOKENS.palette.panel_raised)),
        border: Border {
            color: TOKENS.hero_border,
            width: 1.0,
            radius: radius::HERO.into(),
        },
        ..Default::default()
    }
}

pub fn modal(_theme: &Theme) -> container::Style {
    // No drop shadow: tiny-skia's damage tracking repaints it over the card on
    // partial redraws, which progressively darkens the dialog.
    container::Style {
        background: Some(Background::Color(TOKENS.palette.panel_raised)),
        border: Border {
            color: TOKENS.palette.border_strong,
            width: 1.0,
            radius: radius::MODAL.into(),
        },
        ..Default::default()
    }
}

/// Full-window background for dialog screens. Dialogs replace the shell instead
/// of being stacked over it.
pub fn modal_screen(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TOKENS.palette.background)),
        ..Default::default()
    }
}

pub fn row_header(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TOKENS.row_header_background)),
        border: Border {
            color: TOKENS.palette.border,
            width: 1.0,
            radius: radius::HEADER_ROW.into(),
        },
        ..Default::default()
    }
}

pub fn row_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color {
            a: 0.025,
            ..Color::WHITE
        })),
        border: Border {
            color: TOKENS.palette.border,
            width: 1.0,
            radius: radius::ROW.into(),
        },
        ..Default::default()
    }
}

pub fn job_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(TOKENS.job_background)),
        border: Border {
            color: TOKENS.job_border,
            width: 1.0,
            radius: radius::JOB.into(),
        },
        ..Default::default()
    }
}

pub fn notice_card(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        let color = tone_color(tone);
        container::Style {
            background: Some(Background::Color(Color { a: 0.07, ..color })),
            border: Border {
                color: Color { a: 0.3, ..color },
                width: 1.0,
                radius: radius::NOTICE.into(),
            },
            ..Default::default()
        }
    }
}

pub fn badge(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        let color = tone_color(tone);
        container::Style {
            background: Some(Background::Color(Color { a: 0.1, ..color })),
            border: Border {
                color: Color { a: 0.28, ..color },
                width: 1.0,
                radius: radius::BADGE.into(),
            },
            text_color: Some(color),
            ..Default::default()
        }
    }
}

pub fn nav_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let palette = &TOKENS.palette;
        let (background, text_color) = match status {
            button::Status::Hovered => (
                Some(Background::Color(Color {
                    a: 0.06,
                    ..Color::WHITE
                })),
                palette.text,
            ),
            button::Status::Pressed => (
                Some(Background::Color(Color {
                    a: 0.1,
                    ..Color::WHITE
                })),
                palette.text,
            ),
            button::Status::Disabled => (None, palette.faint),
            button::Status::Active if active => (
                Some(Background::Color(Color {
                    a: 0.15,
                    ..palette.accent
                })),
                palette.text,
            ),
            button::Status::Active => (None, palette.muted),
        };

        button::Style {
            background,
            text_color,
            border: Border {
                color: Color {
                    a: 0.0,
                    ..Color::BLACK
                },
                width: 0.0,
                radius: radius::NAV.into(),
            },
            shadow: Shadow::default(),
            snap: false,
        }
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

pub fn accent_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TOKENS.palette.accent),
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
