use iced::Element;

use unity_hub_model::{PageVm, UiAction};

pub mod changelog;
pub mod diagnostics;
pub mod editors;
pub mod overview;
pub mod projects;
pub mod settings;
pub mod skills;

pub fn view(page: &PageVm) -> Element<'_, UiAction> {
    match page {
        PageVm::Overview(vm) => overview::view(vm),
        PageVm::Editors(vm) => editors::view(vm),
        PageVm::Projects(vm) => projects::view(vm),
        PageVm::Skills(vm) => skills::view(vm),
        PageVm::Diagnostics(vm) => diagnostics::view(vm),
        PageVm::Changelog(vm) => changelog::view(vm),
        PageVm::Settings(vm) => settings::view(vm),
    }
}
