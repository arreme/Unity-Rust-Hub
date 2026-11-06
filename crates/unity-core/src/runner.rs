use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::advisory::{advisories_from_envelope, AdvisorySink, CliAdvisory};
use crate::envelope::{parse_data, parse_envelope, parse_items, ValueEnvelope};
use crate::errors::{code_for_exit_code, message_for_exit_code, UnityError};
use crate::models::{
    AuthStatus, CliStatus, CliUpdateInfo, CliVersion, DoctorReport, Editor, EditorInfo,
    EnvironmentInfo, InstallPath, LicenseStatus, Module, PipelineEditors, Project, PruneReport,
    RawChangelog, Release, RunningEditor, RunningEditors, VerifyReport,
};
use crate::versions::{
    validate_architecture, validate_cli_version, validate_download_directory,
    validate_editor_version, validate_module_id, validate_project_path, validate_release_stream,
    validate_update_channel,
};

/// The standard opt-out signal. The CLI's analytics/update callback adds a
/// noticeable startup stall to most invocations, so every call made by the GUI
/// opts out. Users can still use the CLI directly with their own preference.
pub const DO_NOT_TRACK_ENV: &str = "DO_NOT_TRACK";

/// Variables the AppImage runtime exports into the process it starts. They
/// describe the AppImage that owns the *current* process, which for the GUI is
/// the GUI's own AppImage. The GUI runs `unity` as a plain installed binary, so
/// these must never reach a CLI child: `unity self-update` reads `APPIMAGE` and
/// installs the new CLI AppImage over that path, replacing the GUI binary.
pub const APPIMAGE_ENV_VARS: [&str; 4] = ["APPIMAGE", "APPDIR", "OWD", "ARGV0"];

/// Strips [`APPIMAGE_ENV_VARS`] from a child's inherited environment.
pub fn scrub_appimage_env(command: &mut Command) {
    for key in APPIMAGE_ENV_VARS {
        command.env_remove(key);
    }
}

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

/// Advanced `unity install` options from the collapsed Advanced area.
///
/// Every field is off/empty by default, which keeps the host-default command
/// byte-for-byte the same as earlier releases.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallOptions {
    /// `--resume`: continue an interrupted download from the cache.
    pub resume: bool,
    /// `--download-only`: fetch and verify, install nothing.
    pub download_only: bool,
    /// `--no-wait`: fail immediately when another install holds the lock.
    pub no_wait: bool,
    /// `--timeout <seconds>`: bound how long to wait for the lock.
    pub timeout_secs: Option<u32>,
    /// `--set-default`: store the installed Editor as default.
    pub set_default: bool,
    /// `UNITY_INSTALL_RETRIES`: install has no `--retries` flag.
    pub retries: Option<u32>,
}

