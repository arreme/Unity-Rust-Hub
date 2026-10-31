use iced::theme::Palette;
use iced::widget::{button, container, text};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

pub const BG: Color = Color::from_rgb8(0x0b, 0x0e, 0x13);
pub const PANEL: Color = Color::from_rgb8(0x14, 0x19, 0x22);
pub const PANEL_2: Color = Color::from_rgb8(0x1a, 0x20, 0x2b);
pub const BORDER: Color = Color::from_rgb8(0x24, 0x2c, 0x39);
pub const BORDER_STRONG: Color = Color::from_rgb8(0x31, 0x3c, 0x4c);
pub const TEXT: Color = Color::from_rgb8(0xe8, 0xed, 0xf4);
pub const MUTED: Color = Color::from_rgb8(0x8e, 0x9a, 0xac);
pub const FAINT: Color = Color::from_rgb8(0x66, 0x72, 0x86);
pub const ACCENT: Color = Color::from_rgb8(0xf0, 0xa3, 0x5e);
pub const GREEN: Color = Color::from_rgb8(0x5f, 0xd3, 0x9a);
pub const RED: Color = Color::from_rgb8(0xff, 0x7a, 0x85);
pub const AMBER: Color = Color::from_rgb8(0xf4, 0xc5, 0x66);
pub const BLUE: Color = Color::from_rgb8(0x73, 0xb7, 0xff);
pub const VIOLET: Color = Color::from_rgb8(0xb7, 0x9c, 0xff);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Green,
    Amber,
    Red,
    Blue,
    Violet,
}

impl Tone {
    fn color(self) -> Color {
        match self {
            Tone::Neutral => MUTED,
            Tone::Accent => ACCENT,
            Tone::Green => GREEN,
            Tone::Amber => AMBER,
            Tone::Red => RED,
            Tone::Blue => BLUE,
            Tone::Violet => VIOLET,
        }
    }
}

pub fn app_theme() -> Theme {
    Theme::custom(
        "Unity Rust Hub",
        Palette {
            background: BG,
            text: TEXT,
            primary: ACCENT,
            success: GREEN,
            warning: AMBER,
            danger: RED,
        },
    )
}

pub fn sidebar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x10, 0x14, 0x1b, 0.97))),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn topbar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x0b, 0x0e, 0x13, 0.92))),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PANEL)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}

pub fn hero(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PANEL_2)),
        border: Border {
            color: Color::from_rgba8(0xf0, 0xa3, 0x5e, 0.24),
            width: 1.0,
            radius: 18.0.into(),
        },
        ..Default::default()
    }
}

pub fn modal(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PANEL_2)),
        border: Border {
            color: BORDER_STRONG,
            width: 1.0,
            radius: 16.0.into(),
        },
        shadow: Shadow {
            color: Color {
                a: 0.55,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 14.0),
            blur_radius: 40.0,
        },
        ..Default::default()
    }
}

pub fn backdrop(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color {
            a: 0.72,
            ..Color::BLACK
        })),
        ..Default::default()
    }
}

pub fn row_header(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x20, 0x28, 0x36, 0.7))),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 7.0.into(),
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
            color: BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

pub fn job_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0x15, 0x1d, 0x29, 0.95))),
        border: Border {
            color: Color::from_rgba8(0x73, 0xb7, 0xff, 0.28),
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

pub fn notice_card(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        let color = tone.color();
        container::Style {
            background: Some(Background::Color(Color { a: 0.07, ..color })),
            border: Border {
                color: Color { a: 0.3, ..color },
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        }
    }
}

pub fn badge(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        let color = tone.color();
        container::Style {
            background: Some(Background::Color(Color { a: 0.1, ..color })),
            border: Border {
                color: Color { a: 0.28, ..color },
                width: 1.0,
                radius: 999.0.into(),
            },
            text_color: Some(color),
            ..Default::default()
        }
    }
}

pub fn nav_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (background, text_color) = match status {
            button::Status::Hovered => (
                Some(Background::Color(Color {
                    a: 0.06,
                    ..Color::WHITE
                })),
                TEXT,
            ),
            button::Status::Pressed => (
                Some(Background::Color(Color {
                    a: 0.1,
                    ..Color::WHITE
                })),
                TEXT,
            ),
            button::Status::Disabled => (None, FAINT),
            button::Status::Active if active => {
                (Some(Background::Color(Color { a: 0.15, ..ACCENT })), TEXT)
            }
            button::Status::Active => (None, MUTED),
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
                radius: 8.0.into(),
            },
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

pub fn muted_text(_theme: &Theme) -> text::Style {
    text::Style { color: Some(MUTED) }
}

pub fn faint_text(_theme: &Theme) -> text::Style {
    text::Style { color: Some(FAINT) }
}

pub fn accent_text(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(ACCENT),
    }
}

pub fn success_text(_theme: &Theme) -> text::Style {
    text::Style { color: Some(GREEN) }
}

pub fn danger_text(_theme: &Theme) -> text::Style {
    text::Style { color: Some(RED) }
}

pub fn warning_text(_theme: &Theme) -> text::Style {
    text::Style { color: Some(AMBER) }
}
