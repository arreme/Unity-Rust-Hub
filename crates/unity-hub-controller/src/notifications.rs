//! The notification centre's state: entries, unread count, rail and toast.
//!
//! Everything here is session-only. The store deduplicates CLI advisories by
//! their stable key so a refresh that repeats the same warning does not raise
//! the unread badge or toast again. Events and errors are never deduplicated: a
//! second successful install or a retried failure must always be announced.
//! Remediation commands are only ever stored as validated argument vectors; no
//! shell is involved.

use std::collections::HashSet;

use unity_core::advisory::{Advisory, AdvisoryKind, Remediation};
use unity_core::UnityError;
use unity_hub_model::{NotificationActionState, NotificationVm, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Danger,
}

/// The action state of a stored notification. Kept separate from the view
/// model so the controller never depends on presentation types it does not
/// need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionState {
    Idle,
    Confirming,
    Running,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: u64,
    pub level: NotificationLevel,
    pub label: String,
    pub message: String,
    pub command: Option<String>,
    pub can_run: bool,
    pub state: ActionState,
    /// New entries start unread; opening the rail marks them read.
    pub read: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub level: NotificationLevel,
    pub message: String,
}

/// Newest-first history plus the transient rail/toast state.
#[derive(Debug, Clone, Default)]
pub struct NotificationStore {
    entries: Vec<Notification>,
    rail_open: bool,
    toast: Option<Toast>,
    /// Advisory keys already admitted this session, so a refresh that repeats
    /// the same warning never duplicates an entry.
    seen: HashSet<String>,
    next_id: u64,
}

impl NotificationStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a CLI advisory. Returns false when the same advisory — same code
    /// plus `data`, same code plus message, or the same codeless message — was
    /// already shown this session.
    pub fn push_advisory(&mut self, advisory: &Advisory) -> bool {
        let key = advisory.dedupe_key();
        if self.seen.contains(&key) {
            return false;
        }

        let level = advisory_level(advisory.kind);
        let label = advisory
            .code
            .clone()
            .unwrap_or_else(|| advisory_label(advisory.kind).to_string());
        let (command, can_run) = match &advisory.remediation {
            Some(remediation) => (
                Some(remediation.command.clone()),
                remediation_can_run(remediation),
            ),
            None => (None, false),
        };

        self.seen.insert(key);
        self.admit(level, label, advisory.message.clone(), command, can_run);
        true
    }

    /// Adds a structured failure, keeping its code, detail and remediation.
    /// A repeated failure is a genuine event, so it is always admitted.
    pub fn push_error(&mut self, error: &UnityError) -> bool {
        let (command, can_run) = match error.remediation() {
            Some(remediation) => (
                Some(remediation.command.clone()),
                remediation_can_run(&remediation),
            ),
            None => (None, false),
        };

        self.admit(
            NotificationLevel::Danger,
            error.code.clone(),
            error_text(error),
            command,
            can_run,
        );
        true
    }

    /// Adds a plain message, e.g. a job outcome or a local settings error.
    /// Repeated messages are genuine events and are always admitted.
    pub fn push_event(
        &mut self,
        level: NotificationLevel,
        label: &str,
        message: impl Into<String>,
    ) -> bool {
        self.admit(level, label.to_string(), message.into(), None, false);
        true
    }

    pub fn dismiss(&mut self, id: u64) {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.id != id);
        if self.entries.len() != before && self.toast.as_ref().is_some_and(|toast| toast.id == id) {
            self.toast = None;
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.toast = None;
        // `seen` is deliberately kept: a refresh that repeats a cleared
        // advisory must not resurrect it.
    }

    /// Opening the rail marks every entry read. Entries that arrive afterwards
    /// stay unread until the rail is opened again.
    pub fn set_rail_open(&mut self, open: bool) {
        self.rail_open = open;
        if open {
            for entry in &mut self.entries {
                entry.read = true;
            }
        }
    }

    /// Clears the toast only when it is still the one that expired.
    pub fn toast_expired(&mut self, id: u64) {
        if self.toast.as_ref().is_some_and(|toast| toast.id == id) {
            self.toast = None;
        }
    }

    pub fn set_state(&mut self, id: u64, state: ActionState) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.state = state;
        }
    }

    pub fn get(&self, id: u64) -> Option<&Notification> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn unread(&self) -> u32 {
        let unread = self.entries.iter().filter(|entry| !entry.read).count();
        u32::try_from(unread).unwrap_or(u32::MAX)
    }

    pub fn rail_open(&self) -> bool {
        self.rail_open
    }

    pub fn toast(&self) -> Option<&Toast> {
        self.toast.as_ref()
    }

    /// The newest-first presentation snapshot. Order is preserved.
    pub fn snapshot(&self) -> Vec<NotificationVm> {
        self.entries
            .iter()
            .map(|entry| NotificationVm {
                id: entry.id,
                tone: tone_for(entry.level),
                label: entry.label.clone(),
                message: entry.message.clone(),
                command: entry.command.clone(),
                can_run: entry.can_run,
                state: match entry.state {
                    ActionState::Idle => NotificationActionState::Idle,
                    ActionState::Confirming => NotificationActionState::Confirming,
                    ActionState::Running => NotificationActionState::Running,
                    ActionState::Failed => NotificationActionState::Failed,
                },
            })
            .collect()
    }

    /// Mints the id, replaces the toast and inserts the entry as unread.
    /// Deduplication is the caller's job: only advisories consult `seen`.
    fn admit(
        &mut self,
        level: NotificationLevel,
        label: String,
        message: String,
        command: Option<String>,
        can_run: bool,
    ) {
        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        // A new entry replaces the toast instead of queueing behind it.
        self.toast = Some(Toast {
            id,
            level,
            message: message.clone(),
        });
        self.entries.insert(
            0,
            Notification {
                id,
                level,
                label,
                message,
                command,
                can_run,
                state: ActionState::Idle,
                read: false,
            },
        );
    }
}

