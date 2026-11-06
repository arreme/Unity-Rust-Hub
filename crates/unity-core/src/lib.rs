//! Typed adapter for the Unity CLI.
//!
//! This crate knows every flag the GUI passes to `unity`, how the CLI's JSON
//! envelopes are shaped, and how to turn CLI failures into structured errors.
//! It has no UI toolkit dependency, so it can be tested on its own.

pub mod advisory;
pub mod bootstrap;
pub mod changelog;
pub mod docs;
pub mod envelope;
pub mod errors;
pub mod models;
pub mod runner;
pub mod surface;
pub mod versions;

pub use advisory::{AdvisoryOrigin, AdvisorySink, CliAdvisory};
pub use docs::{DocsLookup, DocsPage, DocsScope};
pub use errors::UnityError;
pub use runner::{
    scrub_appimage_env, CommandSpec, DiagnoseTopic, DoctorRead, EditorVerifyRead, Executed,
    InstallOptions, ModuleInstallOptions, SkillInstallOptions, SkillRefreshOptions, UnityCli,
};