/// The two Advanced `install-modules` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleInstallOptions {
    /// `--retries <n>`: 0 disables module download retries.
    pub retries: Option<u32>,
    /// `-d` / `--download-directory`: add modules to a portable install.
    pub download_directory: Option<String>,
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

        // Collect before the success check: a failed envelope can still carry
        // the advisory that explains or fixes the failure.
        self.advisories.push(advisories_from_envelope(&envelope));

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
        // Pipeline status is a beta.16 addition. An older CLI, a stopped
        // Pipeline server, or an unexpected shape must not break the base list.
        if let Ok(pipeline) = self.pipeline_editors() {
            merge_pipeline_editors(&mut running, pipeline);
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

    /// Lists the module catalog for an Editor version.
    ///
    /// `architecture` disambiguates the two builds the CLI can install for the
    /// same version (for example `arm64` and `x86_64` on Apple Silicon). It is
    /// optional because a version installed once has only one build.
    pub fn modules(
        &self,
        version: &str,
        architecture: Option<&str>,
    ) -> Result<Vec<Module>, UnityError> {
        let parts = modules_parts(version, architecture)?;
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

    /// Builds the `unity doctor` read invocation. It takes no user input.
    pub fn doctor_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&svec(&["doctor"]), "json")
    }

    /// Reads the CLI's own system diagnostics.
    pub fn doctor(&self) -> Result<DoctorRead, UnityError> {
        let executed = self.execute(&self.doctor_spec()?)?;
        let data = executed.envelope.data.ok_or_else(|| {
            UnityError::unexpected_output("`unity doctor` returned no `data` field.")
        })?;
        let raw = serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string());
        let report: DoctorReport = serde_json::from_value(data).map_err(|err| {
            UnityError::unexpected_output(format!("Could not parse the doctor report: {err}"))
        })?;
        Ok(DoctorRead { report, raw })
    }

    /// Builds the `unity env` read invocation. It takes no user input.
    pub fn environment_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&svec(&["env"]), "json")
    }

    /// Reads the CLI's resolved environment paths and proxy.
    pub fn environment(&self) -> Result<EnvironmentInfo, UnityError> {
        let executed = self.execute(&self.environment_spec()?)?;
        parse_data(executed.envelope)
    }

    /// Builds one `unity diagnose <topic>` invocation.
    ///
    /// `diagnose` writes its redacted report as plain text to stdout in every
    /// format except JSON, so this asks for the human report and does not use
    /// the JSON envelope flags.
    pub fn diagnose_spec(&self, topic: DiagnoseTopic) -> Result<CommandSpec, UnityError> {
        self.spec(&svec(&["diagnose", topic.arg()]), "human")
    }

    /// Reads one redacted diagnose report as preformatted text.
    ///
    /// The output is text, not an envelope, but the command is parsed
    /// defensively: if a CLI version answers with JSON instead, the report text
    /// is extracted from it rather than shown as escaped JSON.
    pub fn diagnose(&self, topic: DiagnoseTopic) -> Result<String, UnityError> {
        let spec = self.diagnose_spec(topic)?;
        let output = run_command(&spec, DEFAULT_READ_TIMEOUT)?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let exit_code = output.status.code();

        let envelope = parse_envelope(&stdout).ok();
        if let Some(envelope) = &envelope {
            // A diagnose report is read-only, but it can still carry the CLI's
            // advisories. Collect them before deciding success or failure.
            self.advisories.push(advisories_from_envelope(envelope));
        }

        if let Some(envelope) = &envelope {
            if !envelope.success {
                return Err(envelope_error(envelope, &stderr, exit_code));
            }
        }

        if !output.status.success() {
            if let Some(envelope) = &envelope {
                return Err(envelope_error(envelope, &stderr, exit_code));
            }
            let code = exit_code
                .map(code_for_exit_code)
                .unwrap_or("COMMAND_FAILED");
            let message = exit_code
                .map(message_for_exit_code)
                .unwrap_or("The Unity CLI command failed.");
            let mut error = UnityError::new(code, message).with_exit_code(exit_code);
            let stderr = stderr.trim();
            if !stderr.is_empty() {
                error.detail = Some(stderr.to_string());
            }
            return Err(error);
        }

        Ok(report_text(envelope.as_ref(), &stdout))
    }

    /// Runs `unity install <version> --dry-run` and returns the raw preview.
    ///
    /// `architecture` is the build the card asked for. Leaving it `None` lets
    /// the CLI resolve the host default, which keeps the previous command
    /// (and the dry-run preview) unchanged for single-build users.
    pub fn dry_run_install(
        &self,
        version: &str,
        architecture: Option<&str>,
        modules: &[String],
        headless: bool,
    ) -> Result<Value, UnityError> {
        let mut parts = install_parts(version, architecture, modules, headless, false)?;
        parts.push("--dry-run".to_string());
        let executed = self.run_args(&parts, "json")?;
        Ok(executed.envelope.data.unwrap_or(Value::Null))
    }

    pub fn install_editor_spec(
        &self,
        version: &str,
        architecture: Option<&str>,
        modules: &[String],
        headless: bool,
        accept_eula: bool,
        options: &InstallOptions,
    ) -> Result<CommandSpec, UnityError> {
        let parts = install_parts_with(
            version,
            architecture,
            modules,
            headless,
            accept_eula,
            options,
        )?;
        let mut spec = self.spec(&parts, "ndjson")?;
        if let Some(retries) = options.retries {
            // `unity install` has no `--retries` flag; the CLI reads this env
            // var for both install and install-modules.
            spec = spec.with_env("UNITY_INSTALL_RETRIES", retries.to_string());
        }
        Ok(spec)
    }

    pub fn install_modules_spec(
        &self,
        version: &str,
        architecture: Option<&str>,
        modules: &[String],
        accept_eula: bool,
        options: &ModuleInstallOptions,
    ) -> Result<CommandSpec, UnityError> {
        let parts =
            install_modules_parts_with(version, architecture, modules, accept_eula, options)?;
        self.spec(&parts, "ndjson")
    }

    /// Builds `unity editors default <version> [--architecture]`.
    ///
    /// A version can have an `arm64` and an `x86_64` build installed side by
    /// side. Passing the architecture of the row the user clicked keeps the
    /// stored default on that build instead of letting the CLI pick one.
    pub fn editor_default_spec(
        &self,
        version: &str,
        architecture: Option<&str>,
    ) -> Result<CommandSpec, UnityError> {
        let parts = editor_default_parts(version, architecture)?;
        self.spec(&parts, "ndjson")
    }

    /// Builds `unity editors default --unset`.
    pub fn editor_default_unset_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&editor_default_unset_parts(), "ndjson")
    }

    /// Builds `unity editors verify <version> [--architecture]`.
    ///
    /// Verify is a read: it reports each component's state and never changes
    /// the install. The architecture disambiguates two builds of one version.
    pub fn editors_verify_spec(
        &self,
        version: &str,
        architecture: Option<&str>,
    ) -> Result<CommandSpec, UnityError> {
        let parts = editors_verify_parts(version, architecture)?;
        self.spec(&parts, "json")
    }

    /// Runs `unity editors verify` and keeps both the typed components and the
    /// raw envelope.
    ///
    /// A failed verify is still a result: the CLI returns the component list
    /// (and often a repair command) alongside `success: false`. The read only
    /// becomes an error when no component list came back at all.
    pub fn editors_verify(
        &self,
        version: &str,
        architecture: Option<&str>,
    ) -> Result<EditorVerifyRead, UnityError> {
        let spec = self.editors_verify_spec(version, architecture)?;
        let executed = self.execute_envelope(&spec)?;
        let raw = serde_json::to_string_pretty(&executed.envelope)
            .unwrap_or_else(|_| serde_json::to_string(&executed.envelope).unwrap_or_default());

        let Some(data) = executed.envelope.data.clone() else {
            if executed.envelope.success {
                return Err(UnityError::unexpected_output(
                    "`unity editors verify` returned no `data` field.",
                ));
            }
            return Err(envelope_error(
                &executed.envelope,
                &executed.stderr,
                executed.exit_code,
            ));
        };

        let report: VerifyReport = serde_json::from_value(data).map_err(|err| {
            UnityError::unexpected_output(format!(
                "Could not parse the `unity editors verify` report: {err}"
            ))
        })?;

        if !executed.envelope.success && report.components.is_empty() {
            return Err(envelope_error(
                &executed.envelope,
                &executed.stderr,
                executed.exit_code,
            ));
        }

        Ok(EditorVerifyRead { report, raw })
    }

    /// Builds the report-only `unity editors prune --remove-missing` read.
    ///
    /// Nothing is removed: `--remove-missing` only extends the report with
    /// registry entries whose install folder is already gone, so the prune
    /// dialog can offer that separate, file-free action from one scan.
    pub fn editors_prune_report_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&editors_prune_parts(false, false, false, true), "json")
    }

    /// Reads the prune report without changing anything.
    pub fn editors_prune(&self) -> Result<PruneReport, UnityError> {
        let executed = self.execute(&self.editors_prune_report_spec()?)?;
        parse_data(executed.envelope)
    }

    /// Builds `unity editors prune --remove --yes [--force]` as a streamed job.
    ///
    /// `--yes` is always present because the GUI owns the confirmation; the
    /// CLI must never prompt on a child with no terminal. `--force` is only for
    /// a running-Editor check that could not complete.
    pub fn editors_prune_remove_spec(&self, force: bool) -> Result<CommandSpec, UnityError> {
        self.spec(&editors_prune_parts(true, true, force, false), "ndjson")
    }

    /// Builds `unity editors prune --remove-missing --yes` as a streamed job.
    ///
    /// This changes the registry only; no files are deleted. It is independent
    /// of `--remove` and never implies it.
    pub fn editors_prune_remove_missing_spec(&self) -> Result<CommandSpec, UnityError> {
        self.spec(&editors_prune_parts(false, true, false, true), "ndjson")
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
        self.self_update_check_on(None)
    }

    /// Reads `unity self-update --check [--channel stable|beta]`.
    pub fn self_update_check_on(&self, channel: Option<&str>) -> Result<CliUpdateInfo, UnityError> {
        let parts = self_update_check_parts(channel)?;
        let executed = self.run_args(&parts, "json")?;
        let mut info: CliUpdateInfo = parse_data(executed.envelope)?;
        info.update_available = info.current_version != info.latest_version;
        Ok(info)
    }

    /// Builds the `--check` read. No other self-update flag is ever added.
    pub fn self_update_check_spec(&self, channel: Option<&str>) -> Result<CommandSpec, UnityError> {
        let parts = self_update_check_parts(channel)?;
        self.spec(&parts, "json")
    }

    /// Reads the release notes for the version an update would install.
    ///
    /// `--changelog` is its own mode; `--target` and `--channel` narrow which
    /// release it describes. It is never combined with `--rollback`.
    pub fn self_update_changelog(
        &self,
        target: Option<&str>,
        channel: Option<&str>,
    ) -> Result<RawChangelog, UnityError> {
        let parts = self_update_changelog_parts(target, channel)?;
        let executed = self.run_args(&parts, "json")?;
        parse_data(executed.envelope)
    }

    pub fn self_update_changelog_spec(
        &self,
        target: Option<&str>,
        channel: Option<&str>,
    ) -> Result<CommandSpec, UnityError> {
        let parts = self_update_changelog_parts(target, channel)?;
        self.spec(&parts, "json")
    }

    /// Reads `unity self-update --dry-run [--target] [--channel]`.
    ///
    /// The preview is a read: it downloads nothing and changes no binary.
    pub fn self_update_dry_run(
        &self,
        target: Option<&str>,
        channel: Option<&str>,
    ) -> Result<Value, UnityError> {
        let parts = self_update_preview_parts(target, channel)?;
        let executed = self.run_args(&parts, "json")?;
        Ok(executed.envelope.data.unwrap_or(Value::Null))
    }

    pub fn self_update_dry_run_spec(
        &self,
        target: Option<&str>,
        channel: Option<&str>,
    ) -> Result<CommandSpec, UnityError> {
        let parts = self_update_preview_parts(target, channel)?;
        self.spec(&parts, "json")
    }

    /// The original no-argument apply command, unchanged.
    pub fn self_update_spec(&self) -> Result<CommandSpec, UnityError> {
        self.self_update_apply_spec(None, None)
    }

    /// Builds `unity self-update --yes [--target] [--channel]` as a job.
    pub fn self_update_apply_spec(
        &self,
        target: Option<&str>,
        channel: Option<&str>,
    ) -> Result<CommandSpec, UnityError> {
        let parts = self_update_apply_parts(target, channel)?;
        self.spec(&parts, "ndjson")
    }

    /// Builds `unity self-update --rollback [version] --yes` as a job.
    ///
    /// A rollback is deliberately its own argv: no `--dry-run`, `--check`,
    /// `--changelog`, `--target` or `--channel` can ever be added, because
    /// beta.17 refuses that combination with `BAD_ARGS` and changes nothing.
    pub fn self_update_rollback_spec(
        &self,
        version: Option<&str>,
    ) -> Result<CommandSpec, UnityError> {
        let parts = self_update_rollback_parts(version)?;
        self.spec(&parts, "ndjson")
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
        } else {
            running.instances.push(instance.into_running_editor());
        }
    }
    running.count = running.instances.len();
}

