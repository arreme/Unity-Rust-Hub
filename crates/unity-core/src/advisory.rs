//! Advisories the Unity CLI attaches to its JSON envelope.
//!
//! The CLI reports update notices, deprecations and similar hints in the
//! `warnings` and `notifications` arrays of an envelope. They are not failures,
//! so they never become [`crate::UnityError`]s; the app collects them for the
//! notification centre instead.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// How the CLI classified an advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdvisoryKind {
    Warning,
    Notification,
    Error,
}

/// A command the CLI suggests for resolving an advisory or an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Remediation {
    pub command: String,
    /// When true the GUI must not run the command without an explicit
    /// confirmation from the user.
    pub requires_user_approval: bool,
}

/// One advisory entry, normalized from the loose shapes the CLI emits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Advisory {
    pub kind: AdvisoryKind,
    pub code: Option<String>,
    pub message: String,
    pub data: Option<Value>,
    pub remediation: Option<Remediation>,
}

impl Advisory {
    /// Parses one entry from `warnings` or `notifications`.
    ///
    /// A bare string is a message with no code; an object reads `message`
    /// (falling back to `text`, then `detail`), `code`, `data` and
    /// `remediation`. Entries without usable text are dropped.
    pub fn from_value(kind: AdvisoryKind, value: &Value) -> Option<Self> {
        match value {
            Value::String(text) => {
                let message = text.trim();
                if message.is_empty() {
                    return None;
                }
                Some(Self {
                    kind,
                    code: None,
                    message: message.to_string(),
                    data: None,
                    remediation: None,
                })
            }
            Value::Object(object) => {
                let message = message_from(object)?;
                let code = object
                    .get("code")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|code| !code.is_empty())
                    .map(str::to_string);
                let data = object.get("data").filter(|data| !data.is_null()).cloned();
                let remediation = object.get("remediation").and_then(remediation_from_value);
                Some(Self {
                    kind,
                    code,
                    message,
                    data,
                    remediation,
                })
            }
            _ => None,
        }
    }

    /// The identity used to collapse repeats of the same advisory.
    ///
    /// Entries with a code and `data` are keyed by the code plus canonicalised
    /// `data`, so two identical refreshes produce the same key. Entries with a
    /// code but no `data` include the message, because two different warnings
    /// can share a code. Entries without a code fall back to their message.
    pub fn dedupe_key(&self) -> String {
        match (&self.code, &self.data) {
            (Some(code), Some(data)) => format!("{code}:data:{}", canonical_data(data)),
            (Some(code), None) => format!("{code}:message:{}", self.message),
            (None, _) => self.message.clone(),
        }
    }
}

/// Reads the first non-blank of `message`, `text`, then `detail`.
fn message_from(object: &Map<String, Value>) -> Option<String> {
    ["message", "text", "detail"].iter().find_map(|key| {
        object
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|message| !message.is_empty())
            .map(str::to_string)
    })
}

/// Parses a `remediation` field, which is either an object or a bare command.
pub(crate) fn remediation_from_value(value: &Value) -> Option<Remediation> {
    match value {
        Value::String(command) => {
            let command = command.trim();
            if command.is_empty() {
                return None;
            }
            Some(Remediation {
                command: command.to_string(),
                requires_user_approval: true,
            })
        }
        Value::Object(object) => {
            let command = object
                .get("command")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|command| !command.is_empty())?;
            let requires_user_approval = object
                .get("requiresUserApproval")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            Some(Remediation {
                command: command.to_string(),
                requires_user_approval,
            })
        }
        _ => None,
    }
}

