use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::envelope::{parse_data, parse_envelope, parse_items, ValueEnvelope};
use crate::errors::{message_for_exit_code, UnityError};
use crate::models::{
    AuthStatus, CliStatus, CliUpdateInfo, CliVersion, CreateProjectOptions, Editor, EditorInfo,
    InstallPath, LicenseStatus, Module, PipelineEditors, Project, RawChangelog, Release,
    RunningEditor, RunningEditors, StatusInstance, TemplateBrief, TemplateCreateOptions,
    TemplateFilter, TemplateInfo, TemplatePackOptions,
};
use crate::versions::{
    validate_archive_path, validate_cli_version, validate_editor_version, validate_module_id,
    validate_output_dir, validate_package_id, validate_project_name, validate_project_path,
    validate_release_stream, validate_template_id, validate_template_version,
};

/// The standard opt-out signal. The CLI's analytics/update callback adds a
/// noticeable startup stall to most invocations, so every call made by the GUI
/// opts out. Users can still use the CLI directly with their own preference.
pub const DO_NOT_TRACK_ENV: &str = "DO_NOT_TRACK";

/// Reads are expected to finish quickly. A hung CLI process must not hold the
/// single reader worker forever.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// A fully-resolved command line. Arguments are always passed as separate
/// values; `display()` is for showing the user what will run, never for
/// executing it.
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
}

impl CommandSpec {
    pub fn new(program: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            env: Vec::new(),
            cwd: None,
        }
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// A copy-paste representation for the consent dialog and log pane.
    pub fn display(&self) -> String {
        let mut pieces: Vec<String> = self
            .env
            .iter()
            .map(|(key, value)| format!("{key}={}", shell_quote(value)))
            .collect();
        pieces.push(shell_quote(&self.program.to_string_lossy()));
        pieces.extend(self.args.iter().map(|arg| shell_quote(arg)));
        pieces.join(" ")
    }
}

impl UnityCli {
    /// Builds a read-only invocation with the machine flags every call needs.
    pub fn spec(&self, parts: &[String], format: &str) -> Result<CommandSpec, UnityError> {
        let mut args = parts.to_vec();
        args.extend(
            [
                "--format",
                format,
                "--no-pager",
                "--no-banner",
                "--non-interactive",
            ]
            .into_iter()
            .map(str::to_string),
        );
        Ok(CommandSpec::new(self.resolve()?, args).with_env(DO_NOT_TRACK_ENV, "1"))
    }

    pub fn run_args(&self, parts: &[String], format: &str) -> Result<Executed, UnityError> {
        let spec = self.spec(parts, format)?;
        self.execute(&spec)
    }

    pub fn execute(&self, spec: &CommandSpec) -> Result<Executed, UnityError> {
        self.execute_inner(spec, DEFAULT_READ_TIMEOUT, true)
    }

    /// Runs a read command without turning `success: false` into an error.
    /// Some CLI commands (notably `auth status` when signed out) return useful
    /// data in a failed envelope; callers need a chance to parse it.
    pub fn execute_envelope(&self, spec: &CommandSpec) -> Result<Executed, UnityError> {
        self.execute_inner(spec, DEFAULT_READ_TIMEOUT, false)
    }

    #[doc(hidden)]
    pub fn execute_with_timeout(
        &self,
        spec: &CommandSpec,
        timeout: Duration,
    ) -> Result<Executed, UnityError> {
        self.execute_inner(spec, timeout, true)
    }

    fn execute_inner(
        &self,
        spec: &CommandSpec,
        timeout: Duration,
        require_success: bool,
    ) -> Result<Executed, UnityError> {
        let started = Instant::now();
        let output = match run_command(spec, timeout) {
            Ok(output) => output,
            Err(error) => {
                crate::trace::record("read", &spec.display(), started.elapsed(), None);
                return Err(error);
            }
        };
        crate::trace::record(
            "read",
            &spec.display(),
            started.elapsed(),
            output.status.code(),
        );

        let stdout = match String::from_utf8(output.stdout) {
            Ok(stdout) => stdout,
            Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
        };
        let stderr = match String::from_utf8(output.stderr) {
            Ok(stderr) => stderr,
            Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
        };
        let exit_code = output.status.code();

        let envelope = parse_envelope(&stdout).map_err(|mut err| {
            err.detail = Some(match err.detail {
                Some(detail) if !stderr.trim().is_empty() => {
                    format!("{detail}\n\nstderr:\n{}", stderr.trim())
                }
                Some(detail) => detail,
                None if !stderr.trim().is_empty() => {
                    format!("stderr:\n{}", stderr.trim())
                }
                None => "The CLI returned no parseable JSON.".to_string(),
            });
            err.exit_code = exit_code;
            err
        })?;

        if require_success && !envelope.success {
            return Err(envelope_error(&envelope, &stderr, exit_code));
        }

        Ok(Executed {
            envelope,
            stderr,
            exit_code,
        })
    }

    pub fn version(&self) -> Result<CliVersion, UnityError> {
        let executed = self.run_args(&svec(&["version"]), "json")?;
        parse_data(executed.envelope)
    }

    /// Combines resolution, version and install-path into the state the UI
    /// needs. A missing CLI is reported in `error` instead of failing the
    /// whole status read, so the app can still offer an install action.
    pub fn status(&self) -> CliStatus {
        let location = match self.locate() {
            Ok(location) => location,
            Err(error) => {
                return CliStatus {
                    installed: false,
                    error: Some(error),
                    ..CliStatus::default()
                };
            }
        };

        let mut status = CliStatus {
            installed: true,
            path: Some(location.path.display().to_string()),
            path_source: Some(location.source.to_string()),
            ..CliStatus::default()
        };

        match self.version() {
            Ok(version) => status.version = Some(version),
            Err(error) => status.error = Some(error),
        }
        if let Ok(path) = self.install_path() {
            status.install_path = Some(path);
        }
        status
    }

    pub fn install_path(&self) -> Result<String, UnityError> {
        let executed = self.run_args(&svec(&["install-path"]), "json")?;
        let value: InstallPath = parse_data(executed.envelope)?;
        Ok(value.path)
    }

    pub fn editors_installed(&self) -> Result<Vec<Editor>, UnityError> {
        let parts = svec(&["editors", "--installed", "--long"]);
        let executed = self.run_args(&parts, "json")?;
        parse_items(executed.envelope.data, &[])
    }

    pub fn editors_running(&self) -> Result<RunningEditors, UnityError> {
        let mut running = self.editors_running_base()?;
        // Pipeline status is optional. An older CLI, a stopped Pipeline
        // server, or an unexpected shape must not break the base list.
        if let Ok(pipeline) = self.pipeline_editors() {
            merge_pipeline_editors(&mut running, pipeline);
        }
        // `unity status` adds state, frame count, and player-loop ticking. It
        // exits 12 with STATUS_NO_INSTANCES when nothing is connected, which is
        // a normal empty result rather than a failed read.
        if let Ok(status) = self.status_instances() {
            merge_status_instances(&mut running, status);
        }
        Ok(running)
    }

