use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
pub fn is_retryable_exit_code(code: i32) -> bool {
    matches!(code, 7 | 9 | 10 | 12 | 14 | 130 | 143)
}

pub fn code_for_exit_code(code: i32) -> &'static str {
    match code {
        0 => "SUCCESS",
        1 => "ERROR",
        2 => "BAD_ARGS",
        3 => "AUTH_ERROR",
        4 => "NEEDS_CONFIG",
        6 => "COMMAND_FAILED",
        7 => "NETWORK_UNREACHABLE",
        8 => "TESTS_FAILED",
        9 => "INSTALL_LOCKED",
        10 => "READY_TIMEOUT",
        11 => "NOT_FOUND",
        12 => "EDITOR_UNREACHABLE",
        13 => "REMOTE_COMMAND_FAILED",
        14 => "TIMEOUT",
        15 => "RUN_FAILED",
        16 => "JOB_SUSPENDED",
        130 => "INTERRUPTED",
        141 => "OUTPUT_CLOSED",
        143 => "TERMINATED",
        _ => "ERROR",
    }
}

pub fn message_for_exit_code(code: i32) -> &'static str {
    match code {
        2 => "The Unity CLI rejected the arguments.",
        3 => "Unity sign-in is required, or the session expired.",
        4 => "The Unity CLI needs configuration before this can run.",
        6 => "The Unity CLI command failed.",
        7 => "A Unity service could not be reached.",
        8 => "The tests ran, and some failed.",
        9 => "Another Unity install is holding the install lock.",
        10 => "The Unity Editor did not become ready in time.",
        11 => "The requested project, release, or module does not exist.",
        12 => "No running Unity Editor answered.",
        13 => "The connected Editor does not expose that command.",
        14 => "The operation timed out.",
        15 => "A build or job finished unsuccessfully.",
        16 => "A job is waiting for a person to resume or approve it.",
        130 => "The operation was interrupted.",
        141 => "The output reader closed the pipe.",
        143 => "The operation was terminated.",
        _ => "The Unity CLI command failed.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_codes_match_the_cli_documentation() {
        for code in [7, 9, 10, 12, 14, 130, 143] {
            assert!(is_retryable_exit_code(code), "{code} should be retryable");
        }
        for code in [0, 1, 2, 3, 4, 6, 8, 11, 13, 15, 16, 141] {
            assert!(!is_retryable_exit_code(code), "{code} should be terminal");
        }
    }

    #[test]
    fn exit_code_names_are_stable() {
        assert_eq!(code_for_exit_code(12), "EDITOR_UNREACHABLE");
        assert_eq!(code_for_exit_code(130), "INTERRUPTED");
        assert_eq!(code_for_exit_code(99), "ERROR");
    }
}
