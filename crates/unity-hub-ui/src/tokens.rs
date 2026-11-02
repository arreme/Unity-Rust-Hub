//! Single source of visual primitives.
//!
//! When the Figma design lands, map its variables onto these names. Component
//! code should read colors, spacing, radii, and type sizes from this file (or
//! from `theme` / `components`) rather than inventing new literals.

use iced::Color;

pub struct Palette {
    pub background: Color,
    pub panel: Color,
    pub panel_raised: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub highlight: Color,
}

pub struct Tokens {
    pub palette: Palette,
    pub sidebar_background: Color,
    pub topbar_background: Color,
    pub hero_border: Color,
    pub row_header_background: Color,
    pub job_background: Color,
    pub job_border: Color,
}

pub const TOKENS: Tokens = Tokens {
    palette: Palette {
        background: Color::from_rgb8(0x0b, 0x0e, 0x13),
        panel: Color::from_rgb8(0x14, 0x19, 0x22),
        panel_raised: Color::from_rgb8(0x1a, 0x20, 0x2b),
        border: Color::from_rgb8(0x24, 0x2c, 0x39),
        border_strong: Color::from_rgb8(0x31, 0x3c, 0x4c),
        text: Color::from_rgb8(0xe8, 0xed, 0xf4),
        muted: Color::from_rgb8(0x8e, 0x9a, 0xac),
        faint: Color::from_rgb8(0x66, 0x72, 0x86),
        accent: Color::from_rgb8(0xf0, 0xa3, 0x5e),
        success: Color::from_rgb8(0x5f, 0xd3, 0x9a),
        warning: Color::from_rgb8(0xf4, 0xc5, 0x66),
        danger: Color::from_rgb8(0xff, 0x7a, 0x85),
        info: Color::from_rgb8(0x73, 0xb7, 0xff),
        highlight: Color::from_rgb8(0xb7, 0x9c, 0xff),
    },
    sidebar_background: Color::from_rgba8(0x10, 0x14, 0x1b, 0.97),
    topbar_background: Color::from_rgba8(0x0b, 0x0e, 0x13, 0.92),
    hero_border: Color::from_rgba8(0xf0, 0xa3, 0x5e, 0.24),
    row_header_background: Color::from_rgba8(0x20, 0x28, 0x36, 0.7),
    job_background: Color::from_rgba8(0x15, 0x1d, 0x29, 0.95),
    job_border: Color::from_rgba8(0x73, 0xb7, 0xff, 0.28),
};

pub mod space {
    pub const PAGE: f32 = 16.0;
    pub const CONTENT: f32 = 24.0;
    pub const CARD: f32 = 18.0;
    pub const CARD_GAP: f32 = 10.0;
    pub const ROW: f32 = 8.0;
    pub const SMALL: f32 = 4.0;
}

pub mod radius {
    pub const CARD: f32 = 14.0;
    pub const HERO: f32 = 18.0;
    pub const MODAL: f32 = 16.0;
    pub const ROW: f32 = 8.0;
    pub const HEADER_ROW: f32 = 7.0;
    pub const NOTICE: f32 = 10.0;
    pub const JOB: f32 = 12.0;
    pub const BADGE: f32 = 999.0;
    pub const NAV: f32 = 8.0;
}

pub mod font {
    pub const EYEBROW: f32 = 9.0;
    pub const PAGE_TITLE: f32 = 24.0;
    pub const SUBTITLE: f32 = 11.0;
    pub const CARD_TITLE: f32 = 14.0;
    pub const SECTION_TITLE: f32 = 13.0;
    pub const BODY: f32 = 11.0;
    pub const SMALL: f32 = 9.0;
    pub const BADGE: f32 = 10.0;
    pub const MONO: f32 = 9.0;
}
