use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::errors::{
    code_for_exit_code, is_retryable_exit_code, message_for_exit_code, UnityError,
};

/// The standard Unity CLI JSON envelope.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliEnvelope<T> {
    pub success: bool,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub data: Option<T>,
    #[serde(default)]
    pub errors: Vec<CliError>,
    /// Free-form advisories. The CLI sends strings or objects, so the DTO
    /// accepts both instead of dropping the array.
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub warnings: Vec<CliWarning>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub notifications: Vec<CliNotification>,
}

pub type ValueEnvelope = CliEnvelope<Value>;

/// A `notifications[]` entry. Every field is optional because an older or
/// newer CLI may omit parts of the shape.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliNotification {
    #[serde(default, deserialize_with = "null_as_default_string")]
    pub code: String,
    #[serde(default, deserialize_with = "null_as_default_string")]
    pub message: String,
    /// Per-code payload. Known codes have documented fields; unknown codes are
    /// carried through untouched.
    #[serde(default)]
    pub data: Option<Value>,
    #[serde(default)]
    pub remediation: Option<CliRemediation>,
}

/// A command the CLI names as the remedy for an advisory.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliRemediation {
    #[serde(default, deserialize_with = "null_as_default_string")]
    pub command: String,
    /// The CLI's own request for an explicit human decision. The app never
    /// runs any remediation on its own, and this flag requires a click too.
    #[serde(default, deserialize_with = "null_as_false")]
    pub requires_user_approval: bool,
}

/// The CLI can send `null` for an optional string field; a missing value and an
/// explicit null must both fall back to the default.
fn null_as_default_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn null_as_false<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(false))
}

/// A CLI that writes `null` for an optional array must not fail the read.
fn null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

/// A `warnings[]` entry: a string or an object. `Other` keeps an unexpected
/// scalar from failing the whole envelope.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum CliWarning {
    Text(String),
    Record(CliNotification),
    Other(Value),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliError {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl CliError {
    pub fn to_unity_error(&self, exit_code: Option<i32>, stderr: &str) -> UnityError {
        let code = if self.code.trim().is_empty() {
            exit_code
                .map(code_for_exit_code)
                .unwrap_or("ERROR")
                .to_string()
        } else {
            self.code.clone()
        };
        let message = if self.message.trim().is_empty() {
            exit_code
                .map(message_for_exit_code)
                .unwrap_or("The Unity CLI command failed.")
                .to_string()
        } else {
            self.message.clone()
        };
        let mut detail = self.detail.clone();
        let stderr = stderr.trim();
        if !stderr.is_empty() {
            detail = Some(match detail {
                Some(existing) if !existing.contains(stderr) => {
                    format!("{existing}\n\n{stderr}")
                }
                Some(existing) => existing,
                None => stderr.to_string(),
            });
        }
        UnityError {
            code,
            message,
            detail,
            exit_code,
            retryable: exit_code.map(is_retryable_exit_code).unwrap_or(false),
            extra: self.extra.clone(),
        }
    }
}

pub fn parse_envelope(stdout: &str) -> Result<ValueEnvelope, UnityError> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(UnityError::unexpected_output(
            "The CLI wrote no JSON to stdout.",
        ));
    }
    serde_json::from_str::<ValueEnvelope>(trimmed).map_err(|err| {
        let preview = preview(trimmed, 1200);
        UnityError::unexpected_output(format!("{err}\n\nstdout:\n{preview}"))
    })
}

pub fn parse_data<T: DeserializeOwned>(envelope: ValueEnvelope) -> Result<T, UnityError> {
    let data = envelope
        .data
        .ok_or_else(|| UnityError::unexpected_output("The CLI envelope had no `data` field."))?;
    serde_json::from_value(data)
        .map_err(|err| UnityError::unexpected_output(format!("Could not parse `data`: {err}")))
}

/// Normalizes the shapes the CLI uses for listings.
///
/// The reference documents `data.items[]`, while current beta builds return
/// plain arrays for most listings and `{ count, instances }` for running
/// Editors. Accept all of them so a CLI update on either side does not break
/// the app.
///
/// Takes ownership of `data` so list items can be deserialized in place instead
/// of deep-cloning the entire array first.
pub fn parse_items<T: DeserializeOwned>(
    data: Option<Value>,
    keys: &[&str],
) -> Result<Vec<T>, UnityError> {
    let Some(data) = data else {
        return Ok(Vec::new());
    };
    if data.is_null() {
        return Ok(Vec::new());
    }
    let array = match data {
        Value::Array(array) => array,
        Value::Object(mut object) => {
            let mut found = None;
            for key in keys
                .iter()
                .copied()
                .chain(["items", "instances", "results", "entries"])
            {
                if let Some(Value::Array(array)) = object.remove(key) {
                    found = Some(array);
                    break;
                }
            }
            found.ok_or_else(|| {
                UnityError::unexpected_output(format!(
                    "Expected a list in `data`, found an object with keys: {}",
                    object.keys().cloned().collect::<Vec<_>>().join(", ")
                ))
            })?
        }
        other => {
            return Err(UnityError::unexpected_output(format!(
                "Expected a list in `data`, found {other}."
            )));
        }
    };

    array
        .into_iter()
        .map(|value| {
            serde_json::from_value(value).map_err(|err| {
                UnityError::unexpected_output(format!("Could not parse item: {err}"))
            })
        })
        .collect()
}

