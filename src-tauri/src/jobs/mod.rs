use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, watch};
use unity_core::envelope::CliEnvelope;
use unity_core::errors::{code_for_exit_code, message_for_exit_code};
use unity_core::{CommandSpec, UnityError};

pub const JOB_EVENT: &str = "job:update";

#[derive(Clone)]
pub struct JobManager {
    inner: Arc<Mutex<HashMap<String, JobRecord>>>,
    counter: Arc<AtomicU64>,
}

struct JobRecord {
    cancel: watch::Sender<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStarted {
    pub job_id: String,
    pub kind: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobUpdate {
    pub job_id: String,
    pub kind: String,
    /// started | running | succeeded | failed | cancelled
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<UnityError>,
    pub at: u64,
}

enum StreamLine {
    Stdout(String),
    Stderr(String),
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(
        &self,
        app: AppHandle,
        kind: &str,
        spec: &CommandSpec,
    ) -> Result<JobStarted, UnityError> {
        let job_id = self.next_id();
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
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

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        {
            let mut jobs = self.inner.lock().expect("job registry lock poisoned");
            jobs.insert(job_id.clone(), JobRecord { cancel: cancel_tx });
        }

        let (line_tx, mut line_rx) = mpsc::unbounded_channel::<StreamLine>();
        if let Some(stdout) = stdout {
            let tx = line_tx.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx.send(StreamLine::Stdout(line)).is_err() {
                        break;
                    }
                }
            });
        }
        if let Some(stderr) = stderr {
            let tx = line_tx.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx.send(StreamLine::Stderr(line)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(line_tx);

        let started = JobStarted {
            job_id: job_id.clone(),
            kind: kind.to_string(),
            command: spec.display(),
        };
        let manager = self.clone();
        let task_app = app.clone();
        let task_id = job_id.clone();
        let task_kind = kind.to_string();
        tauri::async_runtime::spawn(async move {
            run_job(
                task_app,
                manager,
                task_id,
                task_kind,
                child,
                cancel_rx,
                &mut line_rx,
            )
            .await;
        });

        Ok(started)
    }

    pub fn cancel(&self, job_id: &str) -> Result<(), UnityError> {
        let jobs = self.inner.lock().expect("job registry lock poisoned");
        let record = jobs
            .get(job_id)
            .ok_or_else(|| UnityError::new("JOB_NOT_FOUND", "That job is no longer running."))?;
        let _ = record.cancel.send(true);
        Ok(())
    }

    pub fn cancel_all(&self) {
        let jobs = self.inner.lock().expect("job registry lock poisoned");
        for record in jobs.values() {
            let _ = record.cancel.send(true);
        }
    }

    fn remove(&self, job_id: &str) {
        let mut jobs = self.inner.lock().expect("job registry lock poisoned");
        jobs.remove(job_id);
    }

    fn next_id(&self) -> String {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("job-{}-{counter}", now_ms())
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

async fn run_job(
    app: AppHandle,
    manager: JobManager,
    job_id: String,
    kind: String,
    mut child: Child,
    mut cancel_rx: watch::Receiver<bool>,
    line_rx: &mut mpsc::UnboundedReceiver<StreamLine>,
) {
    let mut last_result: Option<Value> = None;
    let mut status: Option<std::process::ExitStatus> = None;
    let mut channels_open = true;
    let mut cancel_open = true;
    let mut kill_requested = false;
    let mut ticker = tokio::time::interval(Duration::from_millis(120));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    emit(
        &app,
        JobUpdate {
            job_id: job_id.clone(),
            kind: kind.clone(),
            state: "started".to_string(),
            stream: None,
            line: None,
            progress: Some(0.0),
            message: None,
            result: None,
            error: None,
            at: now_ms(),
        },
    );

    loop {
        tokio::select! {
            _ = ticker.tick(), if status.is_none() => {
                match child.try_wait() {
                    Ok(Some(exit)) => status = Some(exit),
                    Ok(None) => {}
                    Err(err) => {
                        status = None;
                        let _ = err;
                    }
                }
            }
            changed = cancel_rx.changed(), if cancel_open => {
                match changed {
                    Ok(()) => {
                        if *cancel_rx.borrow() && !kill_requested {
                            kill_requested = true;
                            let _ = child.start_kill();
                        }
                    }
                    Err(_) => cancel_open = false,
                }
            }
            line = line_rx.recv(), if channels_open => {
                match line {
                    Some(StreamLine::Stdout(line)) => {
                        let update = stdout_update(&job_id, &kind, &line, &mut last_result);
                        emit(&app, update);
                    }
                    Some(StreamLine::Stderr(line)) => {
                        emit(
                            &app,
                            JobUpdate {
                                job_id: job_id.clone(),
                                kind: kind.clone(),
                                state: "running".to_string(),
                                stream: Some("stderr".to_string()),
                                line: Some(line),
                                progress: None,
                                message: None,
                                result: None,
                                error: None,
                                at: now_ms(),
                            },
                        );
                    }
                    None => channels_open = false,
                }
            }
        }

        if status.is_some() && !channels_open {
            break;
        }
    }

    let exit_code = status.and_then(|status| status.code());
    let process_succeeded = status.map(|status| status.success()).unwrap_or(false);
    let result_succeeded = last_result
        .as_ref()
        .and_then(|result| result.get("success"))
        .and_then(Value::as_bool);
    let cancelled = *cancel_rx.borrow() && !process_succeeded;
    let succeeded = process_succeeded && result_succeeded != Some(false);

    let error = if cancelled {
        Some(UnityError::new("CANCELLED", "The job was cancelled.").with_exit_code(exit_code))
    } else if !succeeded {
        Some(job_error(last_result.as_ref(), exit_code))
    } else {
        None
    };

    emit(
        &app,
        JobUpdate {
            job_id: job_id.clone(),
            kind,
            state: if cancelled {
                "cancelled".to_string()
            } else if succeeded {
                "succeeded".to_string()
            } else {
                "failed".to_string()
            },
            stream: None,
            line: None,
            progress: if succeeded { Some(100.0) } else { None },
            message: None,
            result: last_result,
            error,
            at: now_ms(),
        },
    );
    manager.remove(&job_id);
}

fn stdout_update(
    job_id: &str,
    kind: &str,
    line: &str,
    last_result: &mut Option<Value>,
) -> JobUpdate {
    let parsed = serde_json::from_str::<Value>(line).ok();
    let mut progress = None;
    let mut message = None;
    let mut result = None;

    if let Some(Value::Object(object)) = &parsed {
        progress = progress_from(object);
        message = message_from(object);
        let is_result = object.get("type").and_then(Value::as_str) == Some("result")
            || (object.get("success").is_some()
                && (object.contains_key("errors") || object.contains_key("data")));
        if is_result {
            let value = Value::Object(object.clone());
            *last_result = Some(value.clone());
            result = Some(value);
            progress = Some(progress.unwrap_or(100.0));
        }
    }

    JobUpdate {
        job_id: job_id.to_string(),
        kind: kind.to_string(),
        state: "running".to_string(),
        stream: Some("stdout".to_string()),
        line: Some(line.to_string()),
        progress,
        message,
        result,
        error: None,
        at: now_ms(),
    }
}

fn progress_from(object: &serde_json::Map<String, Value>) -> Option<f64> {
    if let Some(value) = number(object, "progress") {
        return Some(clamp_percent(if value <= 1.0 {
            value * 100.0
        } else {
            value
        }));
    }
    if let Some(value) = number(object, "percent").or_else(|| number(object, "percentage")) {
        return Some(clamp_percent(value));
    }
    let byte_pairs = [
        ("downloadedBytes", "totalBytes"),
        ("bytesDownloaded", "totalBytes"),
        ("downloaded", "total"),
        ("current", "total"),
        ("current", "totalItems"),
    ];
    for (done_key, total_key) in byte_pairs {
        if let (Some(done), Some(total)) = (number(object, done_key), number(object, total_key)) {
            if total > 0.0 {
                return Some(clamp_percent(done / total * 100.0));
            }
        }
    }
    None
}

fn number(object: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    object.get(key).and_then(Value::as_f64)
}

fn clamp_percent(value: f64) -> f64 {
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

fn job_error(result: Option<&Value>, exit_code: Option<i32>) -> UnityError {
    if let Some(result) = result {
        if let Ok(envelope) = serde_json::from_value::<CliEnvelope<Value>>(result.clone()) {
            if let Some(error) = envelope.errors.first() {
                return error.to_unity_error(exit_code, "");
            }
        }
    }
    UnityError::new(
        exit_code
            .map(code_for_exit_code)
            .unwrap_or("JOB_FAILED")
            .to_string(),
        exit_code
            .map(message_for_exit_code)
            .unwrap_or("The job ended without an exit status.")
            .to_string(),
    )
    .with_exit_code(exit_code)
}

fn emit(app: &AppHandle, update: JobUpdate) {
    let _ = app.emit(JOB_EVENT, update);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