    fn editors_running_base(&self) -> Result<RunningEditors, UnityError> {
        let parts = svec(&["editors", "running"]);
        let executed = self.run_args(&parts, "json")?;
        let Some(data) = executed.envelope.data else {
            return Ok(RunningEditors::default());
        };
        if data.is_array() {
            let instances: Vec<RunningEditor> = parse_items(Some(data), &[])?;
            return Ok(RunningEditors {
                count: instances.len(),
                instances,
            });
        }
        let running: RunningEditors = serde_json::from_value(data).map_err(|err| {
            UnityError::unexpected_output(format!("Could not parse running Editors: {err}"))
        })?;
        Ok(running)
    }

    pub fn pipeline_editors(&self) -> Result<PipelineEditors, UnityError> {
        let executed = self.run_args(&svec(&["pipeline", "editors"]), "json")?;
        pipeline_editors_from_data(executed.envelope.data)
    }

    pub fn releases(
        &self,
        limit: u32,
        offset: u32,
        stream: Option<&str>,
    ) -> Result<Vec<Release>, UnityError> {
        let parts = releases_parts(limit, offset, stream)?;
        let executed = self.run_args(&parts, "json")?;
        parse_items(executed.envelope.data, &[])
    }

    pub fn modules(&self, version: &str) -> Result<Vec<Module>, UnityError> {
        let parts = modules_parts(version)?;
        let executed = self.run_args(&parts, "json")?;
        parse_items(executed.envelope.data, &[])
    }

    pub fn editor_info(&self, version: &str) -> Result<EditorInfo, UnityError> {
        validate_editor_version(version)?;
        let parts = svec(&["editors", "info", version]);
        let executed = self.run_args(&parts, "json")?;
        parse_data(executed.envelope)
    }

    pub fn projects(&self) -> Result<Vec<Project>, UnityError> {
        let executed = self.run_args(&projects_parts(), "json")?;
        parse_items(executed.envelope.data, &[])
    }

    /// Lists templates for one Editor version.
    pub fn templates(
        &self,
        editor_version: &str,
        filter: TemplateFilter,
        installed_only: bool,
    ) -> Result<Vec<TemplateBrief>, UnityError> {
        let parts = templates_parts(editor_version, filter, installed_only)?;
        let executed = self.run_args(&parts, "json")?;
        parse_items(executed.envelope.data, &[])
    }

    pub fn template_info(
        &self,
        editor_version: &str,
        name: &str,
    ) -> Result<TemplateInfo, UnityError> {
        validate_editor_version(editor_version)?;
        validate_template_id(name)?;
        let parts = svec(&[
            "templates",
            "info",
            name,
            "--editor-version",
            editor_version,
        ]);
        let executed = self.run_args(&parts, "json")?;
        parse_data(executed.envelope)
    }

    /// `unity templates create` installs a registered template and streams
    /// NDJSON progress.
    pub fn templates_create_spec(
        &self,
        options: &TemplateCreateOptions,
    ) -> Result<CommandSpec, UnityError> {
        let parts = templates_create_parts(options)?;
        self.spec(&parts, "ndjson")
    }

    /// `unity templates pack` writes a portable `.tgz` and registers nothing.
    pub fn templates_pack_spec(
        &self,
        options: &TemplatePackOptions,
    ) -> Result<CommandSpec, UnityError> {
        let parts = templates_pack_parts(options)?;
        self.spec(&parts, "ndjson")
    }

    /// `unity projects create`, always local-only (`--no-cloud`).
    pub fn create_project_spec(
        &self,
        options: &CreateProjectOptions,
    ) -> Result<CommandSpec, UnityError> {
        let parts = create_project_parts(options)?;
        self.spec(&parts, "ndjson")
    }

    /// `unity status`: one row per connected Editor. Exit 12 with
    /// `STATUS_NO_INSTANCES` is the normal "nothing running" result.
    pub fn status_instances(&self) -> Result<Vec<StatusInstance>, UnityError> {
        let spec = self.spec(&svec(&["status"]), "json")?;
        let executed = self.execute_envelope(&spec)?;
        status_instances_from_envelope(&executed.envelope, &executed.stderr, executed.exit_code)
    }

    /// Opens a project with its registered Editor version. The command returns
    /// as soon as the Editor has been launched; it does not wait for it to exit.
    pub fn open_project(&self, path: &str) -> Result<(), UnityError> {
        let parts = open_project_parts(path)?;
        self.run_args(&parts, "json")?;
        Ok(())
    }

    /// Registers an existing Unity project in the Hub registry.
    pub fn add_project(&self, path: &str) -> Result<(), UnityError> {
        let project = validate_project_path(path)?;
        let project = project.to_string_lossy().to_string();
        self.run_args(&svec(&["projects", "add", &project]), "json")?;
        Ok(())
    }

    /// Removes a project from the Hub registry. The files stay on disk.
    pub fn remove_project(&self, path: &str) -> Result<(), UnityError> {
        let project = validate_project_path(path)?;
        let project = project.to_string_lossy().to_string();
        self.run_args(&svec(&["projects", "remove", &project]), "json")?;
        Ok(())
    }

    /// Pins a project to the top of the registry list, or unpins it.
    pub fn set_project_pinned(&self, path: &str, pinned: bool) -> Result<(), UnityError> {
        let project = validate_project_path(path)?;
        let project = project.to_string_lossy().to_string();
        let command = if pinned { "pin" } else { "unpin" };
        self.run_args(&svec(&["projects", command, &project]), "json")?;
        Ok(())
    }

    pub fn changelog(&self, target: Option<&str>) -> Result<RawChangelog, UnityError> {
        let mut parts = svec(&["changelog"]);
        if let Some(target) = target.filter(|value| !value.trim().is_empty()) {
            validate_cli_version(target)?;
            parts.extend(svec(&["--target", target]));
        }
        let executed = self.run_args(&parts, "json")?;
        parse_data(executed.envelope)
    }

    pub fn auth_status(&self) -> Result<AuthStatus, UnityError> {
        let spec = self.spec(&svec(&["auth", "status"]), "json")?;
        match self.execute_envelope(&spec) {
            Ok(executed) if executed.envelope.data.is_some() => parse_data(executed.envelope),
            Ok(executed) if !executed.envelope.success => Err(envelope_error(
                &executed.envelope,
                &executed.stderr,
                executed.exit_code,
            )),
            Ok(executed) => parse_data(executed.envelope),
            Err(error) => match auth_status_from_error(&error) {
                Some(status) => Ok(status),
                None => Err(error),
            },
        }
    }

