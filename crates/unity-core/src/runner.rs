use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::envelope::{parse_data, parse_envelope, parse_items, ValueEnvelope};
use crate::errors::{message_for_exit_code, UnityError};
use crate::models::{
    AuthStatus, CliStatus, CliUpdateInfo, CliVersion, Editor, EditorInfo, InstallPath,
    LicenseStatus, Module, Project, RawChangelog, Release, RunningEditor, RunningEditors,
};
use crate::versions::{
    validate_cli_version, validate_editor_version, validate_module_id, validate_project_path,
    validate_release_stream,
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
        let output = run_command(spec, timeout)?;

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

    /// Opens a project with its registered Editor version. The command returns
    /// as soon as the Editor has been launched; it does not wait for it to exit.
    pub fn open_project(&self, path: &str) -> Result<(), UnityError> {
        let parts = open_project_parts(path)?;
        self.run_args(&parts, "json")?;
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

    pub fn self_update_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&svec(&["self-update", "--yes"]), "ndjson")
    }
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
    let mut command = Command::new(&spec.program);
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
}
