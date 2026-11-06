//! Lucide icon glyphs from the bundled icon font.
//!
//! Only the icons listed in `scripts/subset-fonts.sh` exist in the font. Add a
//! name there and regenerate the fonts before adding a constant here.

use iced::widget::text;

use crate::fonts;
use crate::ui::{self, ScaledText};

pub const DASHBOARD: char = '\u{e1c1}';
pub const BOX: char = '\u{e065}';
pub const FOLDERS: char = '\u{e343}';
pub const NEWSPAPER: char = '\u{e34c}';
pub const SETTINGS: char = '\u{e158}';
pub const REFRESH: char = '\u{e149}';
pub const CHECK: char = '\u{e070}';
pub const CIRCLE_CHECK: char = '\u{e226}';
pub const X: char = '\u{e1b2}';
pub const CIRCLE_X: char = '\u{e088}';
pub const TRIANGLE_ALERT: char = '\u{e193}';
pub const CIRCLE_ALERT: char = '\u{e07b}';
pub const INFO: char = '\u{e0fe}';
pub const TERMINAL: char = '\u{e185}';
pub const DOWNLOAD: char = '\u{e0b6}';
pub const PLAY: char = '\u{e140}';
pub const EXTERNAL_LINK: char = '\u{e0bd}';
pub const FOLDER_OPEN: char = '\u{e247}';
pub const CIRCLE_ARROW_UP: char = '\u{e07f}';
pub const USER: char = '\u{e19f}';
pub const KEY: char = '\u{e4a8}';
pub const SEARCH: char = '\u{e155}';
pub const GIT_BRANCH: char = '\u{e0e5}';
pub const CLOUD: char = '\u{e08c}';
pub const STAR: char = '\u{e17a}';
pub const PUZZLE: char = '\u{e29c}';
pub const CPU: char = '\u{e0ad}';
pub const FILE_TEXT: char = '\u{e0d0}';
pub const CHEVRON_DOWN: char = '\u{e071}';
pub const CHEVRON_RIGHT: char = '\u{e073}';
pub const LOADER: char = '\u{e10d}';
pub const PACKAGE: char = '\u{e12d}';
pub const SQUARE_TERMINAL: char = '\u{e20a}';
pub const MONITOR: char = '\u{e121}';
pub const CALENDAR: char = '\u{e067}';
pub const HARD_DRIVE: char = '\u{e0f0}';
pub const SHIELD_ALERT: char = '\u{e1fe}';
pub const COPY: char = '\u{e0a2}';
pub const LOG_IN: char = '\u{e111}';
pub const BAN: char = '\u{e055}';
pub const ARROW_RIGHT: char = '\u{e04d}';
pub const SPARKLES: char = '\u{e417}';
pub const WRENCH: char = '\u{e1b1}';
pub const TRASH: char = '\u{e18e}';
pub const CIRCLE_DOT: char = '\u{e349}';
pub const PLUS: char = '\u{e141}';
pub const MINUS: char = '\u{e120}';

/// An icon glyph sized like text, so it follows the text-size preference.
pub fn icon<'a>(glyph: char, size: f32) -> ScaledText<'a> {
    ui::text(glyph.to_string())
        .size(size)
        .font(fonts::ICONS)
        .line_height(1.0)
        .shaping(text::Shaping::Basic)
}

/// The same glyph for widgets that take an `Icon` (text input, pick list).
pub fn input_icon(glyph: char, size: f32) -> iced::widget::text_input::Icon<iced::Font> {
    iced::widget::text_input::Icon {
        font: fonts::ICONS,
        code_point: glyph,
        size: Some(ui::font_size(size).into()),
        spacing: 8.0,
        side: iced::widget::text_input::Side::Left,
    }
}

pub fn pick_list_handle(size: f32) -> iced::widget::pick_list::Handle<iced::Font> {
    iced::widget::pick_list::Handle::Static(iced::widget::pick_list::Icon {
        font: fonts::ICONS,
        code_point: CHEVRON_DOWN,
        size: Some(ui::font_size(size).into()),
        line_height: text::LineHeight::Relative(1.0),
        shaping: text::Shaping::Basic,
    })
}
