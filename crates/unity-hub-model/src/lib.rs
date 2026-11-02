//! The stable contract between the application controller and the visual layer.
//!
//! Nothing in this crate knows about the Unity CLI, iced, or controller state.
//! A UI implementation receives [`ViewModel`] values, draws them, and emits
//! [`UiAction`] values. The controller translates those actions into work and
//! rebuilds the view models.

mod action;
mod view;

pub use action::UiAction;
pub use view::*;
