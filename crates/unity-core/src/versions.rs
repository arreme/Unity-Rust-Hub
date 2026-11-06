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
        "`{value}` is not a Unity CLI version. Try `latest` or `1.0.0-beta.17`."
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

fn architecture_name() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_]{1,31}$").expect("valid regex"))
}

/// Normalizes the Editor architecture the CLI accepts as `--architecture`
/// (`x86_64` or `arm64`).
///
/// Older CLI builds and registry rows can leave it empty, so an empty value
/// means "let the CLI decide" rather than an error. Anything else must look
/// like a plain architecture token so it can never be read as a flag.
pub fn validate_architecture(input: &str) -> Result<Option<String>, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if architecture_name().is_match(value) {
        return Ok(Some(value.to_string()));
    }
    Err(UnityError::invalid_input(format!(
        "`{value}` is not a valid Editor architecture. Use `x86_64` or `arm64`."
    )))
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

/// The CLI update channels the app offers.
///
/// `self-update --check`, `--changelog` and an apply all accept only these.
pub fn validate_update_channel(input: &str) -> Result<Option<String>, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Ok(None);
    }
    match value {
        "stable" | "beta" => Ok(Some(value.to_string())),
        other => Err(UnityError::invalid_input(format!(
            "`{other}` is not a CLI update channel. Use stable or beta."
        ))),
    }
}

/// Validates `--timeout <seconds>`: a whole number greater than zero.
///
/// The CLI rejects `5m` and `1.5`, so the app refuses them before they reach
/// the command line. An empty field means "no timeout".
pub fn validate_timeout_seconds(input: &str) -> Result<Option<u32>, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(UnityError::invalid_input(
            "The install timeout must be a whole number of seconds, such as 600.",
        ));
    }
    let seconds = value.parse::<u32>().map_err(|_| {
        UnityError::invalid_input("The install timeout is too large for a whole number of seconds.")
    })?;
    if seconds == 0 {
        return Err(UnityError::invalid_input(
            "The install timeout must be greater than zero seconds.",
        ));
    }
    Ok(Some(seconds))
}

/// Validates `--retries <n>` and the `UNITY_INSTALL_RETRIES` value.
///
/// `0` disables retries. Any whole number is accepted; the CLI owns its own
/// upper bound and reports `BAD_ARGS` if it has one lower than the value.
pub fn validate_retry_count(input: &str) -> Result<Option<u32>, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(UnityError::invalid_input(
            "Download retries must be a whole number; 0 disables retries.",
        ));
    }
    let retries = value.parse::<u32>().map_err(|_| {
        UnityError::invalid_input("Download retries must be a whole number; 0 disables retries.")
    })?;
    Ok(Some(retries))
}

/// Validates `-d` / `--download-directory`.
///
/// The CLI creates the folder, so it does not need to exist. It must not look
/// like a flag and must be a single path argument; an empty field means "use
/// the normal install location".
pub fn validate_download_directory(input: &str) -> Result<Option<String>, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.starts_with('-') {
        return Err(UnityError::invalid_input(
            "The download directory cannot start with `-`.",
        ));
    }
    if value.chars().any(|character| {
        character == '\0' || character == '\n' || character == '\r' || character.is_control()
    }) {
        return Err(UnityError::invalid_input(
            "The download directory contains an invalid character.",
        ));
    }
    Ok(Some(value.to_string()))
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
    fn validates_architectures() {
        assert_eq!(
            validate_architecture(" x86_64 ").unwrap().as_deref(),
            Some("x86_64")
        );
        assert_eq!(
            validate_architecture("arm64").unwrap().as_deref(),
            Some("arm64")
        );
        assert_eq!(validate_architecture("").unwrap(), None);
        for value in ["--headless", "x86_64; rm -rf /", "$(id)"] {
            assert!(validate_architecture(value).is_err(), "{value} should fail");
        }
    }

    #[test]
    fn validates_update_channels_timeouts_retries_and_download_directories() {
        assert_eq!(
            validate_update_channel("beta").unwrap().as_deref(),
            Some("beta")
        );
        assert_eq!(validate_update_channel("  ").unwrap(), None);
        assert!(validate_update_channel("nightly").is_err());

        assert_eq!(validate_timeout_seconds("600").unwrap(), Some(600));
        assert_eq!(validate_timeout_seconds("").unwrap(), None);
        for value in ["0", "5m", "1.5", "-1", "abc"] {
            assert!(
                validate_timeout_seconds(value).is_err(),
                "{value} must be refused"
            );
        }

        assert_eq!(validate_retry_count("0").unwrap(), Some(0));
        assert_eq!(validate_retry_count("10").unwrap(), Some(10));
        assert_eq!(validate_retry_count("25").unwrap(), Some(25));
        assert_eq!(validate_retry_count(" ").unwrap(), None);
        for value in ["-1", "2.5", "many"] {
            assert!(
                validate_retry_count(value).is_err(),
                "{value} must be refused"
            );
        }

        assert_eq!(
            validate_download_directory(" ./.unity ")
                .unwrap()
                .as_deref(),
            Some("./.unity")
        );
        assert_eq!(validate_download_directory("").unwrap(), None);
        assert!(validate_download_directory("--force").is_err());
        assert!(validate_download_directory("bad\npath").is_err());
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
