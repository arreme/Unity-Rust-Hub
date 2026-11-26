use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::envelope::CliRemediation;

/// A structured failure from the Unity CLI or from the adapter itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityError {
    /// Stable machine code. CLI codes (`AUTH_ERROR`, `NOT_FOUND`, ...) are
    /// passed through; adapter failures use `CLI_NOT_FOUND`, `SPAWN_FAILED`,
    /// `UNEXPECTED_OUTPUT`, or `INVALID_INPUT`.
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub retryable: bool,
    /// Extra CLI error fields (`remediation`, `raw`, ...) for forward compatibility.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

impl UnityError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
            exit_code: None,
            retryable: false,
            extra: Map::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_exit_code(mut self, exit_code: Option<i32>) -> Self {
        self.exit_code = exit_code;
        self.retryable = exit_code.map(is_retryable_exit_code).unwrap_or(false);
        self
    }

    /// The fix the CLI suggested for this error, when its envelope carried one.
    ///
    /// `CliError` flattens unknown fields into [`UnityError::extra`], so an
    /// error envelope's `remediation` survives here after being copied over by
    /// [`crate::envelope::CliError::to_unity_error`]. An object keeps the
    /// CLI's own `requiresUserApproval`; a bare command string is treated as
    /// approval-required because the CLI did not say it was safe.
    pub fn remediation(&self) -> Option<CliRemediation> {
        match self.extra.get("remediation")? {
            Value::String(command) => {
                let command = command.trim();
                if command.is_empty() {
                    return None;
                }
                Some(CliRemediation {
                    command: command.to_string(),
                    requires_user_approval: true,
                })
            }
            value @ Value::Object(_) => {
                let remediation: Option<CliRemediation> =
                    serde_json::from_value(value.clone()).ok();
                remediation.filter(|remediation| !remediation.command.trim().is_empty())
            }
            _ => None,
        }
    }

    pub fn cli_not_found() -> Self {
        Self::new(
            "CLI_NOT_FOUND",
            "The Unity CLI (`unity`) was not found on this machine.",
        )
        .with_detail(
            "Install it from the Overview screen, or set an explicit CLI path in Settings.",
        )
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new("INVALID_INPUT", message)
    }

    pub fn unexpected_output(detail: impl Into<String>) -> Self {
        Self::new(
            "UNEXPECTED_OUTPUT",
            "The Unity CLI returned output the app could not parse.",
        )
        .with_detail(detail)
    }
}

impl std::fmt::Display for UnityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for UnityError {}

/// Exit-code semantics from the Unity CLI reference.
///
/// The single table is [`crate::surface::EXIT_CODES`]; these functions only
/// project it, so a new code is one row.
pub fn is_retryable_exit_code(code: i32) -> bool {
    crate::surface::exit_code_spec(code).is_some_and(|spec| spec.retryable)
}

pub fn code_for_exit_code(code: i32) -> &'static str {
    crate::surface::exit_code_spec(code)
        .map(|spec| spec.name)
        .unwrap_or("ERROR")
}

/// The kind of primary next step an error offers.
///
/// This is the adapter's vocabulary, not a UI type: the controller maps it
/// onto a view-model action and decides whether that action can run. Nothing
/// here executes anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemediationKind {
    Retry,
    SignIn,
    OpenSettings,
    PickVersion,
    OpenEditor,
    EnterToken,
    InstallGit,
    ShowCommand,
    Confirm,
    ShowBusy,
    AnswerDialog,
    FixPermissions,
}

impl RemediationKind {
    /// Only reads and idempotent actions may be retried. The controller is
    /// still responsible for proving that a retry target exists.
    pub fn is_retry(self) -> bool {
        matches!(self, Self::Retry)
    }
}

/// One code-specific next step for a failed CLI call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorRemediation {
    pub kind: RemediationKind,
    /// Short imperative button label.
    pub label: &'static str,
    /// The exact command the CLI named, when it named one. The app only shows
    /// or copies this; it never runs a command from an error by itself.
    pub command: Option<String>,
    /// Display guidance appended after the CLI's own `detail`.
    pub hint: Option<String>,
}

impl ErrorRemediation {
    fn new(kind: RemediationKind, label: &'static str, hint: Option<String>) -> Self {
        Self {
            kind,
            label,
            command: None,
            hint,
        }
    }
}

