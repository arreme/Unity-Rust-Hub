//! Single source of visual primitives.
//!
//! Component code should read colors, spacing, radii, and type sizes from this
//! file (or from `theme` / `components`) rather than inventing new literals.

use iced::Color;

pub struct Palette {
    pub background: Color,
    pub panel: Color,
    pub panel_raised: Color,
    pub sunken: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub on_accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub highlight: Color,
}

pub struct Tokens {
    pub palette: Palette,
    pub sidebar_background: Color,
    pub backdrop: Color,
    pub hover: Color,
    pub pressed: Color,
}

// Clean graphite: grey surfaces one step apart, hairline borders, and white as
// the only accent. Status colors are muted and used for small marks only.
pub const TOKENS: Tokens = Tokens {
    palette: Palette {
        background: Color::from_rgb8(0x1f, 0x22, 0x25),
        panel: Color::from_rgb8(0x26, 0x29, 0x2d),
        panel_raised: Color::from_rgb8(0x2f, 0x33, 0x37),
        sunken: Color::from_rgb8(0x1f, 0x22, 0x25),
        border: Color::from_rgb8(0x34, 0x38, 0x3d),
        border_strong: Color::from_rgb8(0x43, 0x48, 0x4e),
        text: Color::from_rgb8(0xf1, 0xf2, 0xf3),
        muted: Color::from_rgb8(0xb4, 0xb8, 0xbd),
        faint: Color::from_rgb8(0x8e, 0x93, 0x99),
        accent: Color::from_rgb8(0xf1, 0xf2, 0xf3),
        accent_hover: Color::from_rgb8(0xff, 0xff, 0xff),
        accent_pressed: Color::from_rgb8(0xd6, 0xd8, 0xdb),
        on_accent: Color::from_rgb8(0x1a, 0x1c, 0x1f),
        success: Color::from_rgb8(0x86, 0xcf, 0xa6),
        warning: Color::from_rgb8(0xe6, 0xc2, 0x7c),
        danger: Color::from_rgb8(0xec, 0x8f, 0x97),
        info: Color::from_rgb8(0x94, 0xbc, 0xe8),
        highlight: Color::from_rgb8(0xbc, 0xae, 0xea),
    },
    sidebar_background: Color::from_rgb8(0x1a, 0x1c, 0x1f),
    backdrop: Color::from_rgb8(0x16, 0x18, 0x1a),
    hover: Color::from_rgba8(0xff, 0xff, 0xff, 0.05),
    pressed: Color::from_rgba8(0xff, 0xff, 0xff, 0.09),
};

pub mod spacing {
    /// Horizontal padding around the page content.
    pub const PAGE_X: f32 = 40.0;
    /// Top padding above the page header.
    pub const PAGE_TOP: f32 = 36.0;
    /// Gap between cards and page sections.
    pub const SECTION: f32 = 24.0;
    /// Inner card padding.
    pub const CARD: f32 = 24.0;
    /// Gap between blocks inside a card.
    pub const CARD_GAP: f32 = 18.0;
    /// Vertical padding of a list row.
    pub const ROW: f32 = 16.0;
    pub const SMALL: f32 = 8.0;
    pub const XS: f32 = 4.0;
}

pub mod radius {
    pub const CARD: f32 = 16.0;
    pub const MODAL: f32 = 20.0;
    /// Inputs, pick lists, insets.
    pub const CONTROL: f32 = 12.0;
    /// Icon tiles and raised blocks inside cards.
    pub const TILE: f32 = 12.0;
    pub const NOTICE: f32 = 14.0;
    pub const NAV: f32 = 10.0;
    /// Buttons, chips, segmented controls.
    pub const PILL: f32 = 999.0;
}

pub mod font {
    /// Uppercase labels: stat captions, table headers, group titles.
    pub const CAPTION: f32 = 11.0;
    pub const SMALL: f32 = 12.0;
    pub const BODY: f32 = 13.0;
    pub const BODY_LARGE: f32 = 14.0;
    pub const CARD_TITLE: f32 = 16.0;
    pub const ROW_TITLE: f32 = 18.0;
    pub const MODAL_TITLE: f32 = 20.0;
    pub const STAT: f32 = 26.0;
    pub const PAGE_TITLE: f32 = 28.0;
    pub const DISPLAY: f32 = 36.0;
    pub const BUTTON: f32 = 13.0;
    pub const BADGE: f32 = 12.0;
    pub const MONO: f32 = 12.0;
    pub const ICON: f32 = 17.0;
    pub const ICON_SMALL: f32 = 15.0;
}
