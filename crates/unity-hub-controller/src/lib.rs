//! Application controller for Unity Rust Hub.
//!
//! This crate owns state, CLI orchestration, jobs, settings, and the mapping
//! from domain/CLI data into [`unity_hub_model`] view models. It contains no
//! widget or screen code, so the visual layer can be replaced independently.

mod app;
mod jobs;
mod settings;
mod util;

pub use app::{App, Message};
pub use settings::TextSize;
