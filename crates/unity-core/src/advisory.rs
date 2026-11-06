//! Advisories the CLI attaches to an invocation.
//!
//! Both `notifications[]` and `warnings[]` in the standard JSON envelope are
//! advisory data: they never make a command fail, and the app must show them
//! without inventing semantics. [`CliAdvisory`] normalizes both arrays into one
//! shape; [`AdvisorySink`] carries them from every read and every streamed job
//! result frame to the controller, which decides what to display.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::envelope::{CliEnvelope, CliNotification, CliRemediation, CliWarning};

/// Which envelope array an advisory came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvisoryOrigin {
    Notification,
    Warning,
}

/// One normalized advisory.
#[derive(Debug, Clone, PartialEq)]
pub struct CliAdvisory {
    pub origin: AdvisoryOrigin,
    /// Notification code. Warnings only carry one when the CLI sent an object
    /// with a `code` field; a plain string warning has none.
    pub code: Option<String>,
    /// The CLI's (localized) message, or the warning text.
    pub message: String,
    /// Per-code payload. Only known codes have documented fields; unknown ones
    /// are passed through untouched.
    pub data: Option<Value>,
    pub remediation: Option<CliRemediation>,
}

impl CliAdvisory {
    fn from_notification(notification: &CliNotification, origin: AdvisoryOrigin) -> Option<Self> {
        let code = non_empty(&notification.code);
        let message = non_empty(&notification.message)
            .or_else(|| code.clone())
            .unwrap_or_default();
        if message.is_empty() {
            // A notification with neither a code nor a message cannot be
            // rendered; dropping it is better than an empty banner.
            return None;
        }
        Some(Self {
            origin,
            code,
            message,
            data: notification.data.clone(),
            remediation: notification
                .remediation
                .as_ref()
                .filter(|remediation| non_empty(&remediation.command).is_some())
                .cloned(),
        })
    }

    fn from_warning(warning: &CliWarning) -> Option<Self> {
        match warning {
            CliWarning::Text(text) => {
                let message = non_empty(text)?;
                Some(Self {
                    origin: AdvisoryOrigin::Warning,
                    code: None,
                    message,
                    data: None,
                    remediation: None,
                })
            }
            CliWarning::Record(notification) => {
                Self::from_notification(notification, AdvisoryOrigin::Warning)
            }
            CliWarning::Other(value) => {
                let message = match value {
                    Value::String(text) => non_empty(text),
                    other => non_empty(&other.to_string()),
                }?;
                Some(Self {
                    origin: AdvisoryOrigin::Warning,
                    code: None,
                    message,
                    data: None,
                    remediation: None,
                })
            }
        }
    }

    /// Stable identity for deduplication and dismissal.
    ///
    /// `code` plus the `data` fields the CLI documents as stable for that code.
    /// Unknown codes have no documented stable fields, so their (localized)
    /// message joins the identity: a polled refresh collapses, while two
    /// genuinely different unknown advisories stay separate. Plain warnings
    /// are identified by their text.
    pub fn identity(&self) -> String {
        let origin = match self.origin {
            AdvisoryOrigin::Notification => "notification",
            AdvisoryOrigin::Warning => "warning",
        };
        let Some(code) = self.code.as_deref() else {
            return format!("{origin}:{}", self.message.trim());
        };

        let mut identity = format!("{origin}:{code}");
        for key in stable_data_keys(code) {
            let Some(value) = self.data.as_ref().and_then(|data| data.get(*key)) else {
                continue;
            };
            identity.push(':');
            identity.push_str(key);
            identity.push('=');
            identity.push_str(&scalar_text(value));
        }
        if stable_data_keys(code).is_empty() {
            identity.push(':');
            identity.push_str(self.message.trim());
        }
        identity
    }
}

/// The `data` fields that identify one advisory for a known code.
fn stable_data_keys(code: &str) -> &'static [&'static str] {
    match code {
        "CLI_UPDATE_AVAILABLE" => &["current", "latest"],
        "PLUGIN_UPDATE_AVAILABLE" => &["plugin", "current", "latest"],
        "FEX_EMU_EXPERIMENTAL" | "AGENT_PLUGIN_AVAILABLE" => &[],
        _ => &[],
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Normalizes every advisory an envelope carries, notifications first.
pub fn advisories_from_envelope<T>(envelope: &CliEnvelope<T>) -> Vec<CliAdvisory> {
    normalize(&envelope.notifications, &envelope.warnings)
}

/// Advisories from a raw JSON frame.
///
/// Streamed `result` frames are close to, but not guaranteed to be, a complete
/// envelope. Reading the two arrays directly means a frame missing `success`
/// still delivers its advisory instead of losing it.
pub fn advisories_from_value(value: &Value) -> Vec<CliAdvisory> {
    let notifications = value
        .get("notifications")
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<CliNotification>>(value).ok())
        .unwrap_or_default();
    let warnings = value
        .get("warnings")
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<CliWarning>>(value).ok())
        .unwrap_or_default();

    normalize(&notifications, &warnings)
}

