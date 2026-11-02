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
    #[serde(default)]
    pub warnings: Vec<Value>,
    #[serde(default)]
    pub notifications: Vec<Value>,
}

pub type ValueEnvelope = CliEnvelope<Value>;

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

pub fn parse_data<T: DeserializeOwned>(envelope: &ValueEnvelope) -> Result<T, UnityError> {
    let data = envelope
        .data
        .clone()
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
pub fn parse_items<T: DeserializeOwned>(
    data: Option<&Value>,
    keys: &[&str],
) -> Result<Vec<T>, UnityError> {
    let Some(data) = data else {
        return Ok(Vec::new());
    };
    if data.is_null() {
        return Ok(Vec::new());
    }
    let array = if let Some(array) = data.as_array() {
        array.clone()
    } else if let Some(object) = data.as_object() {
        let mut found = None;
        for key in keys
            .iter()
            .copied()
            .chain(["items", "instances", "results", "entries"])
        {
            if let Some(Value::Array(array)) = object.get(key) {
                found = Some(array.clone());
                break;
            }
        }
        found.ok_or_else(|| {
            UnityError::unexpected_output(format!(
                "Expected a list in `data`, found an object with keys: {}",
                object.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?
    } else {
        return Err(UnityError::unexpected_output(format!(
            "Expected a list in `data`, found {data}."
        )));
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
        let editors: Vec<Editor> = parse_items(Some(&data), &[]).unwrap();
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
        let editors: Vec<Editor> = parse_items(Some(&data), &[]).unwrap();
        assert_eq!(editors.len(), 1);
    }

    #[test]
    fn parses_running_editor_instances() {
        let data = serde_json::json!({
            "count": 1,
            "instances": [{ "projectPath": "/tmp/Game", "editorVersion": "6000.6.6f1", "pid": 42 }]
        });
        let editors: Vec<RunningEditor> = parse_items(Some(&data), &["instances"]).unwrap();
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
