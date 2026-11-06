//! Bundled fonts.
//!
//! The files under `assets/fonts/` are Latin/icon subsets produced by
//! `scripts/subset-fonts.sh`. Load [`ALL`] into the iced application and use
//! [`SANS`] as its default font.

use iced::font::Weight;
use iced::Font;

pub const SANS: Font = Font::with_name("Inter");
// The static Inter cuts register each weight as its own family, and text
// matching needs the face's exact weight as well as its family name.
pub const MEDIUM: Font = Font {
    weight: Weight::Medium,
    ..Font::with_name("Inter Medium")
};
pub const SEMIBOLD: Font = Font {
    weight: Weight::Semibold,
    ..Font::with_name("Inter SemiBold")
};
pub const MONO: Font = Font::with_name("JetBrains Mono");
pub const ICONS: Font = Font::with_name("lucide");

pub const ALL: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/lucide.ttf"),
];