fn normalize(notifications: &[CliNotification], warnings: &[CliWarning]) -> Vec<CliAdvisory> {
    notifications
        .iter()
        .filter_map(|notification| {
            CliAdvisory::from_notification(notification, AdvisoryOrigin::Notification)
        })
        .chain(warnings.iter().filter_map(CliAdvisory::from_warning))
        .collect()
}

/// A bounded hand-off between the CLI adapter/jobs and the controller.
///
/// Reads and job threads push; the controller drains after every update, so no
/// advisory can be lost between an envelope and the view model. The bound only
/// protects against a sink nobody drains.
#[derive(Debug, Clone, Default)]
pub struct AdvisorySink {
    advisories: Arc<Mutex<Vec<CliAdvisory>>>,
}

const MAX_PENDING_ADVISORIES: usize = 512;

impl AdvisorySink {
    pub fn push(&self, advisories: impl IntoIterator<Item = CliAdvisory>) {
        let mut pending = self.pending();
        pending.extend(advisories);
        if pending.len() > MAX_PENDING_ADVISORIES {
            let overflow = pending.len() - MAX_PENDING_ADVISORIES;
            pending.drain(..overflow);
        }
    }

    pub fn drain(&self) -> Vec<CliAdvisory> {
        std::mem::take(&mut *self.pending())
    }