fn same_project_path(left: &str, right: &str) -> bool {
    let left = left.trim().trim_end_matches(['/', '\\']);
    let right = right.trim().trim_end_matches(['/', '\\']);
    left == right || (cfg!(windows) && left.eq_ignore_ascii_case(right))
}

/// Extracts the report body from a `unity diagnose` answer.
///
/// The documented output is plain text. Some CLI builds can still answer with
/// an envelope (or a JSON object) under a format flag, so a JSON body is
/// unwrapped to its report text and only pretty-printed as a last resort.
fn report_text(envelope: Option<&ValueEnvelope>, stdout: &str) -> String {
    if let Some(envelope) = envelope {
        if let Some(data) = &envelope.data {
            return report_from_data(data);
        }
    }

    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return "The CLI wrote an empty report.".to_string();
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return report_from_data(&value);
    }
    trimmed.to_string()
}

fn report_from_data(data: &Value) -> String {
    if let Some(text) = data.as_str() {
        let text = text.trim();
        if !text.is_empty() {
            return text.to_string();
        }
    }
    for key in ["report", "text", "message"] {
        if let Some(text) = data.get(key).and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                return text.to_string();
            }
        }
    }
    serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string())
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
    scrub_appimage_env(&mut command);
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

/// A `unity diagnose` topic. The set is fixed here because none of these
/// commands takes a user-supplied value; nothing from a form reaches argv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnoseTopic {
    Proxy,
    Accelerator,
    Update,
}