/// The arguments after the `unity` token, or `None` when the command cannot be
/// run safely without a shell.
///
/// The first token must name `unity` or `unity.exe` (a full path is allowed)
/// and no token may contain a shell or newline metacharacter.
pub fn remediation_args(command: &str) -> Option<Vec<String>> {
    if command.chars().any(|character| {
        matches!(
            character,
            '|' | '&' | ';' | '<' | '>' | '$' | '`' | '"' | '\'' | '\n' | '\r'
        )
    }) {
        return None;
    }

    let tokens: Vec<String> = command.split_whitespace().map(str::to_string).collect();
    let (program, args) = tokens.split_first()?;

    let file_name = std::path::Path::new(program).file_name()?.to_str()?;
    if !file_name.eq_ignore_ascii_case("unity") && !file_name.eq_ignore_ascii_case("unity.exe") {
        return None;
    }

    Some(args.to_vec())
}

pub fn is_runnable(command: &str) -> bool {
    remediation_args(command).is_some()
}

fn remediation_can_run(remediation: &Remediation) -> bool {
    !remediation.requires_user_approval && is_runnable(&remediation.command)
}

fn advisory_level(kind: AdvisoryKind) -> NotificationLevel {
    match kind {
        AdvisoryKind::Warning => NotificationLevel::Warning,
        AdvisoryKind::Notification => NotificationLevel::Info,
        AdvisoryKind::Error => NotificationLevel::Danger,
    }
}

fn advisory_label(kind: AdvisoryKind) -> &'static str {
    match kind {
        AdvisoryKind::Warning => "Warning",
        AdvisoryKind::Notification => "Notification",
        AdvisoryKind::Error => "Error",
    }
}

fn tone_for(level: NotificationLevel) -> Tone {
    match level {
        NotificationLevel::Info => Tone::Info,
        NotificationLevel::Success => Tone::Success,
        NotificationLevel::Warning => Tone::Warning,
        NotificationLevel::Danger => Tone::Danger,
    }
}

