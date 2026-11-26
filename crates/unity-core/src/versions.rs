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
        "`{value}` is not a Unity CLI version. Try `latest` or `1.0.1`."
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

fn docs_topic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9 ._-]*$").expect("valid regex"))
}

/// Validates a `unity docs <topic>` token.
///
/// A topic is either a command path (`install-modules`, `pipeline editors`) or
/// a class-like identifier (`MonoBehaviour.Start`). Whitespace is collapsed;
/// flags, placeholders, and shell metacharacters never reach argv.
pub fn validate_docs_topic(input: &str) -> Result<String, UnityError> {
    let value = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        return Err(UnityError::invalid_input(
            "The changelog entry names no documentation topic.",
        ));
    }
    if value.chars().count() > 200 {
        return Err(UnityError::invalid_input(
            "The documentation topic is too long to look up.",
        ));
    }
    if !docs_topic().is_match(&value) {
        return Err(UnityError::invalid_input(format!(
            "`{value}` is not a documentation topic. Use a command such as `install-modules` or a class such as `MonoBehaviour.Start`."
        )));
    }
    Ok(value)
}

fn docs_package() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*$").expect("valid regex"))
}

pub fn validate_docs_package(input: &str) -> Result<Option<String>, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 214 || !docs_package().is_match(value) {
        return Err(UnityError::invalid_input(format!(
            "`{value}` is not a Unity package name."
        )));
    }
    Ok(Some(value.to_string()))
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

/// The agent-skill clients the CLI documents for `unity skill install`.
///
/// This list is display metadata and test vocabulary only: a client token is
/// validated by shape, so a newer CLI that adds a client still works without an
/// adapter release.
pub const SKILL_CLIENTS: [&str; 11] = [
    "claude-code",
    "claude-desktop",
    "grok",
    "cursor",
    "windsurf",
    "vscode",
    "cline",
    "codex",
    "opencode",
    "antigravity",
    "qwen",
];

fn skill_client() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9][a-z0-9-]{0,31}$").expect("valid regex"))
}

/// Validates the optional client positional of `unity skill install`.
///
/// The token is a lowercase client id such as `claude-code`. It must never look
/// like a flag, so an empty value and anything outside the documented shape are
/// refused before they reach argv.
pub fn validate_skill_client(input: &str) -> Result<String, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Err(UnityError::invalid_input(
            "Choose an AI client, or install to every supported client.",
        ));
    }
    if !skill_client().is_match(value) {
        return Err(UnityError::invalid_input(format!(
            "`{value}` is not a Unity skill client id. Use one of the ids from `unity skill install --list`."
        )));
    }
    Ok(value.to_string())
}

fn skill_name() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$").expect("valid regex"))
}

/// Validates `--skill <name>`. An empty field means "every skill the CLI
/// installs", so it is left out of the command.
pub fn validate_skill_name(input: &str) -> Result<Option<String>, UnityError> {
    let value = input.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if !skill_name().is_match(value) {
        return Err(UnityError::invalid_input(format!(
            "`{value}` is not a Unity skill name. Leave it empty to install every skill."
        )));
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
        // The exact SemVer 2.0.0 grammar: three numeric parts with no leading
        // zeros, then optional dot-separated prerelease and build identifiers.
        // The CLI (1.1.0) rejects `01.0.0` and `1.0`; matching its parser here
        // keeps the failure specific instead of a generic `BAD_ARGS`.
        Regex::new(concat!(
            r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)",
            r"(?:-(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)",
            r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*)?",
            r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
        ))
        .expect("valid regex")
    })
}

/// An optional template version. An empty value is valid and means "CLI default".
pub fn validate_template_version(input: &str) -> Result<(), UnityError> {
    let value = input.trim();
    if value.is_empty() || template_version().is_match(value) {
        return Ok(());
    }
    Err(UnityError::invalid_input(format!(
        "`{value}` is not a semantic version. Use three numeric parts such as 1.0.0, without leading zeros; prerelease and build suffixes are allowed."
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
    fn validates_skill_clients_and_names() {
        for client in SKILL_CLIENTS {
            assert_eq!(validate_skill_client(client).unwrap(), client);
        }
        assert!(validate_skill_client(" future-client ").is_ok());
        for value in ["", "--all", "Claude Code", "claude_code", "$(id)"] {
            assert!(
                validate_skill_client(value).is_err(),
                "{value} must be refused"
            );
        }

        assert_eq!(
            validate_skill_name(" unity-cli ").unwrap().as_deref(),
            Some("unity-cli")
        );
        assert_eq!(validate_skill_name("").unwrap(), None);
        for value in ["--force", "with space", "slash/name", "$(id)"] {
            assert!(
                validate_skill_name(value).is_err(),
                "{value} must be refused"
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
        assert!(validate_template_version("1.0.0-beta.1+build.5").is_ok());
        for value in [
            "1.0",
            "v1.0.0",
            "1.0.0; rm",
            // CLI 1.1.0 rejects leading zeros in numeric components and in
            // numeric prerelease identifiers; the adapter must reject them
            // before the command line, not surface a generic BAD_ARGS.
            "01.0.0",
            "1.01.0",
            "1.0.01",
            "1.0.0-01",
        ] {
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

    #[test]
    fn validates_docs_topics_and_packages() {
        assert_eq!(
            validate_docs_topic("  MonoBehaviour.Start ").unwrap(),
            "MonoBehaviour.Start"
        );
        assert_eq!(
            validate_docs_topic("pipeline   editors").unwrap(),
            "pipeline editors"
        );
        for value in ["", "--cli", "$(id)", "Start; rm -rf /", "<topic>"] {
            assert!(
                validate_docs_topic(value).is_err(),
                "{value} must be refused"
            );
        }

        assert_eq!(
            validate_docs_package("com.unity.textmeshpro")
                .unwrap()
                .as_deref(),
            Some("com.unity.textmeshpro")
        );
        assert_eq!(validate_docs_package(" ").unwrap(), None);
        for value in ["--package", "com.unity;rm", "../escape"] {
            assert!(
                validate_docs_package(value).is_err(),
                "{value} must be refused"
            );
        }
    }
}