/// Resolves the primary next step for an error.
///
/// Precedence is fixed: the CLI's own `remediation` first, then exit `18` (a
/// dialog always wins over a streamed `EDITOR_TIMEOUT`), then the stable `code`
/// table, then exit-code semantics. Unknown codes stay generic (`None`) rather
/// than guessing.
///
/// `proxy` is the controller's already-redacted description of a configured
/// proxy (for example `UNITY_PROXY=http://proxy:8080`). The adapter never reads
/// the environment itself, so callers and tests stay deterministic.
pub fn remediation_for(error: &UnityError, proxy: Option<&str>) -> Option<ErrorRemediation> {
    cli_remediation(error)
        .or_else(|| {
            // 1.1.0: a dialog-blocked command exits 18 even when the Editor's
            // own envelope still names `EDITOR_TIMEOUT`.
            error
                .exit_code
                .filter(|code| *code == 18)
                .and_then(|code| exit_remediation(code, proxy))
        })
        .or_else(|| code_remediation(&error.code, proxy))
        .or_else(|| {
            error
                .exit_code
                .and_then(|code| exit_remediation(code, proxy))
        })
}

/// The CLI's `remediation` field, which `CliError` flattens into `extra`.
fn cli_remediation(error: &UnityError) -> Option<ErrorRemediation> {
    let value = error.extra.get("remediation")?;
    let command = match value {
        Value::String(text) => text.trim().to_string(),
        Value::Object(object) => object
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        _ => String::new(),
    };
    if command.is_empty() {
        return None;
    }

    Some(ErrorRemediation {
        kind: RemediationKind::ShowCommand,
        label: "Copy command",
        command: Some(command),
        hint: Some(
            "The Unity CLI named this remediation. The app copies it; it never runs a command from an error by itself."
                .to_string(),
        ),
    })
}

