use std::sync::OnceLock;

use regex::Regex;

use crate::errors::UnityError;

fn exact_editor_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{4}\.\d+\.\d+[abfp]\d+$").expect("valid regex"))
}

fn stream_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d+(\.\d+)?$").expect("valid regex"))
}

fn cli_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$").expect("valid regex"))
}

/// Accepts an exact Editor version, a stream such as `6000` / `6000.3`, or one
/// of the CLI aliases (`latest`, `lts`, `default`).
pub fn validate_editor_version(input: &str) -> Result<(), UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Err(UnityError::invalid_input("Enter a Unity Editor version."));
    }
    if matches!(value, "latest" | "lts" | "default")
        || exact_editor_version().is_match(value)
        || stream_version().is_match(value)
    {
        return Ok(());
    }
    Err(UnityError::invalid_input(format!(
        "`{value}` is not a Unity Editor version, stream, or alias. Try `lts`, `latest`, `6000.3`, or `6000.3.15f1`."
    )))
}

pub fn validate_cli_version(input: &str) -> Result<(), UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Err(UnityError::invalid_input("Enter a Unity CLI version."));
    }
    if value == "latest" || cli_version().is_match(value) {
        return Ok(());
    }
    Err(UnityError::invalid_input(format!(
        "`{value}` is not a Unity CLI version. Try `latest` or `1.0.0`."
    )))
}

fn module_id() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._+@-]*$").expect("valid regex"))
}

pub fn validate_module_id(input: &str) -> Result<(), UnityError> {
    let value = input.trim();
    if value.is_empty() || !module_id().is_match(value) {
        return Err(UnityError::invalid_input(format!(
            "`{value}` is not a valid Unity module id."
        )));
    }
    Ok(())
}

/// Validates the release stream filter accepted by `unity releases --stream`.
pub fn validate_release_stream(input: &str) -> Result<(), UnityError> {
    match input {
        "alpha" | "beta" | "lts" | "tech" => Ok(()),
        other => Err(UnityError::invalid_input(format!(
            "`{other}` is not a release stream. Use alpha, beta, lts, or tech."
        ))),
    }
}

/// Validates a project path that must exist and be a directory.
pub fn validate_project_path(path: &str) -> Result<std::path::PathBuf, UnityError> {
    let value = path.trim();
    if value.is_empty() {
        return Err(UnityError::invalid_input("Select a Unity project."));
    }
    let path = std::path::PathBuf::from(value);
    if !path.exists() {
        return Err(UnityError::invalid_input(format!(
            "The project path does not exist: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(UnityError::invalid_input(format!(
            "The project path is not a directory: {}",
            path.display()
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exact_stream_and_alias_versions() {
        for value in [
            "6000.3.15f1",
            "2022.3.40f1",
            "6000.7.0b7",
            "6000.0.0a12",
            "6000",
            "6000.3",
            "lts",
            "latest",
            "default",
        ] {
            assert!(
                validate_editor_version(value).is_ok(),
                "{value} should be accepted"
            );
        }
    }

    #[test]
    fn rejects_shell_metacharacters() {
        for value in ["6000.3.15f1; rm -rf /", "`whoami`", "$(id)", "latest extra"] {
            assert!(
                validate_editor_version(value).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn validates_cli_versions() {
        assert!(validate_cli_version("1.0.0-beta.14").is_ok());
        assert!(validate_cli_version("latest").is_ok());
        assert!(validate_cli_version("1.0").is_err());
    }

    #[test]
    fn validates_module_ids() {
        for value in ["android", "linux-il2cpp", "android-open-jdk-17.0.18+8"] {
            assert!(validate_module_id(value).is_ok(), "{value} should be valid");
        }
        for value in ["--accept-eula", "android;id", "$(id)", ""] {
            assert!(validate_module_id(value).is_err(), "{value} should fail");
        }
    }
}
