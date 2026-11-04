//! Visual layer for Unity Rust Hub.
//!
//! The crate depends only on `unity-hub-model` and iced. It has no access to
//! `unity-core`, the controller, jobs, or CLI errors. Screens consume view
//! models and emit [`unity_hub_model::UiAction`] values.

mod components;
pub mod fonts;
pub mod icons;
mod modals;
mod pages;
mod shell;
pub mod theme;
pub mod tokens;
pub mod ui;

pub mod fixtures;

use iced::{Element, Size};

use unity_hub_model::{UiAction, ViewModel};

pub const WINDOW_SIZE: Size = Size::new(1280.0, 840.0);
pub const MIN_WINDOW_SIZE: Size = Size::new(1040.0, 680.0);

/// Entry point for every visual. A modal replaces the shell; the iced tiny-skia
/// backend miscomposites stacked layers over the scrollable shell.
pub fn view(vm: &ViewModel) -> Element<'_, UiAction> {
    ui::set_scale(vm.text_scale);

    match &vm.modal {
        Some(modal) => modals::view(modal),
        None => shell::view(vm),
    }
}

pub fn theme() -> iced::Theme {
    theme::app_theme()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixture_renders() {
        for fixture in fixtures::all() {
            let _ = view(&fixture.view_model);
        }
    }
}