    pub fn auth_login_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&auth_login_parts(), "ndjson")
    }

    /// Starts the CLI's resident sign-in broker.
    ///
    /// CLI commands read the stored session through that broker. When it is not
    /// running, each command reads the OS credential store itself; on Linux a
    /// locked Secret Service collection then shows its unlock prompt once per
    /// command. Starting the broker before a read batch collapses those prompts
    /// into at most one.
    pub fn ensure_auth_broker(&self) -> Result<(), UnityError> {
        self.run_args(&auth_broker_ensure_parts(), "json")?;
        Ok(())
    }

    pub fn license_status(&self) -> Result<LicenseStatus, UnityError> {
        let executed = self.run_args(&svec(&["license", "status"]), "json")?;
        parse_data(executed.envelope)
    }

    /// Runs `unity install <version> --dry-run` and returns the raw preview.
    pub fn dry_run_install(&self, version: &str, modules: &[String]) -> Result<Value, UnityError> {
        validate_editor_version(version)?;
        let mut parts = svec(&["install", version, "--yes", "--dry-run"]);
        append_modules(&mut parts, modules)?;
        let executed = self.run_args(&parts, "json")?;
        Ok(executed.envelope.data.unwrap_or(Value::Null))
    }

    pub fn install_editor_spec(
        &self,
        version: &str,
        modules: &[String],
        accept_eula: bool,
    ) -> Result<CommandSpec, UnityError> {
        validate_editor_version(version)?;
        let mut parts = svec(&["install", version, "--yes"]);
        append_modules(&mut parts, modules)?;
        if accept_eula {
            parts.push("--accept-eula".to_string());
        }
        self.spec(&parts, "ndjson")
    }

    pub fn install_modules_spec(
        &self,
        version: &str,
        modules: &[String],
        accept_eula: bool,
    ) -> Result<CommandSpec, UnityError> {
        let parts = install_modules_parts(version, modules, accept_eula)?;
        self.spec(&parts, "ndjson")
    }

    pub fn upgrade_project_spec(
        &self,
        path: &str,
        version: &str,
    ) -> Result<CommandSpec, UnityError> {
        validate_editor_version(version)?;
        let project = validate_project_path(path)?;
        let project = project.to_string_lossy().to_string();
        // No --wait: that would keep this job alive until the Editor exits.
        // The upgrade command itself reports when the version change is done,
        // which is the unit of work this screen owns.
        let parts = svec(&["projects", "upgrade", &project, "--to", version, "--yes"]);
        self.spec(&parts, "ndjson")
    }

    pub fn self_update_check(&self) -> Result<CliUpdateInfo, UnityError> {
        let executed = self.run_args(&svec(&["self-update", "--check"]), "json")?;
        let mut info: CliUpdateInfo = parse_data(executed.envelope)?;
        info.update_available = info.current_version != info.latest_version;
        Ok(info)
    }

    /// `unity self-update` does not emit NDJSON in 1.0.0: it prints a two-column
    /// result line even under `--format ndjson`. Use human output for the job
    /// log, then refresh `unity version` once the job finishes.
    pub fn self_update_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&svec(&["self-update", "--yes"]), "human")
    }
}

fn pipeline_editors_from_data(data: Option<Value>) -> Result<PipelineEditors, UnityError> {
    let Some(data) = data else {
        return Ok(PipelineEditors::default());
    };
    if data.is_null() {
        return Ok(PipelineEditors::default());
    }

    let instances = if data.is_array() {
        parse_items(Some(data.clone()), &[])?
    } else {
        let instances = data.get("instances");
        let instances_data = match instances {
            Some(Value::Array(_)) => instances.cloned(),
            _ => data
                .get("items")
                .filter(|items| items.is_array())
                .cloned()
                .or_else(|| instances.cloned()),
        };
        parse_items(instances_data, &[])?
    };
    let latest_version = data
        .get("latestVersion")
        .and_then(Value::as_str)
        .map(str::to_string);
    let summary = data.get("summary").cloned();

    Ok(PipelineEditors {
        instances,
        latest_version,
        summary,
    })
}

fn merge_pipeline_editors(running: &mut RunningEditors, pipeline: PipelineEditors) {
    for instance in pipeline.instances {
        // A stale pipeline row for an Editor that has already quit must not
        // create a phantom Overview row.
        if instance.is_running == Some(false) {
            continue;
        }
        let pid = instance.pid;
        let project_path = instance.project_path.clone();
        let position = pid
            .and_then(|pid| {
                running
                    .instances
                    .iter()
                    .position(|candidate| candidate.pid == Some(pid))
            })
            .or_else(|| {
                project_path.as_deref().and_then(|path| {
                    running.instances.iter().position(|candidate| {
                        candidate
                            .project_path
                            .as_deref()
                            .is_some_and(|candidate| same_project_path(candidate, path))
                    })
                })
            });

        if let Some(index) = position {
            let target = &mut running.instances[index];
            if target.project_path.is_none() {
                target.project_path = instance.project_path.clone();
            }
            if target.editor_version.is_none() {
                target.editor_version = instance.editor_version.clone();
            }
            if target.pid.is_none() {
                target.pid = instance.pid;
            }
            if target.port.is_none() {
                target.port = instance.server_port();
            }
            if instance.phase.is_some() {
                target.phase = instance.phase.clone();
            }
            if instance.pipeline_version.is_some() {
                target.pipeline_version = instance.pipeline_version.clone();
            }
            target.update_available |= instance.update_available;
            if instance.safe_mode.is_some() {
                target.safe_mode = instance.safe_mode.clone();
            }
            if instance.dialog.is_some() {
                target.dialog = instance.dialog.clone();
            }
            if instance.state.is_some() {
                target.state = instance.state.clone();
            }
            if instance.frame_count.is_some() {
                target.frame_count = instance.frame_count;
            }
            if instance.player_loop_ticking.is_some() {
                target.player_loop_ticking = instance.player_loop_ticking;
            }
        } else {
            running.instances.push(instance.into_running_editor());
        }
    }
    running.count = running.instances.len();
}

fn merge_status_instances(running: &mut RunningEditors, status: Vec<StatusInstance>) {
    for instance in status {
        let position = instance
            .pid
            .and_then(|pid| {
                running
                    .instances
                    .iter()
                    .position(|candidate| candidate.pid == Some(pid))
            })
            .or_else(|| {
                instance.project_path.as_deref().and_then(|path| {
                    running.instances.iter().position(|candidate| {
                        candidate
                            .project_path
                            .as_deref()
                            .is_some_and(|candidate| same_project_path(candidate, path))
                    })
                })
            });

        if let Some(index) = position {
            let target = &mut running.instances[index];
            if target.project_path.is_none() {
                target.project_path = instance.project_path.clone();
            }
            if target.editor_version.is_none() {
                target.editor_version = instance.editor_version.clone();
            }
            if target.pid.is_none() {
                target.pid = instance.pid;
            }
            if target.port.is_none() {
                target.port = instance.port;
            }
            target.state = instance.state.clone();
            target.frame_count = instance.frame_count;
            target.player_loop_ticking = instance.player_loop_ticking;
            if instance.safe_mode.is_some() {
                target.safe_mode = instance.safe_mode.clone();
            }
            if instance.dialog.is_some() {
                target.dialog = instance.dialog.clone();
            }
        } else {
            running.instances.push(RunningEditor {
                project_path: instance.project_path,
                editor_version: instance.editor_version,
                pid: instance.pid,
                port: instance.port,
                state: instance.state,
                frame_count: instance.frame_count,
                player_loop_ticking: instance.player_loop_ticking,
                safe_mode: instance.safe_mode,
                dialog: instance.dialog,
                extra: instance.extra,
                ..RunningEditor::default()
            });
        }
    }
    running.count = running.instances.len();
}