/// Renders `data` with sorted object keys so repeated refreshes compare equal.
///
/// `serde_json` normally keeps object keys sorted, but that depends on the
/// `preserve_order` feature not being enabled anywhere in the dependency graph,
/// so sort the top level explicitly.
fn canonical_data(data: &Value) -> String {
    match data {
        Value::Object(object) => {
            let sorted: BTreeMap<&String, &Value> = object.iter().collect();
            serde_json::to_string(&sorted).unwrap_or_else(|_| data.to_string())
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_string_becomes_a_message_without_a_code() {
        let advisory = Advisory::from_value(
            AdvisoryKind::Warning,
            &Value::String("The CLI is running in compatibility mode.".to_string()),
        )
        .expect("a non-blank string is an advisory");

        assert_eq!(advisory.kind, AdvisoryKind::Warning);
        assert_eq!(
            advisory.message,
            "The CLI is running in compatibility mode."
        );
        assert!(advisory.code.is_none());
        assert!(advisory.data.is_none());
        assert!(advisory.remediation.is_none());
    }

    #[test]
    fn a_notification_keeps_data_and_object_remediation() {
        let value = serde_json::json!({
            "code": "CLI_UPDATE_AVAILABLE",
            "message": "A new version of the Unity CLI is available: 1.0.0-beta.8 → 1.0.0-beta.9",
            "data": { "current": "1.0.0-beta.8", "latest": "1.0.0-beta.9" },
            "remediation": { "command": "unity self-update", "requiresUserApproval": true }
        });

        let advisory =
            Advisory::from_value(AdvisoryKind::Notification, &value).expect("object is valid");

        assert_eq!(advisory.kind, AdvisoryKind::Notification);
        assert_eq!(advisory.code.as_deref(), Some("CLI_UPDATE_AVAILABLE"));
        assert_eq!(
            advisory.data.as_ref().and_then(|data| data.get("latest")),
            Some(&Value::String("1.0.0-beta.9".to_string()))
        );
        assert_eq!(
            advisory.remediation,
            Some(Remediation {
                command: "unity self-update".to_string(),
                requires_user_approval: true,
            })
        );
    }

    #[test]
    fn missing_requires_user_approval_defaults_to_true() {
        let value = serde_json::json!({
            "message": "Run the installer again.",
            "remediation": { "command": "unity install 6000.6.6f1" }
        });

        let advisory = Advisory::from_value(AdvisoryKind::Warning, &value).expect("valid");
        let remediation = advisory.remediation.expect("remediation is present");

        assert_eq!(remediation.command, "unity install 6000.6.6f1");
        assert!(remediation.requires_user_approval);
    }

    #[test]
    fn a_bare_string_remediation_requires_approval() {
        let value = serde_json::json!({
            "message": "Sign in again.",
            "remediation": "unity auth login"
        });

        let advisory = Advisory::from_value(AdvisoryKind::Warning, &value).expect("valid");

        assert_eq!(
            advisory.remediation,
            Some(Remediation {
                command: "unity auth login".to_string(),
                requires_user_approval: true,
            })
        );
    }

    #[test]
    fn text_and_detail_are_message_fallbacks() {
        let text = Advisory::from_value(
            AdvisoryKind::Warning,
            &serde_json::json!({ "text": "Use --long for details." }),
        )
        .expect("text fallback");
        assert_eq!(text.message, "Use --long for details.");

        let detail = Advisory::from_value(
            AdvisoryKind::Notification,
            &serde_json::json!({ "detail": "Licence renewed." }),
        )
        .expect("detail fallback");
        assert_eq!(detail.message, "Licence renewed.");
    }

    #[test]
    fn blank_and_whitespace_entries_are_dropped() {
        for value in [
            serde_json::json!(""),
            serde_json::json!("   \n\t"),
            serde_json::json!({ "message": "   " }),
            serde_json::json!({ "message": "", "text": "\t" }),
            serde_json::json!({ "code": "CLI_HINT" }),
            serde_json::json!(42),
            serde_json::json!(null),
        ] {
            assert!(
                Advisory::from_value(AdvisoryKind::Warning, &value).is_none(),
                "{value} should be dropped"
            );
        }
    }

    #[test]
    fn dedupe_key_is_stable_across_identical_refreshes() {
        let value = serde_json::json!({
            "code": "CLI_UPDATE_AVAILABLE",
            "message": "A new version is available.",
            "data": { "current": "1.0.0", "latest": "1.1.0" }
        });

        let first = Advisory::from_value(AdvisoryKind::Notification, &value).expect("valid");
        let second = Advisory::from_value(AdvisoryKind::Notification, &value).expect("valid");

        assert_eq!(first.dedupe_key(), second.dedupe_key());
    }

    #[test]
    fn dedupe_key_ignores_data_key_order() {
        let first = Advisory::from_value(
            AdvisoryKind::Notification,
            &serde_json::json!({
                "code": "CLI_UPDATE_AVAILABLE",
                "message": "A new version is available.",
                "data": { "latest": "1.1.0", "current": "1.0.0" }
            }),
        )
        .expect("valid");
        let second = Advisory::from_value(
            AdvisoryKind::Notification,
            &serde_json::json!({
                "code": "CLI_UPDATE_AVAILABLE",
                "message": "A new version is available.",
                "data": { "current": "1.0.0", "latest": "1.1.0" }
            }),
        )
        .expect("valid");

        assert_eq!(first.dedupe_key(), second.dedupe_key());
    }

    #[test]
    fn dedupe_key_changes_when_data_changes() {
        let first = Advisory::from_value(
            AdvisoryKind::Notification,
            &serde_json::json!({
                "code": "CLI_UPDATE_AVAILABLE",
                "message": "A new version is available.",
                "data": { "current": "1.0.0", "latest": "1.1.0" }
            }),
        )
        .expect("valid");
        let second = Advisory::from_value(
            AdvisoryKind::Notification,
            &serde_json::json!({
                "code": "CLI_UPDATE_AVAILABLE",
                "message": "A new version is available.",
                "data": { "current": "1.1.0", "latest": "1.2.0" }
            }),
        )
        .expect("valid");

        assert_ne!(first.dedupe_key(), second.dedupe_key());
    }

    #[test]
    fn dedupe_key_falls_back_to_the_message_without_a_code() {
        let advisory = Advisory::from_value(
            AdvisoryKind::Warning,
            &serde_json::json!({ "message": "Compatibility mode is on." }),
        )
        .expect("valid");

        assert_eq!(advisory.dedupe_key(), "Compatibility mode is on.");
    }

    #[test]
    fn dedupe_key_includes_the_message_when_a_code_has_no_data() {
        let first = Advisory::from_value(
            AdvisoryKind::Warning,
            &serde_json::json!({
                "code": "CLI_CACHE_STALE",
                "message": "The Editor cache is out of date."
            }),
        )
        .expect("valid");
        let second = Advisory::from_value(
            AdvisoryKind::Warning,
            &serde_json::json!({
                "code": "CLI_CACHE_STALE",
                "message": "The templates cache is out of date."
            }),
        )
        .expect("valid");

        assert_ne!(
            first.dedupe_key(),
            second.dedupe_key(),
            "two warnings that share a code must not collapse"
        );
    }

    #[test]
    fn dedupe_key_is_stable_for_the_same_code_and_message_without_data() {
        let value = serde_json::json!({
            "code": "CLI_CACHE_STALE",
            "message": "The Editor cache is out of date."
        });

        let first = Advisory::from_value(AdvisoryKind::Warning, &value).expect("valid");
        let second = Advisory::from_value(AdvisoryKind::Warning, &value).expect("valid");

        assert_eq!(first.dedupe_key(), second.dedupe_key());
    }
}
