//! Bundled fonts and the system-font family selected in Settings.
//!
//! The files under `assets/fonts/` are Latin/icon subsets produced by
//! `scripts/subset-fonts.sh`. Load [`ALL`] into the iced application. Every
//! regular UI label goes through [`regular`], so one family chosen by the user
//! covers the whole app; the weights below only vary the weight of that family.
//!
//! iced resolves [`Font::with_name`] against the operating system's font
//! database (cosmic-text loads it at startup), which is what lets the setting
//! offer installed PC fonts such as Tahoma.

use std::cell::Cell;

use iced::font::Weight;
use iced::Font;

/// Shown before the settings file exists and when a saved family is empty.
pub const DEFAULT_FAMILY: &str = "Tahoma";

// `Font::with_name` needs a `'static` name, and the family changes only when
// the user picks another one, so leaking the chosen name is bounded and keeps
// `Font` a `Copy` value.
thread_local! {
    static FAMILY: Cell<Option<&'static str>> = const { Cell::new(None) };
}

/// Selects the family used by every piece of regular UI text this frame.
pub fn set_family(name: &str) {
    let name = name.trim();
    let name = if name.is_empty() {
        DEFAULT_FAMILY
    } else {
        name
    };

    FAMILY.with(|family| {
        let changed = family
            .get()
            .map_or(true, |current| !current.eq_ignore_ascii_case(name));

        if changed {
            family.set(Some(Box::leak(name.to_string().into_boxed_str())));
        }
    });
}

pub fn family() -> &'static str {
    FAMILY
        .with(Cell::get)
        .filter(|name| !name.is_empty())
        .unwrap_or(DEFAULT_FAMILY)
}

/// The app font, used for every regular label.
pub fn regular() -> Font {
    Font::with_name(family())
}

/// Slightly heavier text: buttons, captions, badges.
pub fn medium() -> Font {
    Font {
        weight: Weight::Medium,
        ..regular()
    }
}

/// Titles and other emphasized text.
pub fn semibold() -> Font {
    Font {
        weight: Weight::Semibold,
        ..regular()
    }
}

/// Paths, versions, and commands now share the common family as well.
pub fn mono() -> Font {
    regular()
}

/// The bundled fallback family. iced uses it until the first frame sets the
/// user's family, and it stays registered for systems without the chosen font.
pub const SANS: Font = Font::with_name("Inter");
pub const ICONS: Font = Font::with_name("lucide");

pub const ALL: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/lucide.ttf"),
];
