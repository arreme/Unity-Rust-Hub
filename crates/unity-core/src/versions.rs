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

fn project_name() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"^[^<>:"/\\|?*\x00-\x1f]+$"#).expect("valid regex"))
}

/// A new project's folder name. It must be a single path component, so the
/// CLI cannot be talked into writing outside the chosen parent directory.
pub fn validate_project_name(input: &str) -> Result<(), UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Err(UnityError::invalid_input("Enter a project name."));
    }
    if value.chars().count() > 128 {
        return Err(UnityError::invalid_input(
            "Project names must be 128 characters or fewer.",
        ));
    }
    if value == "."
        || value == ".."
        || value.starts_with('.')
        || value.ends_with('.')
        || value.starts_with('-')
    {
        return Err(UnityError::invalid_input(
            "Project names cannot start with a dot or a hyphen, and cannot end with a dot.",
        ));
    }
    if value.starts_with(' ') || value.ends_with(' ') || !project_name().is_match(value) {
        return Err(UnityError::invalid_input(format!(
            "`{value}` is not a valid project folder name. Avoid path separators and the characters < > : \" | ? *."
        )));
    }
    Ok(())
}

fn package_id() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[a-z][a-z0-9-]*(\.[a-z0-9][a-z0-9._-]*){2,}$").expect("valid regex")
    })
}

/// An npm-style reverse-DNS package id such as `com.myorg.template.game`.
pub fn validate_package_id(input: &str) -> Result<(), UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Err(UnityError::invalid_input("Enter a template package id."));
    }
    if value.chars().count() > 214 || !package_id().is_match(value) {
        return Err(UnityError::invalid_input(format!(
            "`{value}` is not a valid package id. Use a lowercase reverse-DNS id such as com.myorg.template.game."
        )));
    }
    Ok(())
}

/// A template id: a package id, or the CLI's `2d` / `3d` aliases.
pub fn validate_template_id(input: &str) -> Result<(), UnityError> {
    let value = input.trim();
    if matches!(value, "2d" | "3d") {
        return Ok(());
    }
    validate_package_id(value)
}

fn template_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$").expect("valid regex")
    })
}

/// An optional template version. An empty value is valid and means "CLI default".
pub fn validate_template_version(input: &str) -> Result<(), UnityError> {
    let value = input.trim();
    if value.is_empty() || template_version().is_match(value) {
        return Ok(());
    }
    Err(UnityError::invalid_input(format!(
        "`{value}` is not a version. Use a value such as 1.0.0."
    )))
}

/// A non-empty output directory path. It does not need to exist yet.
pub fn validate_output_dir(input: &str) -> Result<std::path::PathBuf, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Err(UnityError::invalid_input("Enter an output directory."));
    }
    let path = std::path::PathBuf::from(value);
    if path.exists() && !path.is_dir() {
        return Err(UnityError::invalid_input(format!(
            "The output path is not a directory: {}",
            path.display()
        )));
    }
    Ok(path)
}

/// A `.tgz` archive destination that is not a directory.
pub fn validate_archive_path(input: &str) -> Result<std::path::PathBuf, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Err(UnityError::invalid_input("Enter an archive path."));
    }
    let path = std::path::PathBuf::from(value);
    if path.is_dir() {
        return Err(UnityError::invalid_input(format!(
            "The archive path is a directory: {}",
            path.display()
        )));
    }
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| !extension.eq_ignore_ascii_case("tgz"))
        .unwrap_or(true)
    {
        return Err(UnityError::invalid_input(
            "The archive path must end in .tgz.",
        ));
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

    #[test]
    fn project_names_are_a_single_safe_path_component() {
        for value in ["My Game", "RoomBattleMultiplayer", "game_2"] {
            assert!(
                validate_project_name(value).is_ok(),
                "{value} should be accepted"
            );
        }
        for value in [
            "",
            ".",
            "..",
            ".hidden",
            "trailing.",
            "../escape",
            "a/b",
            "a\\b",
            "bad:name",
            "--open",
        ] {
            assert!(
                validate_project_name(value).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn package_ids_are_lowercase_reverse_dns() {
        for value in [
            "com.myorg.template.game",
            "com.unity.template.urp-blank",
            "com.acme-tools.2d.starter",
        ] {
            assert!(
                validate_package_id(value).is_ok(),
                "{value} should be accepted"
            );
        }
        for value in [
            "",
            "com.myorg",
            "Com.MyOrg.Template",
            "com..template",
            "com.myorg.template;rm",
            ".com.myorg.template",
        ] {
            assert!(
                validate_package_id(value).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn template_ids_accept_package_ids_and_the_2d_3d_aliases() {
        assert!(validate_template_id("2d").is_ok());
        assert!(validate_template_id("3d").is_ok());
        assert!(validate_template_id("com.unity.template.urp-blank").is_ok());
        assert!(validate_template_id("urp-blank").is_err());
    }

    #[test]
    fn template_versions_are_semver_like_and_optional() {
        assert!(validate_template_version("").is_ok());
        assert!(validate_template_version("1.0.0").is_ok());
        assert!(validate_template_version("1.0.0-beta.1").is_ok());
        for value in ["1.0", "v1.0.0", "1.0.0; rm"] {
            assert!(
                validate_template_version(value).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn archive_paths_must_end_in_tgz_and_not_be_directories() {
        assert!(validate_archive_path("C:\\out\\game.tgz").is_ok());
        assert!(validate_archive_path("./game.TGZ").is_ok());
        for value in ["", "./game.zip", "."] {
            assert!(
                validate_archive_path(value).is_err(),
                "{value} should be rejected"
            );
        }
    }
}