fn same_project_path(left: &str, right: &str) -> bool {
    let left = left.trim().trim_end_matches(['/', '\\']);
    let right = right.trim().trim_end_matches(['/', '\\']);
    left == right || (cfg!(windows) && left.eq_ignore_ascii_case(right))
}

fn status_instances_from_envelope(
    envelope: &ValueEnvelope,
    stderr: &str,
    exit_code: Option<i32>,
) -> Result<Vec<StatusInstance>, UnityError> {
    if !envelope.success {
        let empty = envelope
            .errors
            .iter()
            .any(|error| error.code == "STATUS_NO_INSTANCES");
        if empty {
            return Ok(Vec::new());
        }
        return Err(envelope_error(envelope, stderr, exit_code));
    }
    parse_items(envelope.data.clone(), &["instances"])
}

fn envelope_error(envelope: &ValueEnvelope, stderr: &str, exit_code: Option<i32>) -> UnityError {
    envelope
        .errors
        .first()
        .map(|error| error.to_unity_error(exit_code, stderr))
        .unwrap_or_else(|| {
            UnityError::new(
                "COMMAND_FAILED",
                exit_code
                    .map(message_for_exit_code)
                    .unwrap_or("The Unity CLI command failed."),
            )
            .with_exit_code(exit_code)
            .with_detail(stderr.trim())
        })
}

fn spawn_error(spec: &CommandSpec, err: std::io::Error) -> UnityError {
    if err.kind() == std::io::ErrorKind::NotFound {
        UnityError::new(
            "CLI_NOT_FOUND",
            format!(
                "Could not run `{}`. The Unity CLI is not installed or is not executable.",
                spec.program.display()
            ),
        )
        .with_detail(err.to_string())
    } else {
        UnityError::new("SPAWN_FAILED", "The Unity CLI could not be started.")
            .with_detail(format!("{}: {err}", spec.program.display()))
    }
}

/// Runs a child with a hard deadline. `Command::output` cannot be cancelled, so
/// stdout/stderr are drained on dedicated threads while the supervisor polls.
fn run_command(spec: &CommandSpec, timeout: Duration) -> Result<std::process::Output, UnityError> {
    let mut command = crate::process::hidden(&spec.program);
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

    let mut child = command.spawn().map_err(|err| spawn_error(spec, err))?;
    let stdout_reader = child.stdout.take().map(spawn_reader);
    let stderr_reader = child.stderr.take().map(spawn_reader);

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(UnityError::new(
                    "SPAWN_FAILED",
                    "The Unity CLI process could not be monitored.",
                )
                .with_detail(format!("{}: {err}", spec.program.display())));
            }
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            // Do not join the readers here: a grandchild can keep the pipes
            // open. The process exit closes them and the detached threads end.
            drop(stdout_reader);
            drop(stderr_reader);
            return Err(
                UnityError::new("TIMEOUT", "The Unity CLI did not respond in time.").with_detail(
                    format!(
                        "No result after {:.0}s from `{}`.",
                        timeout.as_secs_f64(),
                        spec.program.display()
                    ),
                ),
            );
        }

        std::thread::sleep(Duration::from_millis(10));
    };

    let stdout = stdout_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let stderr = stderr_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();

    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn spawn_reader<R>(mut reader: R) -> std::thread::JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = reader.read_to_end(&mut buffer);
        buffer
    })
}

/// The result of one synchronous CLI invocation.
#[derive(Debug, Clone)]
pub struct Executed {
    pub envelope: ValueEnvelope,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// Resolves and runs the Unity CLI.
#[derive(Clone)]
pub struct UnityCli {
    override_path: Arc<RwLock<Option<PathBuf>>>,
}

#[derive(Debug, Clone)]
pub struct CliLocation {
    pub path: PathBuf,
    pub source: &'static str,
}

impl Default for UnityCli {
    fn default() -> Self {
        Self::new()
    }
}

impl UnityCli {
    pub fn new() -> Self {
        Self {
            override_path: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_override_path(&self, path: Option<PathBuf>) {
        *self.override_path.write().expect("CLI path lock poisoned") = path;
    }

    pub fn override_path(&self) -> Option<PathBuf> {
        self.override_path
            .read()
            .expect("CLI path lock poisoned")
            .clone()
    }

    /// Finds the executable and reports which strategy won.
    pub fn locate(&self) -> Result<CliLocation, UnityError> {
        if let Some(path) = self.override_path() {
            if path.is_file() {
                return Ok(CliLocation {
                    path,
                    source: "settings override",
                });
            }
            return Err(UnityError::new(
                "CLI_NOT_FOUND",
                "The CLI path saved in Settings no longer exists.",
            )
            .with_detail(format!("Saved path: {}", path.display())));
        }

        if let Some(path) = read_install_receipt() {
            return Ok(CliLocation {
                path,
                source: "Unity install receipt",
            });
        }

        if let Some(home) = std::env::var_os("UNITY_CLI_HOME") {
            let path = PathBuf::from(home).join("bin").join(executable_name());
            if path.is_file() {
                return Ok(CliLocation {
                    path,
                    source: "UNITY_CLI_HOME",
                });
            }
        }

        for (path, source) in platform_candidates() {
            if path.is_file() {
                return Ok(CliLocation { path, source });
            }
        }

        if let Some(path) = find_on_path(executable_name()) {
            return Ok(CliLocation {
                path,
                source: "PATH",
            });
        }

        Err(UnityError::cli_not_found())
    }

    pub fn resolve(&self) -> Result<PathBuf, UnityError> {
        self.locate().map(|location| location.path)
    }
}

fn executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "unity.exe"
    } else {
        "unity"
    }
}

fn platform_candidates() -> Vec<(PathBuf, &'static str)> {
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        if cfg!(target_os = "windows") {
            // Covered by the install-receipt probe in normal installs; this is
            // the documented default from install.ps1.
            if let Some(local) = dirs::data_local_dir() {
                candidates.push((local.join("Unity/bin/unity.exe"), "Windows default"));
            }
        } else if cfg!(target_os = "macos") {
            candidates.push((home.join(".unity/bin/unity"), "macOS default"));
        } else {
            candidates.push((home.join(".local/bin/unity"), "Linux default"));
            candidates.push((home.join(".unity/bin/unity"), "legacy install"));
        }
    }
    candidates
}