fn preview(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_string()
    } else {
        format!("{}…", value.chars().take(limit).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Editor, RunningEditor};

    #[test]
    fn parses_a_plain_array_listing() {
        let data = serde_json::json!([
            { "version": "6000.6.6f1", "moduleNames": ["Windows"] }
        ]);
        let editors: Vec<Editor> = parse_items(Some(data), &[]).unwrap();
        assert_eq!(editors.len(), 1);
        assert_eq!(editors[0].version, "6000.6.6f1");
        assert_eq!(editors[0].module_names, vec!["Windows"]);
    }

    #[test]
    fn parses_an_items_envelope_listing() {
        let data = serde_json::json!({
            "items": [{ "version": "6000.3.15f1" }],
            "total": 1,
            "nextPage": null
        });
        let editors: Vec<Editor> = parse_items(Some(data), &[]).unwrap();
        assert_eq!(editors.len(), 1);
    }

    #[test]
    fn parses_running_editor_instances() {
        let data = serde_json::json!({
            "count": 1,
            "instances": [{ "projectPath": "/tmp/Game", "editorVersion": "6000.6.6f1", "pid": 42 }]
        });
        let editors: Vec<RunningEditor> = parse_items(Some(data), &["instances"]).unwrap();
        assert_eq!(editors.len(), 1);
        assert_eq!(editors[0].project_path.as_deref(), Some("/tmp/Game"));
    }

    #[test]
    fn failed_envelope_becomes_structured_error() {
        let envelope: ValueEnvelope = serde_json::from_value(serde_json::json!({
            "success": false,
            "command": "status",
            "data": { "count": 0, "instances": [] },
            "errors": [{ "code": "STATUS_NO_INSTANCES", "message": "No instances" }],
            "warnings": []
        }))
        .unwrap();
        let error = envelope.errors[0].to_unity_error(Some(12), "");
        assert_eq!(error.code, "STATUS_NO_INSTANCES");
        assert!(error.retryable);
    }

    #[test]
    fn parses_typed_warnings_and_notifications() {
        let envelope = parse_envelope(
            r#"{
                "success": true,
                "warnings": [
                    "This plugin is a beta release.",
                    { "code": null, "message": "The --skip flag is deprecated." }
                ],
                "notifications": [{
                    "code": "CLI_UPDATE_AVAILABLE",
                    "message": "Update available.",
                    "data": { "current": "1.0.0-beta.16", "latest": "1.0.0-beta.17" },
                    "remediation": { "command": "unity self-update", "requiresUserApproval": true }
                }]
            }"#,
        )
        .unwrap();

        assert!(
            matches!(&envelope.warnings[0], CliWarning::Text(text) if text == "This plugin is a beta release.")
        );
        assert!(
            matches!(&envelope.warnings[1], CliWarning::Record(record) if record.message == "The --skip flag is deprecated.")
        );

        let notification = &envelope.notifications[0];
        assert_eq!(notification.code, "CLI_UPDATE_AVAILABLE");
        assert_eq!(notification.message, "Update available.");
        assert_eq!(
            notification
                .data
                .as_ref()
                .and_then(|data| data.get("latest"))
                .and_then(Value::as_str),
            Some("1.0.0-beta.17")
        );
        let remediation = notification.remediation.as_ref().expect("remediation");
        assert_eq!(remediation.command, "unity self-update");
        assert!(remediation.requires_user_approval);
    }

    #[test]
    fn missing_and_null_advisories_do_not_fail_the_envelope() {
        for envelope in [
            parse_envelope(r#"{"success":true}"#),
            parse_envelope(r#"{"success":true,"warnings":null,"notifications":null}"#),
        ] {
            let envelope = envelope.unwrap();
            assert!(envelope.warnings.is_empty());
            assert!(envelope.notifications.is_empty());
        }

        let envelope =
            parse_envelope(r#"{"success":true,"notifications":[{"code":"FUTURE_CODE"},{}]}"#)
                .unwrap();
        assert_eq!(envelope.notifications.len(), 2);
        assert_eq!(envelope.notifications[0].code, "FUTURE_CODE");
        assert!(envelope.notifications[0].message.is_empty());
        assert!(envelope.notifications[1].code.is_empty());
    }

    #[test]
    fn empty_data_is_an_empty_list() {
        let editors: Vec<Editor> = parse_items(None, &[]).unwrap();
        assert!(editors.is_empty());
    }

    #[test]
    fn preview_truncates_without_splitting_utf8() {
        let text = "áéíóú".repeat(10);
        assert_eq!(preview(&text, 5), "áéíóú…");
    }
}