fn code_remediation(code: &str, proxy: Option<&str>) -> Option<ErrorRemediation> {
    let remediation = match code {
        crate::surface::error_code::AUTH_ERROR
            | crate::surface::error_code::NOT_SIGNED_IN
            | crate::surface::error_code::SESSION_EXPIRED => ErrorRemediation::new(
            RemediationKind::SignIn,
            "Sign in",
            Some(
                "Run `unity auth login` from the Overview screen. This is a Unity sign-in, not a VCS token."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::NETWORK_UNREACHABLE
            | crate::surface::error_code::VCS_PROVIDER_UNREACHABLE => ErrorRemediation::new(
            RemediationKind::Retry,
            "Retry",
            Some(network_hint(proxy)),
        ),
        crate::surface::error_code::INSTALL_LOCKED => ErrorRemediation::new(
            RemediationKind::Retry,
            "Retry",
            Some(
                "Another Unity install holds the lock. Retry after it finishes; do not start a second install."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::RELEASE_NOT_FOUND => ErrorRemediation::new(
            RemediationKind::PickVersion,
            "Pick another version",
            Some("Choose a release the CLI lists on the Editors screen.".to_string()),
        ),
        crate::surface::error_code::EDITOR_UNREACHABLE => ErrorRemediation::new(
            RemediationKind::OpenEditor,
            "Open a project",
            Some(
                "Open the project in its Editor first, then refresh. The app does not start an Editor for you."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::TIMEOUT
            | crate::surface::error_code::EDITOR_TIMEOUT => ErrorRemediation::new(
            RemediationKind::Retry,
            "Retry",
            Some(
                "The CLI timed out. Retry, or give the operation more time where it accepts a timeout."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::UNAVAILABLE
            | crate::surface::error_code::STATUS_SAFE_MODE => ErrorRemediation::new(
            RemediationKind::ShowBusy,
            "Show status",
            Some(
                "The target is busy or in Safe Mode. Check the running Editors card; fix compile errors to leave Safe Mode, then retry."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::AWAITING_INPUT
        | crate::surface::error_code::EDITOR_BLOCKED_BY_DIALOG
        | crate::surface::error_code::PROJECTS_CLOSE_BLOCKED_BY_DIALOG => ErrorRemediation::new(
            RemediationKind::AnswerDialog,
            "Answer the dialog",
            Some(
                "A dialog is holding the Editor. Answer it in the Editor, then retry. For a close, `--force` is the explicit fallback; do not auto-retry a close or a command that may have run."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::INVALID_COMMAND_ARGS => ErrorRemediation::new(
            RemediationKind::ShowCommand,
            "Show the command",
            Some(
                "The CLI rejected the arguments. If a proxy environment variable (`UNITY_PROXY`, `HTTPS_PROXY`, `HTTP_PROXY`, or `ALL_PROXY`) or the saved proxy setting names a PAC script, fix or unset it first. Review the command before retrying."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::EDITOR_NOT_INSTALLED => ErrorRemediation::new(
            RemediationKind::PickVersion,
            "Install that version",
            Some(
                "That Editor version is not installed. Install it from the Editors screen, then retry."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::NOT_A_PROJECT => ErrorRemediation::new(
            RemediationKind::OpenEditor,
            "Choose a project",
            Some(
                "That path is not a Unity project. Choose a folder that has `Assets` and `ProjectSettings`."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::PROJECTS_REMOVE_NOT_FOUND => ErrorRemediation::new(
            RemediationKind::OpenEditor,
            "Refresh the project list",
            Some(
                "The CLI no longer has that project registered. Refresh the Projects page and choose a registered project."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::VCS_TOKEN_REQUIRED => ErrorRemediation::new(
            RemediationKind::EnterToken,
            "Enter a token",
            Some(
                "This needs a GitHub or GitLab token, not a Unity sign-in: set `UNITY_GITHUB_TOKEN` or `UNITY_GITLAB_TOKEN`, or pass `--git-token-from-stdin` where the command supports it. This build has no token field yet."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::GIT_START_FAILED => ErrorRemediation::new(
            RemediationKind::InstallGit,
            "Install git",
            Some(
                "Install Git and make sure `git` is on PATH, then retry. The app does not install system packages on its own."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::LOCAL_STORE_UNWRITABLE => ErrorRemediation::new(
            RemediationKind::FixPermissions,
            "Fix permissions",
            Some(
                "Fix write permissions on the CLI's configuration directory, then retry. Settings shows this app's config directory."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::LINK_VCS_DELETE_REMOTE_REQUIRES_YES => ErrorRemediation::new(
            RemediationKind::Confirm,
            "Confirm explicitly",
            Some(
                "Deleting the remote copy needs an explicit confirmation. Review the command and confirm it yourself; the app never passes `--yes` for you here."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::INSTALL_HEADLESS_UNAVAILABLE => ErrorRemediation::new(
            RemediationKind::PickVersion,
            "Pick another version",
            Some(
                "Pick a version that ships a headless build, or clear the headless option and install the regular Editor."
                    .to_string(),
            ),
        ),
        crate::surface::error_code::BAD_ARGS => ErrorRemediation::new(
            RemediationKind::ShowCommand,
            "Show the command",
            Some(
                "Review the command and arguments before running it again. The app passes argv values separately and never builds a shell string."
                    .to_string(),
            ),
        ),
        _ => return None,
    };

    Some(remediation)
}

fn exit_remediation(code: i32, proxy: Option<&str>) -> Option<ErrorRemediation> {
    // Exit codes are the last resort, but they still describe the CLI's
    // documented retry semantics. The table names the stable code whose
    // remediation applies; `None` stays generic.
    let spec = crate::surface::exit_code_spec(code)?;
    code_remediation(spec.remediation_code?, proxy)
}

fn network_hint(proxy: Option<&str>) -> String {
    match proxy.map(str::trim).filter(|value| !value.is_empty()) {
        Some(proxy) => format!(
            "Retry when the connection is back. The CLI is using the configured proxy `{proxy}`; if that proxy is down, fix it or unset it first."
        ),
        None => "Retry when the connection is back. This app has no proxy setting of its own: the CLI reads `UNITY_PROXY` (or `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY`) and the persisted `unity config proxy` value, so configure the proxy there."
            .to_string(),
    }
}

/// Strips credentials from a proxy URL before it reaches a view model, a log,
/// or a command preview. The host and port stay visible because they are the
/// part a user needs to check.
pub fn redact_proxy(value: &str) -> String {
    let value = value.trim();
    let Some((scheme, rest)) = value.split_once("://") else {
        return value.to_string();
    };
    match rest.split_once('@') {
        Some((credentials, host)) if !credentials.is_empty() => {
            format!("{}://***@{host}", scheme)
        }
        _ => value.to_string(),
    }
}

pub fn message_for_exit_code(code: i32) -> &'static str {
    crate::surface::exit_code_spec(code)
        .map(|spec| spec.message)
        .unwrap_or("The Unity CLI command failed.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_codes_match_the_cli_documentation() {
        for code in [7, 9, 10, 12, 14, 17, 18, 130, 143] {
            assert!(is_retryable_exit_code(code), "{code} should be retryable");
        }
        for code in [0, 1, 2, 3, 4, 6, 8, 11, 13, 15, 16, 141] {
            assert!(!is_retryable_exit_code(code), "{code} should be terminal");
        }
    }

    #[test]
    fn exit_code_names_are_stable() {
        assert_eq!(code_for_exit_code(12), "EDITOR_UNREACHABLE");
        assert_eq!(code_for_exit_code(17), "UNAVAILABLE");
        assert_eq!(code_for_exit_code(18), "AWAITING_INPUT");
        assert_eq!(code_for_exit_code(130), "INTERRUPTED");
        assert_eq!(code_for_exit_code(99), "ERROR");
    }

    #[test]
    fn newer_exit_codes_have_friendly_messages() {
        assert!(message_for_exit_code(17).contains("busy"));
        assert!(message_for_exit_code(18).contains("dialog"));
    }

    #[test]
    fn remediation_reads_an_object_from_the_flattened_extra_field() {
        let cli_error: crate::envelope::CliError = serde_json::from_value(serde_json::json!({
            "code": "INSTALL_LOCKED",
            "message": "Another Unity install is holding the install lock.",
            "remediation": {
                "command": "unity editors install --wait 6000.6.6f1",
                "requiresUserApproval": false
            }
        }))
        .expect("error parses");

        let error = cli_error.to_unity_error(Some(9), "");
        let remediation = error.remediation().expect("remediation is preserved");

        assert_eq!(
            remediation.command,
            "unity editors install --wait 6000.6.6f1"
        );
        assert!(!remediation.requires_user_approval);
    }

    #[test]
    fn remediation_reads_a_bare_string_and_requires_approval() {
        let cli_error: crate::envelope::CliError = serde_json::from_value(serde_json::json!({
            "code": "AUTH_ERROR",
            "message": "Unity sign-in is required.",
            "remediation": "unity auth login"
        }))
        .expect("error parses");

        let error = cli_error.to_unity_error(Some(3), "");
        let remediation = error.remediation().expect("remediation is preserved");

        assert_eq!(remediation.command, "unity auth login");
        assert!(remediation.requires_user_approval);
    }

    #[test]
    fn remediation_is_absent_when_the_error_has_none() {
        let error = UnityError::new("ERROR", "Something failed.");

        assert!(error.remediation().is_none());
    }

    fn error(code: &str, exit_code: Option<i32>) -> UnityError {
        UnityError::new(code, "CLI message")
            .with_exit_code(exit_code)
            .with_detail("CLI detail")
    }

    #[test]
    fn every_documented_code_maps_to_its_action() {
        let expected = [
            ("AUTH_ERROR", RemediationKind::SignIn),
            ("NOT_SIGNED_IN", RemediationKind::SignIn),
            ("SESSION_EXPIRED", RemediationKind::SignIn),
            ("NETWORK_UNREACHABLE", RemediationKind::Retry),
            ("VCS_PROVIDER_UNREACHABLE", RemediationKind::Retry),
            ("INSTALL_LOCKED", RemediationKind::Retry),
            ("RELEASE_NOT_FOUND", RemediationKind::PickVersion),
            ("EDITOR_UNREACHABLE", RemediationKind::OpenEditor),
            ("TIMEOUT", RemediationKind::Retry),
            ("EDITOR_TIMEOUT", RemediationKind::Retry),
            ("UNAVAILABLE", RemediationKind::ShowBusy),
            ("STATUS_SAFE_MODE", RemediationKind::ShowBusy),
            ("AWAITING_INPUT", RemediationKind::AnswerDialog),
            ("EDITOR_BLOCKED_BY_DIALOG", RemediationKind::AnswerDialog),
            (
                "PROJECTS_CLOSE_BLOCKED_BY_DIALOG",
                RemediationKind::AnswerDialog,
            ),
            ("INVALID_COMMAND_ARGS", RemediationKind::ShowCommand),
            ("EDITOR_NOT_INSTALLED", RemediationKind::PickVersion),
            ("NOT_A_PROJECT", RemediationKind::OpenEditor),
            ("PROJECTS_REMOVE_NOT_FOUND", RemediationKind::OpenEditor),
            ("VCS_TOKEN_REQUIRED", RemediationKind::EnterToken),
            ("GIT_START_FAILED", RemediationKind::InstallGit),
            ("LOCAL_STORE_UNWRITABLE", RemediationKind::FixPermissions),
            (
                "LINK_VCS_DELETE_REMOTE_REQUIRES_YES",
                RemediationKind::Confirm,
            ),
            ("INSTALL_HEADLESS_UNAVAILABLE", RemediationKind::PickVersion),
            ("BAD_ARGS", RemediationKind::ShowCommand),
        ];

        for (code, kind) in expected {
            let remediation = remediation_for(&error(code, Some(1)), None)
                .unwrap_or_else(|| panic!("{code} should have a remediation"));
            assert_eq!(remediation.kind, kind, "{code} mapped to the wrong action");
            assert!(!remediation.label.trim().is_empty(), "{code} has no label");
            assert!(remediation.hint.is_some(), "{code} has no guidance");
        }
    }

    #[test]
    fn cli_remediation_wins_over_code_and_exit_code() {
        let mut error = error("NETWORK_UNREACHABLE", Some(7));
        error.extra.insert(
            "remediation".to_string(),
            serde_json::json!({ "command": "unity diagnose proxy", "requiresUserApproval": true }),
        );

        let remediation = remediation_for(&error, None).expect("CLI remediation");
        assert_eq!(remediation.kind, RemediationKind::ShowCommand);
        assert_eq!(remediation.command.as_deref(), Some("unity diagnose proxy"));
    }

    #[test]
    fn cli_remediation_accepts_a_plain_string() {
        let mut error = error("BAD_ARGS", Some(2));
        error.extra.insert(
            "remediation".to_string(),
            Value::String("unity --help".to_string()),
        );

        let remediation = remediation_for(&error, None).expect("CLI remediation");
        assert_eq!(remediation.command.as_deref(), Some("unity --help"));
    }

    #[test]
    fn code_mapping_wins_over_exit_code() {
        // AUTH_ERROR is exit 3, but the code is what identifies the action.
        let remediation = remediation_for(&error("AUTH_ERROR", Some(7)), None).expect("mapping");
        assert_eq!(remediation.kind, RemediationKind::SignIn);
    }

    #[test]
    fn unknown_codes_fall_back_to_the_exit_code_then_to_generic_rendering() {
        let remediation =
            remediation_for(&error("FUTURE_CODE", Some(7)), None).expect("exit fallback");
        assert_eq!(remediation.kind, RemediationKind::Retry);

        assert!(remediation_for(&error("FUTURE_CODE", Some(1)), None).is_none());
        assert!(remediation_for(&error("FUTURE_CODE", None), None).is_none());
    }

    #[test]
    fn exit_code_fallbacks_cover_the_documented_retry_semantics() {
        for (code, expected) in [
            (2, RemediationKind::ShowCommand),
            (3, RemediationKind::SignIn),
            (7, RemediationKind::Retry),
            (9, RemediationKind::Retry),
            (12, RemediationKind::OpenEditor),
            (14, RemediationKind::Retry),
            (17, RemediationKind::ShowBusy),
            (18, RemediationKind::AnswerDialog),
        ] {
            let remediation = remediation_for(&error("FUTURE_CODE", Some(code)), None)
                .unwrap_or_else(|| panic!("exit {code} should have a fallback"));
            assert_eq!(remediation.kind, expected, "exit {code}");
        }
        for code in [1, 6, 8, 11, 15, 16, 130, 143] {
            assert!(
                remediation_for(&error("FUTURE_CODE", Some(code)), None).is_none(),
                "exit {code} should stay generic"
            );
        }
    }

    #[test]
    fn exit_18_overrides_an_editor_timeout_code() {
        let remediation =
            remediation_for(&error("EDITOR_TIMEOUT", Some(18)), None).expect("mapping");
        assert_eq!(remediation.kind, RemediationKind::AnswerDialog);
        assert!(remediation
            .hint
            .expect("hint")
            .to_lowercase()
            .contains("dialog"));
    }

    #[test]
    fn dialog_codes_offer_the_dialog_action() {
        for code in [
            "EDITOR_BLOCKED_BY_DIALOG",
            "PROJECTS_CLOSE_BLOCKED_BY_DIALOG",
        ] {
            let remediation = remediation_for(&error(code, Some(18)), None).expect("mapping");
            assert_eq!(remediation.kind, RemediationKind::AnswerDialog, "{code}");
        }
    }

    #[test]
    fn not_a_project_does_not_offer_the_release_picker() {
        let remediation =
            remediation_for(&error("NOT_A_PROJECT", Some(11)), None).expect("mapping");
        assert_eq!(remediation.kind, RemediationKind::OpenEditor);
        let hint = remediation.hint.expect("hint").to_lowercase();
        assert!(hint.contains("assets"));
        assert!(hint.contains("projectsettings"));
    }

    #[test]
    fn unregistered_project_does_not_offer_the_release_picker() {
        let remediation =
            remediation_for(&error("PROJECTS_REMOVE_NOT_FOUND", Some(11)), None).expect("mapping");
        assert_eq!(remediation.kind, RemediationKind::OpenEditor);
    }

    #[test]
    fn editor_not_installed_points_at_the_editors_screen() {
        let remediation =
            remediation_for(&error("EDITOR_NOT_INSTALLED", Some(11)), None).expect("mapping");
        assert_eq!(remediation.kind, RemediationKind::PickVersion);
        assert_eq!(remediation.label, "Install that version");
    }

    #[test]
    fn invalid_command_args_mentions_a_pac_proxy() {
        let remediation =
            remediation_for(&error("INVALID_COMMAND_ARGS", Some(2)), None).expect("mapping");
        assert_eq!(remediation.kind, RemediationKind::ShowCommand);
        let hint = remediation.hint.expect("hint").to_lowercase();
        assert!(hint.contains("pac"));
        assert!(hint.contains("proxy"));
    }

    #[test]
    fn network_errors_name_the_proxy_only_when_one_is_configured() {
        let with_proxy = remediation_for(
            &error("NETWORK_UNREACHABLE", Some(7)),
            Some("UNITY_PROXY=http://proxy.example:8080"),
        )
        .expect("mapping");
        let hint = with_proxy.hint.expect("hint");
        assert!(hint.contains("UNITY_PROXY=http://proxy.example:8080"));

        let without_proxy =
            remediation_for(&error("NETWORK_UNREACHABLE", Some(7)), None).expect("mapping");
        let hint = without_proxy.hint.expect("hint");
        assert!(hint.contains("no proxy setting"));
        assert!(hint.contains("UNITY_PROXY"));
    }

    #[test]
    fn vcs_token_required_points_at_a_token_not_a_sign_in() {
        let remediation =
            remediation_for(&error("VCS_TOKEN_REQUIRED", Some(4)), None).expect("mapping");
        assert_eq!(remediation.kind, RemediationKind::EnterToken);
        let hint = remediation.hint.expect("hint").to_lowercase();
        assert!(hint.contains("token"));
        assert!(hint.contains("unity_github_token"));
        assert!(hint.contains("--git-token-from-stdin"));
        assert!(!hint.contains("auth login"));
    }

    #[test]
    fn headless_install_failure_names_both_ways_out() {
        let remediation = remediation_for(&error("INSTALL_HEADLESS_UNAVAILABLE", Some(17)), None)
            .expect("mapping");
        assert_eq!(remediation.kind, RemediationKind::PickVersion);
        let hint = remediation.hint.expect("hint");
        assert!(hint.contains("headless"));
        assert!(hint.contains("version"));
    }

    #[test]
    fn proxy_credentials_are_redacted() {
        assert_eq!(
            redact_proxy("http://user:secret@proxy.example:8080"),
            "http://***@proxy.example:8080"
        );
        assert_eq!(
            redact_proxy("https://token@proxy.example"),
            "https://***@proxy.example"
        );
        assert_eq!(
            redact_proxy("socks5://proxy.example:1080"),
            "socks5://proxy.example:1080"
        );
    }

    #[test]
    fn only_retry_remediations_are_marked_retryable() {
        assert!(RemediationKind::Retry.is_retry());
        assert!(!RemediationKind::SignIn.is_retry());
        assert!(!RemediationKind::ShowCommand.is_retry());
    }
}