fn install_receipt_paths() -> Vec<PathBuf> {
    let Some(config) = dirs::config_dir() else {
        return Vec::new();
    };
    vec![
        config.join("unityhub").join("cli-install.json"),
        config.join("UnityHub").join("cli-install.json"),
    ]
}

fn read_install_receipt() -> Option<PathBuf> {
    for receipt in install_receipt_paths() {
        let Ok(contents) = std::fs::read_to_string(&receipt) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&contents) else {
            continue;
        };
        let Some(path) = value.get("path").and_then(Value::as_str) else {
            continue;
        };
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn find_on_path(executable: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(executable))
        .find(|candidate| candidate.is_file())
}

fn releases_parts(
    limit: u32,
    offset: u32,
    stream: Option<&str>,
) -> Result<Vec<String>, UnityError> {
    if limit == 0 || limit > 200 {
        return Err(UnityError::invalid_input(
            "Release limit must be between 1 and 200.",
        ));
    }
    let mut parts = svec(&["releases"]);
    if let Some(stream) = stream.filter(|value| !value.trim().is_empty()) {
        validate_release_stream(stream)?;
        parts.extend(svec(&["--stream", stream]));
    }
    parts.extend(svec(&[
        "--limit",
        &limit.to_string(),
        "--offset",
        &offset.to_string(),
    ]));
    Ok(parts)
}

fn modules_parts(version: &str) -> Result<Vec<String>, UnityError> {
    validate_editor_version(version)?;
    Ok(svec(&["modules", version]))
}

fn projects_parts() -> Vec<String> {
    svec(&["projects", "--all"])
}

fn templates_parts(
    editor_version: &str,
    filter: TemplateFilter,
    installed_only: bool,
) -> Result<Vec<String>, UnityError> {
    validate_editor_version(editor_version)?;
    let mut parts = svec(&["templates", "--editor-version", editor_version, "--brief"]);
    if filter != TemplateFilter::All {
        parts.extend(svec(&["--type", filter.cli_value()]));
    }
    if installed_only {
        parts.push("--installed".to_string());
    }
    Ok(parts)
}

fn templates_create_parts(options: &TemplateCreateOptions) -> Result<Vec<String>, UnityError> {
    let project = validate_project_path(&options.project_path)?;
    validate_package_id(&options.name)?;
    let display_name = options.display_name.trim();
    if display_name.is_empty() {
        return Err(UnityError::invalid_input("Enter a template display name."));
    }
    validate_template_version(&options.version)?;
    let output_dir = validate_output_dir(&options.output_dir)?;
    let project = project.to_string_lossy().into_owned();
    let output_dir = output_dir.to_string_lossy().into_owned();

    let mut parts = svec(&["templates", "create", &project]);
    parts.extend(svec(&["--name", options.name.trim()]));
    parts.extend(svec(&["--display-name", display_name]));
    if !options.description.trim().is_empty() {
        parts.extend(svec(&["--description", options.description.trim()]));
    }
    if !options.version.trim().is_empty() {
        parts.extend(svec(&["--version", options.version.trim()]));
    }
    parts.extend(svec(&["--output-dir", &output_dir]));
    if options.overwrite {
        parts.push("--overwrite".to_string());
    }
    Ok(parts)
}

fn templates_pack_parts(options: &TemplatePackOptions) -> Result<Vec<String>, UnityError> {
    let project = validate_project_path(&options.project_path)?;
    let output = validate_archive_path(&options.output)?;
    if absolute_for_compare(&output).starts_with(absolute_for_compare(&project)) {
        return Err(UnityError::invalid_input(
            "The archive destination cannot be inside the project being packed.",
        ));
    }
    if !options.name.trim().is_empty() {
        validate_package_id(&options.name)?;
        if options.display_name.trim().is_empty() {
            return Err(UnityError::invalid_input(
                "Enter a template display name, or clear the package id.",
            ));
        }
        validate_template_version(&options.version)?;
    }

    let project = project.to_string_lossy().into_owned();
    let output = output.to_string_lossy().into_owned();

    let mut parts = svec(&["templates", "pack", &project]);
    parts.extend(svec(&["--output", &output]));
    if !options.name.trim().is_empty() {
        parts.extend(svec(&["--name", options.name.trim()]));
        parts.extend(svec(&["--display-name", options.display_name.trim()]));
        if !options.description.trim().is_empty() {
            parts.extend(svec(&["--description", options.description.trim()]));
        }
        if !options.version.trim().is_empty() {
            parts.extend(svec(&["--version", options.version.trim()]));
        }
    }
    if options.overwrite {
        parts.push("--overwrite".to_string());
    }
    Ok(parts)
}

fn create_project_parts(options: &CreateProjectOptions) -> Result<Vec<String>, UnityError> {
    validate_project_name(&options.name)?;
    let output_dir = validate_output_dir(&options.output_dir)?;
    validate_editor_version(&options.editor_version)?;
    validate_template_id(&options.template)?;
    let architecture = options.architecture.trim();
    if !matches!(architecture, "x86_64" | "arm64") {
        return Err(UnityError::invalid_input(
            "Choose the x86_64 or arm64 Editor architecture.",
        ));
    }

    let output_dir = output_dir.to_string_lossy().into_owned();
    let mut parts = svec(&[
        "projects",
        "create",
        options.name.trim(),
        "--output-dir",
        &output_dir,
        "--editor-version",
        options.editor_version.trim(),
        "--template",
        options.template.trim(),
        "--architecture",
        architecture,
        // The GUI must never link a new project to Cloud implicitly.
        "--no-cloud",
    ]);
    if options.open {
        parts.push("--open".to_string());
    }
    if options.bare {
        parts.push("--bare".to_string());
    }
    Ok(parts)
}

