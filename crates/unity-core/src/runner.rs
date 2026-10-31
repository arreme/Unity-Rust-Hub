use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};

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
        Ok(CommandSpec::new(self.resolve()?, args))
    }

    pub fn run_args(&self, parts: &[String], format: &str) -> Result<Executed, UnityError> {
        let spec = self.spec(parts, format)?;
        self.execute(&spec)
    }

    pub fn execute(&self, spec: &CommandSpec) -> Result<Executed, UnityError> {
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

        let output = command.output().map_err(|err| {
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
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
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

        if !envelope.success {
            let error = envelope
                .errors
                .first()
                .map(|error| error.to_unity_error(exit_code, &stderr))
                .unwrap_or_else(|| {
                    UnityError::new(
                        "COMMAND_FAILED",
                        exit_code
                            .map(message_for_exit_code)
                            .unwrap_or("The Unity CLI command failed."),
                    )
                    .with_exit_code(exit_code)
                    .with_detail(stderr.trim())
                });
            return Err(error);
        }

        Ok(Executed {
            envelope,
            stderr,
            exit_code,
        })
    }

    pub fn version(&self) -> Result<CliVersion, UnityError> {
        let executed = self.run_args(&svec(&["version"]), "json")?;
        parse_data(&executed.envelope)
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
        let value: InstallPath = parse_data(&executed.envelope)?;
        Ok(value.path)
    }

    pub fn editors_installed(&self) -> Result<Vec<Editor>, UnityError> {
        let parts = svec(&["editors", "--installed", "--long"]);
        let executed = self.run_args(&parts, "json")?;
        parse_items(executed.envelope.data.as_ref(), &[])
    }

    pub fn editors_running(&self) -> Result<RunningEditors, UnityError> {
        let parts = svec(&["editors", "running"]);
        let executed = self.run_args(&parts, "json")?;
        let Some(data) = executed.envelope.data else {
            return Ok(RunningEditors::default());
        };
        if data.is_array() {
            let instances: Vec<RunningEditor> = parse_items(Some(&data), &[])?;
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
        skip: u32,
        stream: Option<&str>,
    ) -> Result<Vec<Release>, UnityError> {
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
            "--skip",
            &skip.to_string(),
        ]));
        let executed = self.run_args(&parts, "json")?;
        parse_items(executed.envelope.data.as_ref(), &[])
    }

    pub fn modules(&self, version: &str) -> Result<Vec<Module>, UnityError> {
        validate_editor_version(version)?;
        let parts = svec(&["modules", "list", version]);
        let executed = self.run_args(&parts, "json")?;
        parse_items(executed.envelope.data.as_ref(), &[])
    }

    pub fn editor_info(&self, version: &str) -> Result<EditorInfo, UnityError> {
        validate_editor_version(version)?;
        let parts = svec(&["editors", "info", version]);
        let executed = self.run_args(&parts, "json")?;
        parse_data(&executed.envelope)
    }

    pub fn projects(&self) -> Result<Vec<Project>, UnityError> {
        let parts = svec(&["projects", "list"]);
        let executed = self.run_args(&parts, "json")?;
        parse_items(executed.envelope.data.as_ref(), &[])
    }

    pub fn changelog(&self, target: Option<&str>) -> Result<RawChangelog, UnityError> {
        let mut parts = svec(&["changelog"]);
        if let Some(target) = target.filter(|value| !value.trim().is_empty()) {
            validate_cli_version(target)?;
            parts.extend(svec(&["--target", target]));
        }
        let executed = self.run_args(&parts, "json")?;
        parse_data(&executed.envelope)
    }

    pub fn auth_status(&self) -> Result<AuthStatus, UnityError> {
        let executed = self.run_args(&svec(&["auth", "status"]), "json")?;
        parse_data(&executed.envelope)
    }

    pub fn license_status(&self) -> Result<LicenseStatus, UnityError> {
        let executed = self.run_args(&svec(&["license", "status"]), "json")?;
        parse_data(&executed.envelope)
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
        validate_editor_version(version)?;
        if modules.is_empty() {
            return Err(UnityError::invalid_input(
                "Select at least one module to install.",
            ));
        }
        let mut parts = svec(&["install-modules", "-e", version, "--yes"]);
        append_modules(&mut parts, modules)?;
        if accept_eula {
            parts.push("--accept-eula".to_string());
        }
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
        let mut info: CliUpdateInfo = parse_data(&executed.envelope)?;
        info.update_available = info.current_version != info.latest_version;
        Ok(info)
    }

    pub fn self_update_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&svec(&["self-update", "--yes"]), "ndjson")
    }
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
        let spec = CommandSpec::new("/home/me/unity", svec(&["projects", "list"]));
        assert_eq!(spec.display(), "/home/me/unity projects list");

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
}