    fn pending(&self) -> std::sync::MutexGuard<'_, Vec<CliAdvisory>> {
        self.advisories.lock().expect("advisory lock poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::ValueEnvelope;

    fn envelope(value: Value) -> ValueEnvelope {
        serde_json::from_value(value).expect("envelope should deserialize")
    }

    #[test]
    fn normalizes_notifications_and_warnings_in_order() {
        let envelope = envelope(serde_json::json!({
            "success": true,
            "notifications": [
                {
                    "code": "CLI_UPDATE_AVAILABLE",
                    "message": "A new CLI build is available.",
                    "data": { "current": "1.0.0-beta.16", "latest": "1.0.0-beta.17" },
                    "remediation": { "command": "unity self-update", "requiresUserApproval": true }
                },
                { "code": "FEX_EMU_EXPERIMENTAL", "message": "Running under FEX-Emu." }
            ],
            "warnings": [
                "The plugin is a beta release.",
                { "message": "The --skip flag is deprecated.", "code": null }
            ]
        }));

        let advisories = advisories_from_envelope(&envelope);
        assert_eq!(advisories.len(), 4);

        assert_eq!(advisories[0].origin, AdvisoryOrigin::Notification);
        assert_eq!(advisories[0].code.as_deref(), Some("CLI_UPDATE_AVAILABLE"));
        assert_eq!(
            advisories[0]
                .remediation
                .as_ref()
                .map(|remediation| remediation.command.as_str()),
            Some("unity self-update")
        );
        assert!(advisories[0]
            .remediation
            .as_ref()
            .is_some_and(|remediation| remediation.requires_user_approval));

        assert_eq!(advisories[1].code.as_deref(), Some("FEX_EMU_EXPERIMENTAL"));
        assert!(advisories[1].remediation.is_none());

        assert_eq!(advisories[2].origin, AdvisoryOrigin::Warning);
        assert_eq!(advisories[2].code, None);
        assert_eq!(advisories[2].message, "The plugin is a beta release.");

        assert_eq!(advisories[3].message, "The --skip flag is deprecated.");
        assert_eq!(advisories[3].origin, AdvisoryOrigin::Warning);
    }

    #[test]
    fn unknown_notification_codes_and_missing_fields_survive() {
        let envelope = envelope(serde_json::json!({
            "success": true,
            "notifications": [{ "code": "FUTURE_CODE" }]
        }));

        let advisories = advisories_from_envelope(&envelope);
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].code.as_deref(), Some("FUTURE_CODE"));
        // No message: the code is the only thing the CLI gave us.
        assert_eq!(advisories[0].message, "FUTURE_CODE");
        assert!(advisories[0].data.is_none());
        assert!(advisories[0].remediation.is_none());
    }

    #[test]
    fn advisories_without_a_code_or_message_are_dropped() {
        let envelope = envelope(serde_json::json!({
            "success": true,
            "notifications": [{ "data": { "x": 1 } }],
            "warnings": ["  ", { "message": "" }]
        }));

        assert!(advisories_from_envelope(&envelope).is_empty());
    }

    #[test]
    fn warnings_accept_strings_objects_and_unexpected_shapes() {
        let envelope = envelope(serde_json::json!({
            "success": true,
            "warnings": [
                "plain text",
                { "message": "object text", "detail": "ignored" },
                42
            ]
        }));

        let advisories = advisories_from_envelope(&envelope);
        let messages: Vec<&str> = advisories
            .iter()
            .map(|advisory| advisory.message.as_str())
            .collect();
        assert_eq!(messages, vec!["plain text", "object text", "42"]);
    }

    #[test]
    fn identity_uses_code_and_stable_data_fields() {
        let base = envelope(serde_json::json!({
            "success": true,
            "notifications": [{
                "code": "CLI_UPDATE_AVAILABLE",
                "message": "Update available (localized text may change).",
                "data": { "current": "1.0.0-beta.16", "latest": "1.0.0-beta.17", "platform": "linux" }
            }]
        }));
        let other_platform = envelope(serde_json::json!({
            "success": true,
            "notifications": [{
                "code": "CLI_UPDATE_AVAILABLE",
                "message": "A new CLI is available.",
                "data": { "current": "1.0.0-beta.16", "latest": "1.0.0-beta.17", "platform": "macos" }
            }]
        }));
        let newer = envelope(serde_json::json!({
            "success": true,
            "notifications": [{
                "code": "CLI_UPDATE_AVAILABLE",
                "message": "Update available.",
                "data": { "current": "1.0.0-beta.16", "latest": "1.0.0-beta.18" }
            }]
        }));

        let first = advisories_from_envelope(&base).remove(0);
        let second = advisories_from_envelope(&other_platform).remove(0);
        let third = advisories_from_envelope(&newer).remove(0);

        // Localized message and non-stable fields do not change identity.
        assert_eq!(first.identity(), second.identity());
        assert_ne!(first.identity(), third.identity());
    }

    #[test]
    fn plugin_identity_includes_the_plugin_id() {
        let one = envelope(serde_json::json!({
            "success": true,
            "notifications": [{
                "code": "PLUGIN_UPDATE_AVAILABLE",
                "message": "Plugin update available.",
                "data": { "plugin": "com.unity.ai", "current": "1.0.0", "latest": "1.1.0" }
            }]
        }));
        let two = envelope(serde_json::json!({
            "success": true,
            "notifications": [{
                "code": "PLUGIN_UPDATE_AVAILABLE",
                "message": "Plugin update available.",
                "data": { "plugin": "com.unity.mcp", "current": "1.0.0", "latest": "1.1.0" }
            }]
        }));

        assert_ne!(
            advisories_from_envelope(&one)[0].identity(),
            advisories_from_envelope(&two)[0].identity()
        );
    }

    #[test]
    fn plain_warnings_dedupe_on_text() {
        let envelope = envelope(serde_json::json!({
            "success": true,
            "warnings": ["Beta plugin.", "Beta plugin."]
        }));
        let advisories = advisories_from_envelope(&envelope);
        assert_eq!(advisories[0].identity(), advisories[1].identity());
    }

    #[test]
    fn raw_frames_deliver_advisories_without_a_full_envelope() {
        let advisories = advisories_from_value(&serde_json::json!({
            "type": "result",
            "warnings": ["This plugin is a beta release."]
        }));

        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].message, "This plugin is a beta release.");
    }

    #[test]
    fn sink_drains_and_keeps_only_the_most_recent_advisories() {
        let sink = AdvisorySink::default();
        let advisory = CliAdvisory {
            origin: AdvisoryOrigin::Warning,
            code: None,
            message: "Beta plugin.".to_string(),
            data: None,
            remediation: None,
        };
        sink.push([advisory.clone()]);
        sink.push(std::iter::repeat_n(advisory, MAX_PENDING_ADVISORIES + 8));

        let drained = sink.drain();
        assert_eq!(drained.len(), MAX_PENDING_ADVISORIES);
        assert!(sink.drain().is_empty());
    }
}