fn absolute_for_compare(path: &std::path::Path) -> std::path::PathBuf {
    if let Ok(path) = path.canonicalize() {
        return path;
    }
    if let Some(file_name) = path.file_name() {
        if let Some(parent) = path.parent() {
            if let Ok(parent) = parent.canonicalize() {
                return parent.join(file_name);
            }
        }
    }
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

fn open_project_parts(path: &str) -> Result<Vec<String>, UnityError> {
    let project = validate_project_path(path)?;
    let project = project.to_string_lossy().to_string();
    Ok(svec(&["open", &project]))
}

fn install_modules_parts(
    version: &str,
    modules: &[String],
    accept_eula: bool,
) -> Result<Vec<String>, UnityError> {
    validate_editor_version(version)?;
    if modules.is_empty() {
        return Err(UnityError::invalid_input(
            "Select at least one module to install.",
        ));
    }
    let mut parts = svec(&["install-modules", "--editor-version", version, "--yes"]);
    append_modules(&mut parts, modules)?;
    if accept_eula {
        parts.push("--accept-eula".to_string());
    }
    Ok(parts)
}

fn auth_status_from_error(error: &UnityError) -> Option<AuthStatus> {
    matches!(error.code.as_str(), "NOT_SIGNED_IN" | "SESSION_EXPIRED").then(|| AuthStatus {
        logged_in: false,
        session_state: Some(error.code.clone()),
        ..AuthStatus::default()
    })
}

fn auth_login_parts() -> Vec<String> {
    svec(&["auth", "login"])
}

fn auth_broker_ensure_parts() -> Vec<String> {
    svec(&["auth", "broker", "ensure"])
}

fn append_modules(parts: &mut Vec<String>, modules: &[String]) -> Result<(), UnityError> {
    for module in modules {
        validate_module_id(module)?;
        parts.push("-m".to_string());
        parts.push(module.clone());
    }
    Ok(())
}

pub fn svec(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// Quotes for display only. Not a shell execution path.
fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let safe = value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=@+".contains(ch));
    if safe {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PipelineEditor;

    #[test]
    fn parses_pipeline_editors_status_fields() {
        let data = serde_json::json!({
            "instances": [{
                "projectName": "Game",
                "projectPath": "/tmp/Game",
                "pid": 42,
                "isRunning": true,
                "editorPhase": "loading project",
                "pipelineVersion": "0.9.1-exp.1",
                "updateAvailable": true,
                "safeMode": null,
                "dialog": null,
                "pipelineServer": { "port": 7800, "isReachable": true }
            }],
            "items": [],
            "latestVersion": "0.10.0-exp.1",
            "summary": { "totalInstances": 1, "instancesInSafeMode": 0 }
        });

        let parsed = pipeline_editors_from_data(Some(data)).unwrap();
        assert_eq!(parsed.instances.len(), 1);
        assert_eq!(parsed.latest_version.as_deref(), Some("0.10.0-exp.1"));
        assert_eq!(
            parsed
                .summary
                .as_ref()
                .and_then(|summary| summary.get("totalInstances"))
                .and_then(Value::as_u64),
            Some(1)
        );

        let editor = &parsed.instances[0];
        assert_eq!(editor.phase.as_deref(), Some("loading project"));
        assert_eq!(editor.pipeline_version.as_deref(), Some("0.9.1-exp.1"));
        assert!(editor.update_available);
        assert_eq!(editor.server_port(), Some(7800));

        let running = parsed
            .instances
            .into_iter()
            .next()
            .unwrap()
            .into_running_editor();
        assert!(!running.in_safe_mode());
        assert!(running.dialog_text().is_none());
    }

    #[test]
    fn parses_pipeline_editors_items_fallback_and_safe_mode_dialog() {
        let data = serde_json::json!({
            "items": [{
                "projectPath": "/tmp/Game",
                "pid": 7,
                "safeMode": {
                    "detected": true,
                    "compileErrors": [{ "message": "CS1002: ; expected" }]
                },
                "dialog": {
                    "title": "Save changes?",
                    "message": "Save before continuing?"
                }
            }]
        });

        let parsed = pipeline_editors_from_data(Some(data)).unwrap();
        let running = parsed
            .instances
            .into_iter()
            .next()
            .unwrap()
            .into_running_editor();

        assert!(running.in_safe_mode());
        assert_eq!(running.dialog_text().as_deref(), Some("Save changes?"));
    }

    #[test]
    fn pipeline_status_merges_by_pid_and_adds_missing_editors() {
        let mut running = RunningEditors {
            count: 1,
            instances: vec![RunningEditor {
                project_path: Some("/tmp/Game".to_string()),
                editor_version: Some("6000.6.6f1".to_string()),
                pid: Some(42),
                ..RunningEditor::default()
            }],
        };
        let pipeline = PipelineEditors {
            instances: vec![
                PipelineEditor {
                    project_path: Some("/tmp/Game".to_string()),
                    pid: Some(42),
                    phase: Some("ready".to_string()),
                    pipeline_version: Some("0.9.1-exp.1".to_string()),
                    update_available: true,
                    ..PipelineEditor::default()
                },
                PipelineEditor {
                    project_path: Some("/tmp/Other".to_string()),
                    pid: Some(43),
                    phase: Some("loading project".to_string()),
                    ..PipelineEditor::default()
                },
            ],
            ..PipelineEditors::default()
        };

        merge_pipeline_editors(&mut running, pipeline);

        assert_eq!(running.count, 2);
        assert_eq!(running.instances[0].phase.as_deref(), Some("ready"));
        assert_eq!(
            running.instances[0].pipeline_version.as_deref(),
            Some("0.9.1-exp.1")
        );
        assert!(running.instances[0].update_available);
        assert_eq!(
            running.instances[1].project_path.as_deref(),
            Some("/tmp/Other")
        );
        assert!(running.instances[1].editor_version.is_none());
    }

    #[test]
    fn pipeline_only_instances_are_added_to_an_empty_base() {
        let mut running = RunningEditors::default();
        let pipeline = PipelineEditors {
            instances: vec![PipelineEditor {
                project_path: Some("/tmp/Game".to_string()),
                pid: Some(7),
                is_running: Some(true),
                phase: Some("loading project".to_string()),
                ..PipelineEditor::default()
            }],
            ..PipelineEditors::default()
        };

        merge_pipeline_editors(&mut running, pipeline);

        assert_eq!(running.count, 1);
        assert_eq!(
            running.instances[0].project_path.as_deref(),
            Some("/tmp/Game")
        );
        assert_eq!(
            running.instances[0].phase.as_deref(),
            Some("loading project")
        );
        assert!(running.instances[0].editor_version.is_none());
    }

    #[test]
    fn pipeline_instances_merge_by_path_when_neither_has_a_pid() {
        let mut running = RunningEditors {
            count: 1,
            instances: vec![RunningEditor {
                project_path: Some("/tmp/Game/".to_string()),
                ..RunningEditor::default()
            }],
        };
        let pipeline = PipelineEditors {
            instances: vec![PipelineEditor {
                project_path: Some("/tmp/Game".to_string()),
                phase: Some("ready".to_string()),
                ..PipelineEditor::default()
            }],
            ..PipelineEditors::default()
        };

        merge_pipeline_editors(&mut running, pipeline);

        assert_eq!(running.count, 1);
        assert_eq!(running.instances.len(), 1);
        assert_eq!(running.instances[0].phase.as_deref(), Some("ready"));
    }

    #[test]
    fn stopped_pipeline_instances_are_not_added() {
        let mut running = RunningEditors::default();
        let pipeline = PipelineEditors {
            instances: vec![PipelineEditor {
                project_path: Some("/tmp/Game".to_string()),
                pid: Some(7),
                is_running: Some(false),
                ..PipelineEditor::default()
            }],
            ..PipelineEditors::default()
        };

        merge_pipeline_editors(&mut running, pipeline);

        assert_eq!(running.count, 0);
        assert!(running.instances.is_empty());
    }

    #[test]
    fn pipeline_editor_server_port_falls_back_to_the_top_level_field() {
        let editor: PipelineEditor =
            serde_json::from_value(serde_json::json!({ "serverPort": 7801 })).unwrap();
        assert_eq!(editor.server_port(), Some(7801));
    }

    #[test]
    fn pipeline_editors_falls_back_to_items_when_instances_is_unusable() {
        for instances in [Value::Null, Value::String("not-a-list".to_string())] {
            let data = serde_json::json!({
                "instances": instances,
                "items": [{ "projectPath": "/tmp/Game", "pid": 7 }]
            });

            let parsed = pipeline_editors_from_data(Some(data)).unwrap();

            assert_eq!(parsed.instances.len(), 1);
            assert_eq!(
                parsed.instances[0].project_path.as_deref(),
                Some("/tmp/Game")
            );
        }
    }

    #[test]
    fn display_quotes_paths_with_spaces() {
        let spec = CommandSpec::new("/home/me/unity", svec(&["projects", "--all"]));
        assert_eq!(spec.display(), "/home/me/unity projects --all");

        let spec = CommandSpec::new(
            "/home/me/unity",
            svec(&["projects", "upgrade", "/home/me/My Game"]),
        );
        assert!(spec.display().contains("'/home/me/My Game'"));
    }

    #[test]
    fn rejects_module_ids_that_look_like_flags() {
        let mut parts = Vec::new();
        assert!(append_modules(&mut parts, &["--accept-eula".to_string()]).is_err());
        assert!(parts.is_empty());
    }

    #[test]
    fn projects_uses_the_flat_listing_without_the_removed_list_subcommand() {
        assert_eq!(projects_parts(), svec(&["projects", "--all"]));
    }

    #[test]
    fn modules_uses_the_flat_listing_without_the_removed_list_subcommand() {
        assert_eq!(
            modules_parts("6000.6.6f1").unwrap(),
            svec(&["modules", "6000.6.6f1"])
        );
    }

    #[test]
    fn releases_use_offset_instead_of_the_deprecated_skip_flag() {
        let parts = releases_parts(40, 20, None).unwrap();
        assert!(parts.contains(&"--offset".to_string()));
        assert!(parts.contains(&"20".to_string()));
        assert!(!parts.contains(&"--skip".to_string()));
    }

    #[test]
    fn install_modules_uses_the_current_editor_version_flag() {
        let parts =
            install_modules_parts("6000.6.6f1", &["windows-il2cpp".to_string()], true).unwrap();
        assert_eq!(parts[0], "install-modules");
        assert_eq!(parts[1], "--editor-version");
        assert_eq!(parts[2], "6000.6.6f1");
        assert!(parts.contains(&"--accept-eula".to_string()));
    }

    #[test]
    fn signed_out_auth_errors_become_a_logged_out_status() {
        let status =
            auth_status_from_error(&UnityError::new("NOT_SIGNED_IN", "You are not signed in."))
                .expect("signed-out errors should be a normal status");

        assert!(!status.logged_in);
        assert_eq!(status.session_state.as_deref(), Some("NOT_SIGNED_IN"));
    }

    #[test]
    fn expired_auth_sessions_become_a_logged_out_status() {
        let status =
            auth_status_from_error(&UnityError::new("SESSION_EXPIRED", "The session expired."))
                .expect("expired sessions should be a normal status");

        assert!(!status.logged_in);
        assert_eq!(status.session_state.as_deref(), Some("SESSION_EXPIRED"));
    }

    #[test]
    fn other_auth_errors_remain_errors() {
        assert!(
            auth_status_from_error(&UnityError::new("NETWORK_UNREACHABLE", "offline")).is_none()
        );
    }

    #[test]
    fn auth_login_uses_the_standard_login_command() {
        assert_eq!(auth_login_parts(), svec(&["auth", "login"]));
    }

    #[test]
    fn auth_broker_ensure_uses_the_supported_broker_command() {
        assert_eq!(
            auth_broker_ensure_parts(),
            svec(&["auth", "broker", "ensure"])
        );
    }

    #[test]
    fn open_project_uses_the_open_command_without_auto_accepting_upgrades() {
        let project = std::env::temp_dir();
        let project = project.to_string_lossy();
        let parts = open_project_parts(&project).unwrap();

        assert_eq!(parts[0], "open");
        assert_eq!(parts[1], project);
        assert!(!parts.contains(&"--wait".to_string()));
        assert!(!parts.contains(&"--accept-upgrades".to_string()));
    }

    #[test]
    fn every_cli_spec_opts_out_of_telemetry() {
        let path =
            std::env::temp_dir().join(format!("unity-rust-hub-cli-test-{}", std::process::id()));
        std::fs::write(&path, b"test").unwrap();

        let cli = UnityCli::new();
        cli.set_override_path(Some(path.clone()));
        let spec = cli.spec(&svec(&["version"]), "json").unwrap();
        std::fs::remove_file(path).ok();

        assert!(spec
            .env
            .iter()
            .any(|(key, value)| key == DO_NOT_TRACK_ENV && value == "1"));
    }

    #[test]
    fn self_update_uses_human_output_because_the_cli_does_not_emit_ndjson() {
        let path = std::env::temp_dir().join(format!(
            "unity-rust-hub-self-update-test-{}",
            std::process::id()
        ));
        std::fs::write(&path, b"test").unwrap();

        let cli = UnityCli::new();
        cli.set_override_path(Some(path.clone()));
        let spec = cli.self_update_spec().unwrap();
        std::fs::remove_file(path).ok();

        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["--format", "human"]));
    }

    #[cfg(unix)]
    #[test]
    fn execute_times_out_instead_of_hanging() {
        let spec = CommandSpec::new("sleep", svec(&["5"]));
        let started = Instant::now();
        let error = UnityCli::new()
            .execute_with_timeout(&spec, Duration::from_millis(100))
            .expect_err("sleep should time out");

        assert_eq!(error.code, "TIMEOUT");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn execute_envelope_keeps_data_on_failure() {
        let spec = CommandSpec::new(
            "sh",
            svec(&[
                "-c",
                r#"printf '%s' '{"success":false,"data":{"loggedIn":false},"errors":[{"code":"AUTH_ERROR","message":"signed out"}],"warnings":[]}'"#,
            ]),
        );
        let cli = UnityCli::new();
        let executed = cli.execute_envelope(&spec).unwrap();

        assert!(!executed.envelope.success);
        assert!(executed.envelope.data.is_some());
        assert!(cli.execute(&spec).is_err());
    }

    fn envelope(value: Value) -> ValueEnvelope {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn templates_use_the_brief_listing_with_the_editor_version() {
        let parts = templates_parts("6000.3.15f1", TemplateFilter::Core, true).unwrap();
        assert_eq!(parts[0], "templates");
        assert!(parts.contains(&"--brief".to_string()));
        assert!(parts.contains(&"--editor-version".to_string()));
        assert!(parts
            .windows(2)
            .any(|pair| pair == ["--editor-version", "6000.3.15f1"]));
        assert!(parts.windows(2).any(|pair| pair == ["--type", "core"]));
        assert!(parts.contains(&"--installed".to_string()));
    }

    #[test]
    fn template_listing_without_a_filter_omits_the_type_flag() {
        let parts = templates_parts("6000.3.15f1", TemplateFilter::All, false).unwrap();
        assert!(!parts.contains(&"--type".to_string()));
        assert!(!parts.contains(&"--installed".to_string()));
    }

    #[test]
    fn template_info_tolerates_a_missing_render_pipeline() {
        let info: TemplateInfo = serde_json::from_value(serde_json::json!({
            "name": "com.unity.template.universal-2d",
            "displayName": "Universal 2D",
            "version": "6.1.2",
            "status": "READY"
        }))
        .unwrap();
        assert_eq!(info.render_pipeline_label(), None);
        assert!(info.is_installed());
    }

    #[test]
    fn template_info_keeps_a_blank_render_pipeline_as_none() {
        let info: TemplateInfo = serde_json::from_value(serde_json::json!({
            "name": "com.unity.template.universal-2d",
            "renderPipeline": "  "
        }))
        .unwrap();
        assert_eq!(info.render_pipeline_label(), None);
    }

    #[test]
    fn create_project_always_passes_no_cloud_and_output_dir() {
        let dir = std::env::temp_dir();
        let options = CreateProjectOptions {
            name: "MyGame".to_string(),
            output_dir: dir.to_string_lossy().into_owned(),
            editor_version: "6000.3.15f1".to_string(),
            template: "com.unity.template.urp-blank".to_string(),
            architecture: "x86_64".to_string(),
            open: false,
            bare: false,
        };
        let parts = create_project_parts(&options).unwrap();
        assert_eq!(parts[0], "projects");
        assert_eq!(parts[1], "create");
        assert_eq!(parts[2], "MyGame");
        assert!(parts.contains(&"--no-cloud".to_string()));
        assert!(parts.contains(&"--output-dir".to_string()));
        assert!(!parts.contains(&"--path".to_string()));
        assert!(!parts.contains(&"--open".to_string()));
    }

    #[test]
    fn create_project_passes_open_only_when_requested() {
        let options = CreateProjectOptions {
            name: "MyGame".to_string(),
            output_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            editor_version: "6000.3.15f1".to_string(),
            template: "3d".to_string(),
            architecture: "arm64".to_string(),
            open: true,
            bare: true,
        };
        let parts = create_project_parts(&options).unwrap();
        assert!(parts.contains(&"--open".to_string()));
        assert!(parts.contains(&"--bare".to_string()));
    }

    #[test]
    fn create_project_rejects_an_unknown_architecture() {
        let options = CreateProjectOptions {
            name: "MyGame".to_string(),
            output_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            editor_version: "6000.3.15f1".to_string(),
            template: "3d".to_string(),
            architecture: "ppc".to_string(),
            ..CreateProjectOptions::default()
        };
        assert!(create_project_parts(&options).is_err());
    }

    #[test]
    fn pack_refuses_a_destination_inside_the_source_project() {
        let project =
            std::env::temp_dir().join(format!("unity-rust-hub-pack-test-{}", std::process::id()));
        std::fs::create_dir_all(&project).unwrap();
        let options = TemplatePackOptions {
            project_path: project.to_string_lossy().into_owned(),
            output: project.join("out.tgz").to_string_lossy().into_owned(),
            ..TemplatePackOptions::default()
        };
        let result = templates_pack_parts(&options);
        std::fs::remove_dir_all(&project).ok();
        assert!(result.is_err());
    }

    #[test]
    fn templates_create_validates_before_the_cli_runs() {
        let options = TemplateCreateOptions {
            project_path: std::env::temp_dir().to_string_lossy().into_owned(),
            name: "not a package id".to_string(),
            display_name: "Game".to_string(),
            output_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            ..TemplateCreateOptions::default()
        };
        assert!(templates_create_parts(&options).is_err());
    }

    #[test]
    fn status_exit_12_no_instances_is_an_empty_list_not_an_error() {
        let envelope = envelope(serde_json::json!({
            "success": false,
            "data": { "count": 0, "instances": [] },
            "errors": [{
                "code": "STATUS_NO_INSTANCES",
                "message": "No Unity Editor instances found with the Pipeline package installed."
            }]
        }));
        let instances = status_instances_from_envelope(&envelope, "", Some(12)).unwrap();
        assert!(instances.is_empty());
    }

    #[test]
    fn other_status_failures_stay_errors() {
        let envelope = envelope(serde_json::json!({
            "success": false,
            "errors": [{ "code": "NETWORK_UNREACHABLE", "message": "offline" }]
        }));
        assert!(status_instances_from_envelope(&envelope, "", Some(7)).is_err());
    }

    #[test]
    fn status_merges_by_pid_and_updates_the_live_fields() {
        let mut running = RunningEditors {
            count: 1,
            instances: vec![RunningEditor {
                project_path: Some("/tmp/Game".to_string()),
                pid: Some(42),
                phase: Some("loading project".to_string()),
                ..RunningEditor::default()
            }],
        };
        let status = vec![StatusInstance {
            project_path: Some("/tmp/Game".to_string()),
            pid: Some(42),
            state: Some("ready".to_string()),
            frame_count: Some(1200),
            player_loop_ticking: Some(true),
            ..StatusInstance::default()
        }];

        merge_status_instances(&mut running, status);

        assert_eq!(running.count, 1);
        assert_eq!(running.instances[0].state.as_deref(), Some("ready"));
        assert_eq!(running.instances[0].frame_count, Some(1200));
        assert_eq!(running.instances[0].player_loop_ticking, Some(true));
        assert_eq!(
            running.instances[0].phase.as_deref(),
            Some("loading project")
        );
    }

    #[test]
    fn status_adds_editors_missing_from_the_running_list() {
        let mut running = RunningEditors::default();
        let status = vec![StatusInstance {
            project_path: Some("/tmp/Game".to_string()),
            pid: Some(7),
            state: Some("unreachable".to_string()),
            ..StatusInstance::default()
        }];

        merge_status_instances(&mut running, status);

        assert_eq!(running.count, 1);
        assert_eq!(running.instances[0].state.as_deref(), Some("unreachable"));
    }

    #[test]
    fn status_merges_by_path_when_pids_are_missing() {
        let mut running = RunningEditors {
            count: 1,
            instances: vec![RunningEditor {
                project_path: Some("/tmp/Game/".to_string()),
                ..RunningEditor::default()
            }],
        };
        let status = vec![StatusInstance {
            project_path: Some("/tmp/Game".to_string()),
            state: Some("starting".to_string()),
            ..StatusInstance::default()
        }];

        merge_status_instances(&mut running, status);

        assert_eq!(running.instances.len(), 1);
        assert_eq!(running.instances[0].state.as_deref(), Some("starting"));
    }
}
