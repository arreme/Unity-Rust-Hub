use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use unity_core::advisory::{Advisory, AdvisoryKind};
use unity_core::envelope::CliEnvelope;
use unity_core::errors::{code_for_exit_code, message_for_exit_code};
use unity_core::{CommandSpec, UnityError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn label(self) -> &'static str {
        match self {
            JobState::Running => "running",
            JobState::Succeeded => "succeeded",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub struct JobEvent {
    pub job_id: String,
    pub state: JobState,
    pub stream: Option<StreamKind>,
    pub line: Option<String>,
    pub progress: Option<f32>,
    pub message: Option<String>,
    pub result: Option<Value>,
    pub advisories: Vec<Advisory>,
    pub error: Option<UnityError>,
}

#[derive(Debug, Clone)]
pub struct JobStarted {
    pub id: String,
    pub kind: String,
    pub command: String,
}

#[derive(Clone)]
pub struct JobManager {
    inner: Arc<Mutex<Inner>>,
    counter: Arc<AtomicU64>,
}

impl Hash for JobManager {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

struct Inner {
    events: VecDeque<JobEvent>,
    handles: HashMap<String, Handle>,
    notifier: Option<mpsc::SyncSender<()>>,
}

const MAX_QUEUED_EVENTS: usize = 2048;

struct Handle {
    child: Arc<Mutex<Child>>,
    cancelled: Arc<AtomicBool>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                events: VecDeque::new(),
                handles: HashMap::new(),
                notifier: None,
            })),
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(&self, kind: &str, spec: CommandSpec) -> Result<JobStarted, UnityError> {
        let id = format!(
            "job-{}-{}",
            now_millis(),
            self.counter.fetch_add(1, Ordering::Relaxed)
        );

        let mut command = unity_core::process::hidden(&spec.program);
        command
            .args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &spec.env {
            command.env(key, value);
        }
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }

        let mut child = command.spawn().map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                UnityError::new(
                    "CLI_NOT_FOUND",
                    format!("Could not start `{}`.", spec.program.display()),
                )
                .with_detail(err.to_string())
            } else {
                UnityError::new("SPAWN_FAILED", "The command could not be started.")
                    .with_detail(format!("{}: {err}", spec.program.display()))
            }
        })?;

        unity_core::trace::record("job", &spec.display(), Duration::ZERO, None);

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child = Arc::new(Mutex::new(child));
        let cancelled = Arc::new(AtomicBool::new(false));

        {
            let mut inner = self.inner.lock().expect("job registry lock poisoned");
            inner.handles.insert(
                id.clone(),
                Handle {
                    child: child.clone(),
                    cancelled: cancelled.clone(),
                },
            );
        }

        let manager = self.clone();
        let task_id = id.clone();

        std::thread::spawn(move || {
            let stderr_thread = stderr.map(|stderr| {
                let manager = manager.clone();
                let id = task_id.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stderr);
                    let mut line = String::new();
                    while reader.read_line(&mut line).unwrap_or(0) > 0 {
                        let text = line.trim_end().to_string();
                        manager.push(JobEvent {
                            job_id: id.clone(),
                            state: JobState::Running,
                            stream: Some(StreamKind::Stderr),
                            line: Some(text),
                            progress: None,
                            message: None,
                            result: None,
                            advisories: Vec::new(),
                            error: None,
                        });
                        line.clear();
                    }
                })
            });

            let mut stream = JobStream::default();

            if let Some(stdout) = stdout {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 {
                    let text = line.trim_end().to_string();
                    let mut parsed = parse_stdout(&text);
                    let advisories = stream.mid_job_advisories(&mut parsed);
                    manager.push(JobEvent {
                        job_id: task_id.clone(),
                        state: JobState::Running,
                        stream: Some(StreamKind::Stdout),
                        line: Some(text),
                        progress: parsed.progress,
                        message: parsed.message,
                        result: parsed.result,
                        advisories,
                        error: None,
                    });
                    line.clear();
                }
            }

            if let Some(thread) = stderr_thread {
                let _ = thread.join();
            }

            let last_result = stream.last_result;
            let last_advisories = stream.last_advisories;

            let status = wait_for_exit(&child);
            let exit_code = status.and_then(|status| status.code());
            let process_succeeded = status.map(|status| status.success()).unwrap_or(false);
            let was_cancelled = cancelled.load(Ordering::SeqCst) && !process_succeeded;
            let succeeded = job_succeeded(process_succeeded, last_result.as_ref());

            let error = if was_cancelled {
                Some(
                    UnityError::new("CANCELLED", "The job was cancelled.")
                        .with_exit_code(exit_code),
                )
            } else if !succeeded {
                Some(job_error(last_result.as_ref(), exit_code))
            } else {
                None
            };

            manager.push(JobEvent {
                job_id: task_id.clone(),
                state: if was_cancelled {
                    JobState::Cancelled
                } else if succeeded {
                    JobState::Succeeded
                } else {
                    JobState::Failed
                },
                stream: None,
                line: None,
                progress: if succeeded { Some(100.0) } else { None },
                message: None,
                result: last_result,
                advisories: last_advisories,
                error,
            });

            manager.remove(&task_id);
        });

        Ok(JobStarted {
            id,
            kind: kind.to_string(),
            command: spec.display(),
        })
    }

    /// Registers the subscription's wake-up channel. iced calls this when the
    /// job subscription starts, so the UI only wakes when a job actually emits
    /// an event instead of on a fixed timer.
    pub fn set_notifier(&self, notifier: mpsc::SyncSender<()>) {
        let mut inner = self.inner.lock().expect("job registry lock poisoned");
        inner.notifier = Some(notifier);
    }

    pub fn poll(&self) -> Vec<JobEvent> {
        let mut inner = self.inner.lock().expect("job registry lock poisoned");
        inner.events.drain(..).collect()
    }

    pub fn cancel(&self, job_id: &str) -> bool {
        let handle = {
            let inner = self.inner.lock().expect("job registry lock poisoned");
            inner
                .handles
                .get(job_id)
                .map(|handle| (handle.child.clone(), handle.cancelled.clone()))
        };

        let Some((child, cancelled)) = handle else {
            return false;
        };

        cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut child) = child.lock() {
            let _ = child.kill();
        }
        true
    }

    fn push(&self, event: JobEvent) {
        let mut inner = self.inner.lock().expect("job registry lock poisoned");
        inner.events.push_back(event);
        while inner.events.len() > MAX_QUEUED_EVENTS {
            inner.events.pop_front();
        }
        if let Some(notifier) = &inner.notifier {
            // Capacity-one channel: duplicate wake-ups are intentionally
            // dropped; one wake-up drains the whole batch.
            let _ = notifier.try_send(());
        }
    }

    fn remove(&self, job_id: &str) {
        let mut inner = self.inner.lock().expect("job registry lock poisoned");
        inner.handles.remove(job_id);
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

fn wait_for_exit(child: &Arc<Mutex<Child>>) -> Option<std::process::ExitStatus> {
    loop {
        {
            let mut child = child.lock().ok()?;
            match child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) => {}
                Err(_) => return None,
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Accumulates what the terminal event needs from the stdout frames.
#[derive(Default)]
struct JobStream {
    last_result: Option<Value>,
    last_advisories: Vec<Advisory>,
}

impl JobStream {
    /// Takes the advisories the mid-job event for this line should carry.
    ///
    /// A notice frame keeps its advisories so the user hears about it while the
    /// job runs. A result frame instead hands its advisories to the terminal
    /// event, so they are delivered once instead of being repeated on the
    /// running event for the same frame.
    fn mid_job_advisories(&mut self, parsed: &mut ParsedLine) -> Vec<Advisory> {
        let Some(result) = parsed.result.as_ref() else {
            return std::mem::take(&mut parsed.advisories);
        };
        self.last_result = Some(result.clone());
        self.last_advisories = std::mem::take(&mut parsed.advisories);
        Vec::new()
    }
}

struct ParsedLine {
    progress: Option<f32>,
    message: Option<String>,
    result: Option<Value>,
    advisories: Vec<Advisory>,
}

impl ParsedLine {
    fn empty() -> Self {
        Self {
            progress: None,
            message: None,
            result: None,
            advisories: Vec::new(),
        }
    }
}

fn parse_stdout(line: &str) -> ParsedLine {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return ParsedLine::empty();
    };

    let Value::Object(object) = &value else {
        return ParsedLine::empty();
    };

    let is_result = object.get("type").and_then(Value::as_str) == Some("result")
        || (object.get("success").is_some()
            && (object.contains_key("errors") || object.contains_key("data")));

    let advisories = if is_result {
        result_advisories(line)
    } else {
        notice_advisory(&value).into_iter().collect()
    };

    ParsedLine {
        progress: progress_from(object).map(|value| value as f32),
        message: message_from(object),
        result: is_result.then_some(value),
        advisories,
    }
}

/// The advisories a result frame carries in its `warnings` and `notifications`.
fn result_advisories(line: &str) -> Vec<Advisory> {
    if let Ok(envelope) = serde_json::from_str::<CliEnvelope<Value>>(line) {
        envelope.advisories()
    } else {
        Vec::new()
    }
}

/// A mid-job `warning`, `notice` or `notification` frame as one advisory.
fn notice_advisory(value: &Value) -> Option<Advisory> {
    let object = value.as_object()?;
    let kind = match object.get("type").and_then(Value::as_str)? {
        "warning" => AdvisoryKind::Warning,
        "notice" | "notification" => AdvisoryKind::Notification,
        _ => return None,
    };
    Advisory::from_value(kind, value)
}

fn progress_from(object: &serde_json::Map<String, Value>) -> Option<f64> {
    if let Some(value) = number(object, "progress") {
        return Some(clamp(if value <= 1.0 { value * 100.0 } else { value }));
    }
    if let Some(value) = number(object, "percent").or_else(|| number(object, "percentage")) {
        return Some(clamp(value));
    }

    for (done_key, total_key) in [
        ("downloadedBytes", "totalBytes"),
        ("bytesDownloaded", "totalBytes"),
        ("downloaded", "total"),
        ("current", "total"),
        ("current", "totalItems"),
    ] {
        if let (Some(done), Some(total)) = (number(object, done_key), number(object, total_key)) {
            if total > 0.0 {
                return Some(clamp(done / total * 100.0));
            }
        }
    }

    None
}

fn number(object: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    object.get(key).and_then(Value::as_f64)
}

fn clamp(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn message_from(object: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["message", "status", "stage", "phase", "label", "name"] {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            if !value.trim().is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn job_succeeded(process_succeeded: bool, last_result: Option<&Value>) -> bool {
    // `install-modules` exits 0 with no result frame when every named module
    // is already installed. An exit-0 run is success unless a result frame
    // explicitly says otherwise.
    process_succeeded
        && last_result
            .and_then(|result| result.get("success"))
            .and_then(Value::as_bool)
            != Some(false)
}

fn job_error(result: Option<&Value>, exit_code: Option<i32>) -> UnityError {
    if let Some(result) = result {
        if let Ok(envelope) = serde_json::from_value::<CliEnvelope<Value>>(result.clone()) {
            if let Some(error) = envelope.errors.first() {
                return error.to_unity_error(exit_code, "");
            }
        }
    }

    UnityError::new(
        exit_code.map(code_for_exit_code).unwrap_or("JOB_FAILED"),
        exit_code
            .map(message_for_exit_code)
            .unwrap_or("The job ended without an exit status."),
    )
    .with_exit_code(exit_code)
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(state: JobState) -> JobEvent {
        JobEvent {
            job_id: "job-test".to_string(),
            state,
            stream: None,
            line: None,
            progress: None,
            message: None,
            result: None,
            advisories: Vec::new(),
            error: None,
        }
    }

    #[test]
    fn event_queue_is_bounded_and_keeps_the_terminal_event() {
        let manager = JobManager::new();
        for _ in 0..MAX_QUEUED_EVENTS + 10 {
            manager.push(event(JobState::Running));
        }
        manager.push(event(JobState::Succeeded));

        let events = manager.poll();
        assert_eq!(events.len(), MAX_QUEUED_EVENTS);
        assert_eq!(
            events.last().map(|event| event.state),
            Some(JobState::Succeeded)
        );
    }

    #[test]
    fn pushing_an_event_wakes_the_subscription_once() {
        let manager = JobManager::new();
        let (sender, receiver) = mpsc::sync_channel(1);
        manager.set_notifier(sender);
        manager.push(event(JobState::Running));
        manager.push(event(JobState::Running));

        assert!(receiver.try_recv().is_ok());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn exit_zero_without_result_frames_is_success() {
        // The CLI exits 0 (and may print only the human "already installed"
        // line) when every named module is installed.
        assert!(job_succeeded(true, None));
        assert!(!job_succeeded(false, None));
    }

    #[test]
    fn result_frame_can_veto_an_exit_zero() {
        let failure = serde_json::json!({
            "type": "result",
            "success": false,
            "errors": [{ "code": "PROJECT_OPEN_IN_EDITOR", "message": "Project is open." }]
        });
        assert!(!job_succeeded(true, Some(&failure)));

        let success = serde_json::json!({ "type": "result", "success": true });
        assert!(job_succeeded(true, Some(&success)));
    }

    #[test]
    fn job_error_prefers_the_cli_error_frame() {
        let result = serde_json::json!({
            "type": "result",
            "success": false,
            "errors": [{
                "code": "PROJECT_OPEN_IN_EDITOR",
                "message": "The project is already open in Unity Editor (pid 4242).",
                "detail": "Close that Editor and retry."
            }]
        });

        let error = job_error(Some(&result), Some(6));

        assert_eq!(error.code, "PROJECT_OPEN_IN_EDITOR");
        assert_eq!(
            error.message,
            "The project is already open in Unity Editor (pid 4242)."
        );
        assert_eq!(
            error.detail.as_deref(),
            Some("Close that Editor and retry.")
        );
        assert_eq!(error.exit_code, Some(6));
    }

    #[test]
    fn job_error_falls_back_to_exit_code_semantics() {
        let error = job_error(None, Some(6));

        assert_eq!(error.code, "COMMAND_FAILED");
        assert_eq!(error.message, "The Unity CLI command failed.");
        assert_eq!(error.exit_code, Some(6));
    }

    #[test]
    fn a_warning_frame_carries_one_advisory_on_its_own_event() {
        let mut parsed = parse_stdout(
            r#"{"type":"warning","code":"CLI_COMPATIBILITY","message":"Compatibility mode is on."}"#,
        );
        let mut stream = JobStream::default();

        let advisories = stream.mid_job_advisories(&mut parsed);

        assert!(parsed.result.is_none());
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].kind, AdvisoryKind::Warning);
        assert_eq!(advisories[0].code.as_deref(), Some("CLI_COMPATIBILITY"));
        assert_eq!(advisories[0].message, "Compatibility mode is on.");
        assert!(stream.last_advisories.is_empty());
    }

    #[test]
    fn a_progress_only_stream_carries_no_advisories() {
        let mut stream = JobStream::default();

        for line in [
            r#"{"progress":0.1,"message":"Downloading."}"#,
            r#"{"percent":50,"stage":"Verifying"}"#,
            r#"{"current":2,"total":4}"#,
            r#"[{"version":"6000.6.6f1"}]"#,
            "not json at all",
        ] {
            let mut parsed = parse_stdout(line);
            assert!(
                stream.mid_job_advisories(&mut parsed).is_empty(),
                "{line} should carry no advisories"
            );
        }

        assert!(stream.last_result.is_none());
        assert!(stream.last_advisories.is_empty());
    }

    #[test]
    fn a_result_frame_delivers_its_notifications_once_on_the_terminal_event() {
        let mut stream = JobStream::default();

        for _ in 0..3 {
            let mut progress = parse_stdout(r#"{"progress":0.25,"message":"Installing."}"#);
            assert!(stream.mid_job_advisories(&mut progress).is_empty());
        }

        let mut result = parse_stdout(
            r#"{
                "type": "result",
                "success": true,
                "notifications": [{
                    "code": "CLI_UPDATE_AVAILABLE",
                    "message": "A new version of the Unity CLI is available."
                }]
            }"#,
        );
        assert!(
            stream.mid_job_advisories(&mut result).is_empty(),
            "the running event for a result frame carries no advisories"
        );

        let terminal = JobEvent {
            advisories: stream.last_advisories,
            ..event(JobState::Succeeded)
        };

        assert_eq!(terminal.advisories.len(), 1);
        assert_eq!(terminal.advisories[0].kind, AdvisoryKind::Notification);
        assert_eq!(
            terminal.advisories[0].code.as_deref(),
            Some("CLI_UPDATE_AVAILABLE")
        );
    }

    #[test]
    fn only_the_last_result_frame_supplies_the_terminal_advisories() {
        let mut stream = JobStream::default();

        for line in [
            r#"{"type":"result","success":true,"warnings":["First warning."]}"#,
            r#"{"type":"result","success":true,"warnings":["Second warning."]}"#,
        ] {
            let mut parsed = parse_stdout(line);
            assert!(stream.mid_job_advisories(&mut parsed).is_empty());
        }

        assert_eq!(stream.last_advisories.len(), 1);
        assert_eq!(stream.last_advisories[0].message, "Second warning.");
    }

    #[test]
    fn notice_and_notification_frames_map_to_notification_advisories() {
        for line in [
            r#"{"type":"notice","message":"Licence renewed."}"#,
            r#"{"type":"notification","message":"Update available."}"#,
        ] {
            let mut parsed = parse_stdout(line);
            let advisories = JobStream::default().mid_job_advisories(&mut parsed);

            assert_eq!(advisories.len(), 1, "{line} should carry one advisory");
            assert_eq!(advisories[0].kind, AdvisoryKind::Notification);
            assert!(advisories[0].code.is_none());
        }
    }
}