fn error_text(error: &UnityError) -> String {
    let mut message = format!("{}: {}", error.code, error.message);
    if let Some(detail) = &error.detail {
        let detail = detail.trim();
        if !detail.is_empty() {
            message.push_str("\n\n");
            message.push_str(detail);
        }
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn advisory(kind: AdvisoryKind, value: Value) -> Advisory {
        Advisory::from_value(kind, &value).expect("test advisory is valid")
    }

    fn notification_with(code: &str, data: Value) -> Advisory {
        advisory(
            AdvisoryKind::Notification,
            json!({ "code": code, "message": "A notice.", "data": data }),
        )
    }

    #[test]
    fn the_same_code_and_data_is_admitted_once() {
        let mut store = NotificationStore::new();

        assert!(store.push_advisory(&notification_with("CLI_UPDATE", json!({ "latest": "1" }))));
        assert!(!store.push_advisory(&notification_with("CLI_UPDATE", json!({ "latest": "1" }))));
        assert!(store.push_advisory(&notification_with("CLI_UPDATE", json!({ "latest": "2" }))));

        assert_eq!(store.snapshot().len(), 2);
        assert_eq!(store.unread(), 2);
    }

    #[test]
    fn repeated_events_are_admitted_each_time() {
        let mut store = NotificationStore::new();

        assert!(store.push_event(
            NotificationLevel::Success,
            "Job",
            "Job finished successfully."
        ));
        assert!(store.push_event(
            NotificationLevel::Success,
            "Job",
            "Job finished successfully."
        ));

        let snapshot = store.snapshot();
        assert_eq!(
            snapshot.len(),
            2,
            "a second identical job outcome is still news"
        );
        assert_eq!(snapshot[0].message, snapshot[1].message);
        assert_eq!(store.unread(), 2);
        assert_eq!(
            store.toast().map(|toast| toast.message.as_str()),
            Some("Job finished successfully.")
        );
    }

    #[test]
    fn repeated_advisories_collapse_to_one_entry() {
        let mut store = NotificationStore::new();
        let advisory = advisory(
            AdvisoryKind::Warning,
            json!({ "code": "CLI_COMPATIBILITY", "message": "Compatibility mode is on." }),
        );

        assert!(store.push_advisory(&advisory));
        assert!(!store.push_advisory(&advisory));

        assert_eq!(store.snapshot().len(), 1);
        assert_eq!(store.unread(), 1);
    }

    #[test]
    fn the_badge_counts_entries_that_are_still_unread() {
        let mut store = NotificationStore::new();
        store.push_event(NotificationLevel::Success, "Job", "One.");
        store.push_event(NotificationLevel::Success, "Job", "Two.");

        assert_eq!(store.unread(), 2);

        let newest = store.snapshot()[0].id;
        store.dismiss(newest);
        assert_eq!(
            store.unread(),
            1,
            "dismissing an unread entry lowers the badge"
        );

        store.set_rail_open(true);
        assert!(store.rail_open());
        assert_eq!(store.unread(), 0, "opening the rail marks everything read");

        store.push_event(NotificationLevel::Success, "Job", "Three.");
        assert_eq!(
            store.unread(),
            1,
            "an arrival while the rail is open is unread until the next open"
        );

        store.set_rail_open(true);
        assert_eq!(store.unread(), 0);

        store.set_rail_open(false);
        assert!(!store.rail_open());
        assert_eq!(store.unread(), 0);
    }

    #[test]
    fn dismiss_removes_one_entry_and_its_toast_but_clear_removes_all() {
        let mut store = NotificationStore::new();
        store.push_event(NotificationLevel::Info, "Notification", "One.");
        store.push_event(NotificationLevel::Info, "Notification", "Two.");
        let newest = store.snapshot()[0].id;

        store.dismiss(newest);
        assert!(store.toast().is_none());
        assert_eq!(store.snapshot().len(), 1);

        store.clear();
        assert!(store.snapshot().is_empty());
        assert!(store.toast().is_none());
        assert_eq!(store.unread(), 0);
    }

    #[test]
    fn a_new_entry_replaces_the_toast_instead_of_queueing() {
        let mut store = NotificationStore::new();
        store.push_event(NotificationLevel::Info, "Notification", "First.");
        store.push_event(NotificationLevel::Warning, "Warning", "Second.");

        let toast = store.toast().expect("the newest entry is toasted");
        assert_eq!(toast.message, "Second.");
        assert_eq!(toast.level, NotificationLevel::Warning);
        assert_eq!(toast.id, store.snapshot()[0].id);
    }

    #[test]
    fn a_stale_toast_expiry_is_ignored() {
        let mut store = NotificationStore::new();
        store.push_event(NotificationLevel::Info, "Notification", "First.");
        let stale = store.toast().map(|toast| toast.id).expect("toast");
        store.push_event(NotificationLevel::Info, "Notification", "Second.");

        store.toast_expired(stale);
        assert_eq!(
            store.toast().map(|toast| toast.message.as_str()),
            Some("Second.")
        );

        let current = store.toast().map(|toast| toast.id).expect("toast");
        store.toast_expired(current);
        assert!(store.toast().is_none());
    }

    #[test]
    fn approval_required_remediation_is_not_runnable() {
        let mut store = NotificationStore::new();

        store.push_advisory(&advisory(
            AdvisoryKind::Warning,
            json!({
                "code": "CLI_UPDATE",
                "message": "Update the CLI.",
                "remediation": { "command": "unity self-update", "requiresUserApproval": true }
            }),
        ));

        let entry = &store.snapshot()[0];
        assert_eq!(entry.command.as_deref(), Some("unity self-update"));
        assert!(!entry.can_run, "approval is still required");

        let mut store = NotificationStore::new();
        store.push_advisory(&advisory(
            AdvisoryKind::Warning,
            json!({
                "code": "CLI_UPDATE",
                "message": "Update the CLI.",
                "remediation": { "command": "unity self-update", "requiresUserApproval": false }
            }),
        ));
        assert!(store.snapshot()[0].can_run);

        let mut store = NotificationStore::new();
        store.push_advisory(&advisory(
            AdvisoryKind::Warning,
            json!({
                "code": "SHELL",
                "message": "Unsafe command.",
                "remediation": { "command": "unity env && rm -rf /", "requiresUserApproval": false }
            }),
        ));
        assert!(!store.snapshot()[0].can_run);
    }

    #[test]
    fn remediation_args_accepts_a_bare_or_pathed_unity_command() {
        assert_eq!(
            remediation_args("unity self-update"),
            Some(vec!["self-update".to_string()])
        );
        // `Path` treats `/` as a separator on every supported platform.
        assert_eq!(
            remediation_args("/usr/local/bin/unity status"),
            Some(vec!["status".to_string()])
        );
        assert_eq!(
            remediation_args("/usr/local/bin/UNITY status"),
            Some(vec!["status".to_string()])
        );
        // A Windows drive path only parses where `\` separates components.
        #[cfg(windows)]
        assert_eq!(
            remediation_args(r"C:\Tools\unity\unity.exe install lts"),
            Some(vec!["install".to_string(), "lts".to_string()])
        );
        assert_eq!(remediation_args("unity"), Some(Vec::new()));
    }

    #[test]
    fn remediation_args_rejects_shell_metacharacters_and_other_programs() {
        for command in [
            "",
            "   ",
            "curl https://example.com | sh",
            "unity env && rm -rf /",
            "unity status; whoami",
            "unity status > out.txt",
            "unity status < in.txt",
            "unity status $HOME",
            "unity status `whoami`",
            "unity status \"quoted\"",
            "unity status 'quoted'",
            "unity status\nwhoami",
            "unity status\rwhoami",
            "node unity status",
            "unity-tool status",
            "unity.exe.bat status",
            // An unquoted path with a space cannot be split unambiguously.
            r"C:\Program Files\Unity\unity.exe status",
        ] {
            assert!(
                remediation_args(command).is_none(),
                "{command:?} must not be runnable"
            );
            assert!(!is_runnable(command));
        }
    }

    #[test]
    fn push_error_keeps_the_code_detail_and_remediation() {
        let mut store = NotificationStore::new();
        let mut error = UnityError::new("PROJECT_OPEN_IN_EDITOR", "The project is open.")
            .with_detail("Close the Editor and retry.")
            .with_exit_code(Some(6));
        error.extra.insert(
            "remediation".to_string(),
            json!({ "command": "unity projects open /games/alpha", "requiresUserApproval": true }),
        );

        assert!(store.push_error(&error));
        assert!(
            store.push_error(&error),
            "a retried identical failure is a new failure"
        );

        let snapshot = store.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(store.unread(), 2);
        let entry = &snapshot[0];
        assert_eq!(entry.label, "PROJECT_OPEN_IN_EDITOR");
        assert_eq!(entry.tone, Tone::Danger);
        assert!(entry.message.contains("PROJECT_OPEN_IN_EDITOR"));
        assert!(entry.message.contains("The project is open."));
        assert!(entry.message.contains("Close the Editor and retry."));
        assert_eq!(
            entry.command.as_deref(),
            Some("unity projects open /games/alpha")
        );
        assert!(!entry.can_run);
    }

    #[test]
    fn advisories_map_kinds_to_tones_and_default_labels() {
        let mut store = NotificationStore::new();

        store.push_advisory(&advisory(
            AdvisoryKind::Warning,
            json!({ "message": "Compatibility mode is on." }),
        ));
        store.push_advisory(&advisory(
            AdvisoryKind::Notification,
            json!({ "message": "Licence renewed." }),
        ));
        store.push_advisory(&advisory(
            AdvisoryKind::Error,
            json!({ "code": "BROKEN", "message": "Something failed." }),
        ));

        let snapshot = store.snapshot();
        assert_eq!(snapshot[0].tone, Tone::Danger);
        assert_eq!(snapshot[0].label, "BROKEN");
        assert_eq!(snapshot[1].tone, Tone::Info);
        assert_eq!(snapshot[1].label, "Notification");
        assert_eq!(snapshot[2].tone, Tone::Warning);
        assert_eq!(snapshot[2].label, "Warning");
        assert_eq!(snapshot[0].state, NotificationActionState::Idle);
    }

    #[test]
    fn set_state_only_touches_the_named_entry() {
        let mut store = NotificationStore::new();
        store.push_event(NotificationLevel::Info, "Notification", "One.");
        store.push_event(NotificationLevel::Info, "Notification", "Two.");
        let ids: Vec<u64> = store.snapshot().iter().map(|entry| entry.id).collect();

        store.set_state(ids[0], ActionState::Confirming);
        assert_eq!(
            store.snapshot()[0].state,
            NotificationActionState::Confirming
        );
        assert_eq!(store.snapshot()[1].state, NotificationActionState::Idle);
        assert!(store.get(ids[0]).is_some());
        assert!(store.get(u64::MAX).is_none());
    }
}