impl DiagnoseTopic {
    pub const ALL: [DiagnoseTopic; 3] = [
        DiagnoseTopic::Proxy,
        DiagnoseTopic::Accelerator,
        DiagnoseTopic::Update,
    ];

    pub fn arg(self) -> &'static str {
        match self {
            Self::Proxy => "proxy",
            Self::Accelerator => "accelerator",
            Self::Update => "update",
        }
    }
}

/// A doctor report plus the raw JSON the CLI returned.
///
/// The page renders the typed checks and keeps the raw document visible, so a
/// report section this build does not model yet is never hidden from the user.
#[derive(Debug, Clone)]
pub struct DoctorRead {
    pub report: DoctorReport,
    pub raw: String,
}

/// A verify report plus the raw envelope the CLI returned.
///
/// A failed verify still lists the failing components and its repair command;
/// the raw document is what the dialog exposes when `ok` is false.
#[derive(Debug, Clone)]
pub struct EditorVerifyRead {
    pub report: VerifyReport,
    pub raw: String,
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
    advisories: AdvisorySink,
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
            advisories: AdvisorySink::default(),
        }
    }

    /// The sink every read and streamed job reports advisories to.
    pub fn advisory_sink(&self) -> AdvisorySink {
        self.advisories.clone()
    }

    /// Drains the advisories collected since the last call.
    pub fn take_advisories(&self) -> Vec<CliAdvisory> {
        self.advisories.drain()
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

fn modules_parts(version: &str, architecture: Option<&str>) -> Result<Vec<String>, UnityError> {
    validate_editor_version(version)?;
    let mut parts = svec(&["modules", version]);
    append_architecture(&mut parts, architecture)?;
    Ok(parts)
}

pub fn install_parts(
    version: &str,
    architecture: Option<&str>,
    modules: &[String],
    headless: bool,
    accept_eula: bool,
) -> Result<Vec<String>, UnityError> {
    install_parts_with(
        version,
        architecture,
        modules,
        headless,
        accept_eula,
        &InstallOptions::default(),
    )
}

/// Builds the complete `unity install` argv.
///
/// The Advanced flags are appended only when their field is set, so the
/// no-Advanced command keeps exactly the old byte sequence.
pub fn install_parts_with(
    version: &str,
    architecture: Option<&str>,
    modules: &[String],
    headless: bool,
    accept_eula: bool,
    options: &InstallOptions,
) -> Result<Vec<String>, UnityError> {
    validate_editor_version(version)?;
    if headless && !headless_editor_supported() {
        return Err(UnityError::invalid_input(
            "The headless Editor is only available on Linux.",
        ));
    }
    // These combinations are usage errors in beta.17; refuse them before the
    // user sees a consent dialog for a command the CLI will reject.
    if options.download_only && (options.resume || options.set_default) {
        return Err(UnityError::invalid_input(
            "Download-only cannot be combined with resume or set-default.",
        ));
    }
    if options.resume && options.set_default {
        return Err(UnityError::invalid_input(
            "Resume cannot be combined with set-default.",
        ));
    }
    if options.timeout_secs == Some(0) {
        return Err(UnityError::invalid_input(
            "The install timeout must be greater than zero seconds.",
        ));
    }

    let mut parts = svec(&["install", version, "--yes"]);
    // The CLI resolves the host build when `--architecture` is absent. Only
    // name a build when the user actually chose one, so the single-build
    // command stays exactly what earlier releases produced.
    append_architecture(&mut parts, architecture)?;
    append_modules(&mut parts, modules)?;
    if headless {
        // The headless build leaves out modules it cannot use, such as iOS.
        parts.push("--headless".to_string());
    }
    if accept_eula {
        parts.push("--accept-eula".to_string());
    }
    if options.resume {
        parts.push("--resume".to_string());
    }
    if options.download_only {
        parts.push("--download-only".to_string());
    }
    if options.no_wait {
        parts.push("--no-wait".to_string());
    }
    if let Some(timeout) = options.timeout_secs {
        parts.extend(["--timeout".to_string(), timeout.to_string()]);
    }
    if options.set_default {
        parts.push("--set-default".to_string());
    }
    Ok(parts)
}

fn editor_default_parts(
    version: &str,
    architecture: Option<&str>,
) -> Result<Vec<String>, UnityError> {
    validate_editor_version(version)?;
    let mut parts = svec(&["editors", "default", version]);
    append_architecture(&mut parts, architecture)?;
    Ok(parts)
}

fn editor_default_unset_parts() -> Vec<String> {
    svec(&["editors", "default", "--unset"])
}

fn editors_verify_parts(
    version: &str,
    architecture: Option<&str>,
) -> Result<Vec<String>, UnityError> {
    validate_editor_version(version)?;
    let mut parts = svec(&["editors", "verify", version]);
    append_architecture(&mut parts, architecture)?;
    Ok(parts)
}

/// Builds the `unity editors prune` argv. Every field is a boolean controlled
/// by the app, so no user token can reach this command line.
fn editors_prune_parts(remove: bool, yes: bool, force: bool, remove_missing: bool) -> Vec<String> {
    let mut parts = svec(&["editors", "prune"]);
    if remove {
        parts.push("--remove".to_string());
    }
    if remove_missing {
        parts.push("--remove-missing".to_string());
    }
    if yes {
        parts.push("--yes".to_string());
    }
    if force {
        parts.push("--force".to_string());
    }
    parts
}

/// Whether this platform can install the headless Unity Editor.
///
/// The CLI ships the headless build for Linux only, so the window that does
/// not need a display is offered only there.
pub fn headless_editor_supported() -> bool {
    cfg!(target_os = "linux")
}

fn projects_parts() -> Vec<String> {
    svec(&["projects", "--all"])
}

fn open_project_parts(path: &str) -> Result<Vec<String>, UnityError> {
    let project = validate_project_path(path)?;
    let project = project.to_string_lossy().to_string();
    Ok(svec(&["open", &project]))
}

#[cfg(test)]
fn install_modules_parts(
    version: &str,
    architecture: Option<&str>,
    modules: &[String],
    accept_eula: bool,
) -> Result<Vec<String>, UnityError> {
    install_modules_parts_with(
        version,
        architecture,
        modules,
        accept_eula,
        &ModuleInstallOptions::default(),
    )
}

fn install_modules_parts_with(
    version: &str,
    architecture: Option<&str>,
    modules: &[String],
    accept_eula: bool,
    options: &ModuleInstallOptions,
) -> Result<Vec<String>, UnityError> {
    validate_editor_version(version)?;
    if modules.is_empty() {
        return Err(UnityError::invalid_input(
            "Select at least one module to install.",
        ));
    }
    let mut parts = svec(&["install-modules", "--editor-version", version, "--yes"]);
    // CLI beta.17 makes `install-modules` respect `--architecture` when one
    // build of the version is installed, and fail when the named build is not.
    // Passing the architecture of the row the user opened keeps the module
    // install on that build instead of whatever the CLI picks.
    append_architecture(&mut parts, architecture)?;
    append_modules(&mut parts, modules)?;
    if accept_eula {
        parts.push("--accept-eula".to_string());
    }
    if let Some(retries) = options.retries {
        parts.extend(["--retries".to_string(), retries.to_string()]);
    }
    if let Some(directory) = &options.download_directory {
        let directory = validate_download_directory(directory)?.unwrap_or_default();
        if !directory.is_empty() {
            parts.extend(["--download-directory".to_string(), directory]);
        }
    }
    Ok(parts)
}

fn self_update_check_parts(channel: Option<&str>) -> Result<Vec<String>, UnityError> {
    let mut parts = svec(&["self-update", "--check"]);
    append_update_channel(&mut parts, channel)?;
    Ok(parts)
}

fn self_update_changelog_parts(
    target: Option<&str>,
    channel: Option<&str>,
) -> Result<Vec<String>, UnityError> {
    let mut parts = svec(&["self-update", "--changelog"]);
    append_update_target(&mut parts, target)?;
    append_update_channel(&mut parts, channel)?;
    Ok(parts)
}

fn self_update_preview_parts(
    target: Option<&str>,
    channel: Option<&str>,
) -> Result<Vec<String>, UnityError> {
    let mut parts = svec(&["self-update", "--dry-run"]);
    append_update_target(&mut parts, target)?;
    append_update_channel(&mut parts, channel)?;
    Ok(parts)
}

fn self_update_apply_parts(
    target: Option<&str>,
    channel: Option<&str>,
) -> Result<Vec<String>, UnityError> {
    let mut parts = svec(&["self-update", "--yes"]);
    append_update_target(&mut parts, target)?;
    append_update_channel(&mut parts, channel)?;
    Ok(parts)
}

fn self_update_rollback_parts(version: Option<&str>) -> Result<Vec<String>, UnityError> {
    let mut parts = svec(&["self-update", "--rollback"]);
    if let Some(version) = version.map(str::trim).filter(|value| !value.is_empty()) {
        validate_cli_version(version)?;
        parts.push(version.to_string());
    }
    // The GUI owns the confirmation; the child must never prompt. The only
    // self-update flags in this argv are `--rollback` and the optional version.
    parts.push("--yes".to_string());
    Ok(parts)
}

fn append_update_target(parts: &mut Vec<String>, target: Option<&str>) -> Result<(), UnityError> {
    let Some(target) = target.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    validate_cli_version(target)?;
    parts.extend(["--target".to_string(), target.to_string()]);
    Ok(())
}

fn append_update_channel(parts: &mut Vec<String>, channel: Option<&str>) -> Result<(), UnityError> {
    let Some(channel) = validate_update_channel(channel.unwrap_or_default())? else {
        return Ok(());
    };
    parts.extend(["--channel".to_string(), channel]);
    Ok(())
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

fn append_architecture(
    parts: &mut Vec<String>,
    architecture: Option<&str>,
) -> Result<(), UnityError> {
    let Some(architecture) = architecture else {
        return Ok(());
    };
    let Some(architecture) = validate_architecture(architecture)? else {
        return Ok(());
    };
    parts.extend(["--architecture".to_string(), architecture]);
    Ok(())
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
            modules_parts("6000.6.6f1", None).unwrap(),
            svec(&["modules", "6000.6.6f1"])
        );
    }

    #[test]
    fn modules_forward_the_architecture_when_a_row_names_one() {
        assert_eq!(
            modules_parts("6000.6.6f1", Some("arm64")).unwrap(),
            svec(&["modules", "6000.6.6f1", "--architecture", "arm64"])
        );
        // An older registry row can leave the architecture empty; the CLI then
        // picks the only build it knows about.
        assert_eq!(
            modules_parts("6000.6.6f1", Some(" ")).unwrap(),
            svec(&["modules", "6000.6.6f1"])
        );
    }

    #[test]
    fn install_modules_forward_the_architecture() {
        let parts = install_modules_parts(
            "6000.6.6f1",
            Some("arm64"),
            &["windows-il2cpp".to_string()],
            false,
        )
        .unwrap();
        assert_eq!(
            &parts[..4],
            &["install-modules", "--editor-version", "6000.6.6f1", "--yes"]
        );
        let architecture = parts
            .iter()
            .position(|part| part == "--architecture")
            .expect("--architecture");
        assert_eq!(parts[architecture + 1], "arm64");
    }

    #[test]
    fn headless_installs_add_the_flag_only_when_requested() {
        let modules = vec!["linux-il2cpp".to_string()];
        let regular = install_parts("6000.6.6f1", None, &modules, false, true).unwrap();
        assert!(!regular.contains(&"--headless".to_string()));

        let headless = install_parts("6000.6.6f1", None, &modules, true, true);
        if headless_editor_supported() {
            assert!(headless.unwrap().contains(&"--headless".to_string()));
        } else {
            // The headless build is Linux-only; other platforms refuse it
            // instead of sending a flag their CLI does not know.
            assert!(headless.is_err());
        }
    }

    #[test]
    fn install_omits_the_architecture_when_the_host_default_is_chosen() {
        let parts = install_parts("6000.6.6f1", None, &[], false, false).unwrap();
        assert_eq!(
            parts,
            svec(&["install", "6000.6.6f1", "--yes"]),
            "a host-default install must stay byte-for-byte the old command"
        );
    }

    #[test]
    fn install_names_the_chosen_architecture_before_the_modules() {
        let parts = install_parts(
            "6000.6.6f1",
            Some("arm64"),
            &["linux-il2cpp".to_string()],
            false,
            true,
        )
        .unwrap();
        assert_eq!(
            parts,
            svec(&[
                "install",
                "6000.6.6f1",
                "--yes",
                "--architecture",
                "arm64",
                "-m",
                "linux-il2cpp",
                "--accept-eula",
            ])
        );
    }

    #[test]
    fn install_rejects_an_architecture_that_looks_like_a_flag() {
        let error = install_parts("6000.6.6f1", Some("--force"), &[], false, false)
            .expect_err("invalid architectures must never reach the command line");
        assert_eq!(error.code, "INVALID_INPUT");
    }

    #[test]
    fn install_architecture_can_be_left_blank_by_an_old_registry_row() {
        let parts = install_parts("6000.6.6f1", Some("  "), &[], false, false).unwrap();
        assert!(!parts.iter().any(|part| part == "--architecture"));
    }

    #[test]
    fn editor_default_targets_one_build_and_unset_is_explicit() {
        let set = editor_default_parts("6000.6.6f1", Some("x86_64")).unwrap();
        assert_eq!(
            set,
            svec(&[
                "editors",
                "default",
                "6000.6.6f1",
                "--architecture",
                "x86_64"
            ])
        );
        // A host-default choice must not grow an arbitrary build.
        assert_eq!(
            editor_default_parts("6000.6.6f1", None).unwrap(),
            svec(&["editors", "default", "6000.6.6f1"])
        );
        assert_eq!(
            editor_default_unset_parts(),
            svec(&["editors", "default", "--unset"])
        );
    }

    #[test]
    fn editors_verify_forwards_the_architecture() {
        assert_eq!(
            editors_verify_parts("6000.6.6f1", Some("arm64")).unwrap(),
            svec(&["editors", "verify", "6000.6.6f1", "--architecture", "arm64"])
        );
        assert_eq!(
            editors_verify_parts("6000.6.6f1", None).unwrap(),
            svec(&["editors", "verify", "6000.6.6f1"])
        );
        assert!(editors_verify_parts("6000.6.6f1; rm -rf /", None).is_err());
    }

    #[test]
    fn editors_prune_is_report_only_by_default_and_names_each_mutation() {
        assert_eq!(
            editors_prune_parts(false, false, false, false),
            svec(&["editors", "prune"]),
            "the plain report must not carry a removal flag"
        );
        assert_eq!(
            editors_prune_parts(false, false, false, true),
            svec(&["editors", "prune", "--remove-missing"]),
            "the scan adds the missing class but still removes nothing"
        );
        assert_eq!(
            editors_prune_parts(true, true, false, false),
            svec(&["editors", "prune", "--remove", "--yes"])
        );
        assert_eq!(
            editors_prune_parts(true, true, true, false),
            svec(&["editors", "prune", "--remove", "--yes", "--force"])
        );
        assert_eq!(
            editors_prune_parts(false, true, false, true),
            svec(&["editors", "prune", "--remove-missing", "--yes"]),
            "--remove-missing never implies --remove"
        );
    }

    #[test]
    fn dry_run_install_accepts_the_headless_flag() {
        let parts = install_parts("6000.6.6f1", None, &[], true, false);
        if headless_editor_supported() {
            let parts = parts.unwrap();
            assert_eq!(parts[0], "install");
            assert!(parts.contains(&"--yes".to_string()));
            assert!(parts.contains(&"--headless".to_string()));
        } else {
            assert!(parts.is_err());
        }
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
            install_modules_parts("6000.6.6f1", None, &["windows-il2cpp".to_string()], true)
                .unwrap();
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
    fn install_advanced_options_add_only_the_requested_flags() {
        let modules = vec!["linux-il2cpp".to_string()];
        let options = InstallOptions {
            resume: true,
            download_only: false,
            no_wait: true,
            timeout_secs: None,
            set_default: false,
            retries: None,
        };
        assert_eq!(
            install_parts_with("6000.6.6f1", Some("arm64"), &modules, false, true, &options)
                .unwrap(),
            svec(&[
                "install",
                "6000.6.6f1",
                "--yes",
                "--architecture",
                "arm64",
                "-m",
                "linux-il2cpp",
                "--accept-eula",
                "--resume",
                "--no-wait",
            ])
        );

        let download_only = InstallOptions {
            download_only: true,
            ..InstallOptions::default()
        };
        let parts = install_parts_with("lts", None, &[], false, false, &download_only).unwrap();
        assert_eq!(parts, svec(&["install", "lts", "--yes", "--download-only"]));

        let timeout = InstallOptions {
            timeout_secs: Some(600),
            ..InstallOptions::default()
        };
        assert_eq!(
            install_parts_with("lts", None, &[], false, false, &timeout).unwrap(),
            svec(&["install", "lts", "--yes", "--timeout", "600"])
        );

        let set_default = InstallOptions {
            set_default: true,
            ..InstallOptions::default()
        };
        assert_eq!(
            install_parts_with("lts", None, &[], false, false, &set_default).unwrap(),
            svec(&["install", "lts", "--yes", "--set-default"])
        );
    }

    #[test]
    fn install_refuses_combinations_the_cli_calls_bad_args() {
        let download_only_resume = InstallOptions {
            download_only: true,
            resume: true,
            ..InstallOptions::default()
        };
        assert_eq!(
            install_parts_with("lts", None, &[], false, false, &download_only_resume)
                .unwrap_err()
                .code,
            "INVALID_INPUT"
        );

        let download_only_default = InstallOptions {
            download_only: true,
            set_default: true,
            ..InstallOptions::default()
        };
        assert!(
            install_parts_with("lts", None, &[], false, false, &download_only_default).is_err()
        );

        let resume_default = InstallOptions {
            resume: true,
            set_default: true,
            ..InstallOptions::default()
        };
        assert!(install_parts_with("lts", None, &[], false, false, &resume_default).is_err());
    }

    #[test]
    fn install_retries_go_through_the_environment_not_argv() {
        let path = std::env::temp_dir().join(format!(
            "unity-rust-hub-cli-test-{}-retries",
            std::process::id()
        ));
        std::fs::write(&path, b"test").unwrap();

        let cli = UnityCli::new();
        cli.set_override_path(Some(path.clone()));
        let spec = cli
            .install_editor_spec(
                "lts",
                None,
                &[],
                false,
                false,
                &InstallOptions {
                    retries: Some(0),
                    ..InstallOptions::default()
                },
            )
            .expect("install spec");
        std::fs::remove_file(path).ok();

        assert!(!spec.args.iter().any(|arg| arg == "--retries"));
        assert!(spec
            .env
            .iter()
            .any(|(key, value)| key == "UNITY_INSTALL_RETRIES" && value == "0"));
    }

    #[test]
    fn install_modules_advanced_options_name_retries_and_the_download_directory() {
        let options = ModuleInstallOptions {
            retries: Some(3),
            download_directory: Some("./.unity".to_string()),
        };
        assert_eq!(
            install_modules_parts_with(
                "6000.6.6f1",
                Some("x86_64"),
                &["android".to_string()],
                true,
                &options
            )
            .unwrap(),
            svec(&[
                "install-modules",
                "--editor-version",
                "6000.6.6f1",
                "--yes",
                "--architecture",
                "x86_64",
                "-m",
                "android",
                "--accept-eula",
                "--retries",
                "3",
                "--download-directory",
                "./.unity",
            ])
        );

        let no_retries = ModuleInstallOptions {
            retries: Some(0),
            ..ModuleInstallOptions::default()
        };
        let parts = install_modules_parts_with(
            "6000.6.6f1",
            None,
            &["android".to_string()],
            false,
            &no_retries,
        )
        .unwrap();
        assert!(parts.windows(2).any(|pair| pair == ["--retries", "0"]));
    }

    #[test]
    fn self_update_reads_and_applies_name_only_the_requested_flags() {
        let path = std::env::temp_dir().join(format!(
            "unity-rust-hub-cli-test-{}-self-update",
            std::process::id()
        ));
        std::fs::write(&path, b"test").unwrap();

        let cli = UnityCli::new();
        cli.set_override_path(Some(path.clone()));

        let check = cli
            .self_update_check_spec(Some("beta"))
            .expect("check spec");
        assert_eq!(
            &check.args[..4],
            &["self-update", "--check", "--channel", "beta"]
        );

        let changelog = cli
            .self_update_changelog_spec(Some("1.0.0-beta.18"), Some("beta"))
            .expect("changelog spec");
        assert_eq!(
            &changelog.args[..6],
            &[
                "self-update",
                "--changelog",
                "--target",
                "1.0.0-beta.18",
                "--channel",
                "beta"
            ]
        );

        let preview = cli
            .self_update_dry_run_spec(None, Some("stable"))
            .expect("dry-run spec");
        assert_eq!(
            &preview.args[..4],
            &["self-update", "--dry-run", "--channel", "stable"]
        );
        assert!(!preview.args.iter().any(|arg| arg == "--yes"));

        let apply = cli
            .self_update_apply_spec(Some("1.0.0-beta.18"), Some("beta"))
            .expect("apply spec");
        assert_eq!(
            &apply.args[..6],
            &[
                "self-update",
                "--yes",
                "--target",
                "1.0.0-beta.18",
                "--channel",
                "beta"
            ]
        );
        assert_eq!(
            cli.self_update_spec().expect("default apply").args[..2],
            ["self-update".to_string(), "--yes".to_string()]
        );

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn rollback_is_its_own_argv_and_never_joins_a_refused_flag() {
        let path = std::env::temp_dir().join(format!(
            "unity-rust-hub-cli-test-{}-rollback",
            std::process::id()
        ));
        std::fs::write(&path, b"test").unwrap();

        let cli = UnityCli::new();
        cli.set_override_path(Some(path.clone()));

        let latest = cli
            .self_update_rollback_spec(None)
            .expect("latest rollback spec");
        assert_eq!(&latest.args[..3], &["self-update", "--rollback", "--yes"]);

        let named = cli
            .self_update_rollback_spec(Some("1.0.0-beta.16"))
            .expect("named rollback spec");
        assert_eq!(
            &named.args[..4],
            &["self-update", "--rollback", "1.0.0-beta.16", "--yes"]
        );
        for refused in [
            "--dry-run",
            "--check",
            "--changelog",
            "--target",
            "--channel",
        ] {
            assert!(
                !named.args.iter().any(|arg| arg == refused),
                "{refused} must never join --rollback"
            );
        }
        std::fs::remove_file(path).ok();
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

    #[cfg(unix)]
    #[test]
    fn execute_collects_advisories_even_from_a_failed_envelope() {
        let spec = CommandSpec::new(
            "sh",
            svec(&[
                "-c",
                r#"printf '%s' '{"success":false,"errors":[{"code":"AUTH_ERROR","message":"signed out"}],"warnings":["Beta plugin."],"notifications":[{"code":"CLI_UPDATE_AVAILABLE","message":"Update available.","data":{"current":"1.0.0-beta.16","latest":"1.0.0-beta.17"}}]}'"#,
            ]),
        );
        let cli = UnityCli::new();

        let error = cli.execute(&spec).expect_err("the envelope failed");
        assert_eq!(error.code, "AUTH_ERROR");

        let advisories = cli.take_advisories();
        assert_eq!(advisories.len(), 2);
        assert_eq!(advisories[0].code.as_deref(), Some("CLI_UPDATE_AVAILABLE"));
        assert_eq!(advisories[1].message, "Beta plugin.");
        // Draining twice never duplicates the advisory.
        assert!(cli.take_advisories().is_empty());
    }

    /// The AppImage runtime exports APPIMAGE/APPDIR/OWD/ARGV0 for the AppImage
    /// that owns the GUI process. A CLI child that inherits them believes it runs
    /// from that AppImage, so `unity self-update` installs a CLI AppImage over
    /// the GUI binary and the GUI stops starting.
    #[cfg(unix)]
    #[test]
    fn cli_children_do_not_inherit_the_gui_appimage_environment() {
        const VARS: [&str; 4] = ["APPIMAGE", "APPDIR", "OWD", "ARGV0"];

        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                for key in VARS {
                    std::env::remove_var(key);
                }
            }
        }

        for key in VARS {
            std::env::set_var(key, format!("/tmp/unity-rust-hub-test-{key}"));
        }
        let _restore = Restore;

        let spec = CommandSpec::new(
            "sh",
            svec(&[
                "-c",
                r#"printf '%s|%s|%s|%s' "${APPIMAGE-unset}" "${APPDIR-unset}" "${OWD-unset}" "${ARGV0-unset}""#,
            ]),
        );
        let output = run_command(&spec, Duration::from_secs(10)).expect("sh should run");
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(stdout, "unset|unset|unset|unset");
    }

    #[test]
    fn diagnostics_read_specs_take_no_user_value_and_use_the_standard_machine_flags() {
        let path = std::env::temp_dir().join(format!(
            "unity-rust-hub-cli-test-{}-doctor-env",
            std::process::id()
        ));
        std::fs::write(&path, b"test").unwrap();

        let cli = UnityCli::new();
        cli.set_override_path(Some(path.clone()));
        let doctor = cli.doctor_spec().expect("the doctor spec resolves");
        let environment = cli.environment_spec().expect("the env spec resolves");
        std::fs::remove_file(path).ok();

        assert_eq!(
            doctor.args,
            svec(&[
                "doctor",
                "--format",
                "json",
                "--no-pager",
                "--no-banner",
                "--non-interactive",
            ])
        );
        assert_eq!(
            environment.args,
            svec(&[
                "env",
                "--format",
                "json",
                "--no-pager",
                "--no-banner",
                "--non-interactive",
            ])
        );
        assert!(doctor
            .env
            .iter()
            .any(|(key, value)| key == DO_NOT_TRACK_ENV && value == "1"));
        assert!(environment
            .env
            .iter()
            .any(|(key, value)| key == DO_NOT_TRACK_ENV && value == "1"));
    }

    #[test]
    fn diagnose_specs_ask_for_the_plain_report_and_never_a_shell_string() {
        let path = std::env::temp_dir().join(format!(
            "unity-rust-hub-cli-test-{}-diagnose",
            std::process::id()
        ));
        std::fs::write(&path, b"test").unwrap();

        let cli = UnityCli::new();
        cli.set_override_path(Some(path.clone()));
        let spec = cli
            .diagnose_spec(DiagnoseTopic::Proxy)
            .expect("the diagnose spec resolves");
        std::fs::remove_file(path).ok();

        assert_eq!(
            spec.args,
            svec(&[
                "diagnose",
                "proxy",
                "--format",
                "human",
                "--no-pager",
                "--no-banner",
                "--non-interactive",
            ])
        );
        assert!(spec
            .env
            .iter()
            .any(|(key, value)| key == DO_NOT_TRACK_ENV && value == "1"));
        assert_eq!(DiagnoseTopic::Accelerator.arg(), "accelerator");
        assert_eq!(DiagnoseTopic::Update.arg(), "update");
    }

    #[test]
    fn diagnose_text_unwraps_an_envelope_or_keeps_the_plain_report() {
        let envelope = parse_envelope(
            r#"{"success":true,"data":{"report":"Proxy: none\nNo proxy configured."}}"#,
        )
        .unwrap();
        assert_eq!(
            report_text(Some(&envelope), ""),
            "Proxy: none\nNo proxy configured."
        );

        let envelope = parse_envelope(r#"{"success":true,"data":"Proxy: none"}"#).unwrap();
        assert_eq!(report_text(Some(&envelope), ""), "Proxy: none");

        let envelope = parse_envelope(r#"{"success":true,"data":{"endpoint":"none"}}"#).unwrap();
        assert!(report_text(Some(&envelope), "").contains("\"endpoint\": \"none\""));

        assert_eq!(
            report_text(None, "Proxy: http://proxy.example\n  source: env"),
            "Proxy: http://proxy.example\n  source: env"
        );
        assert_eq!(report_text(None, "   "), "The CLI wrote an empty report.");
    }
}
