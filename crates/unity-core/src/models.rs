use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::errors::UnityError;

/// What the app knows about the local Unity CLI.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub installed: bool,
    pub path: Option<String>,
    /// Where the executable came from: override, install-receipt,
    /// UNITY_CLI_HOME, platform-default, or PATH.
    pub path_source: Option<String>,
    pub version: Option<CliVersion>,
    pub install_path: Option<String>,
    pub error: Option<UnityError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CliVersion {
    pub version: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub build_date: Option<String>,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CliUpdateInfo {
    #[serde(default)]
    pub current_version: String,
    #[serde(default)]
    pub latest_version: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallPath {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Editor {
    pub version: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub location: String,
    /// Full module names, from `editors --installed --long`.
    #[serde(default)]
    pub module_names: Vec<String>,
    /// Concise module display text from older CLI output.
    #[serde(default)]
    pub modules: String,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub stream: String,
    #[serde(default)]
    pub lts: bool,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub revision_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub id: String,
    #[serde(default)]
    pub aliases: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub download: String,
    #[serde(default)]
    pub installed: String,
    #[serde(default)]
    pub download_bytes: u64,
    #[serde(default)]
    pub installed_bytes: u64,
    #[serde(default)]
    pub is_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub title: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub changeset: Option<String>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub cloud_enabled: bool,
    #[serde(default)]
    pub vcs_provider: Option<String>,
    #[serde(default)]
    pub render_pipeline: Option<String>,
    #[serde(default)]
    pub build_target: Option<String>,
    #[serde(default)]
    pub last_modified: Option<i64>,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub organization_name: Option<String>,
    #[serde(default)]
    pub repository_name: Option<String>,
    #[serde(default)]
    pub cloud_project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditorInfo {
    pub version: String,
    #[serde(default)]
    pub changeset: Option<String>,
    #[serde(default)]
    pub stream: String,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub release_notes_url: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub modules: Vec<EditorInfoModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditorInfoModule {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub download_size: Option<ModuleSize>,
    #[serde(default)]
    pub installed_size: Option<ModuleSize>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub pre_selected: bool,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModuleSize {
    #[serde(default)]
    pub value: u64,
    #[serde(default)]
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunningEditors {
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub instances: Vec<RunningEditor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunningEditor {
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub editor_version: Option<String>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub port: Option<u16>,
    /// Live lifecycle phase from `unity pipeline editors`, e.g. `loading project`.
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub pipeline_version: Option<String>,
    #[serde(default)]
    pub update_available: bool,
    /// Present when the Editor is in Safe Mode. The CLI shape is still evolving.
    #[serde(default)]
    pub safe_mode: Option<Value>,
    /// Present when a modal dialog blocks the Editor. The CLI shape is still evolving.
    #[serde(default)]
    pub dialog: Option<Value>,
    /// Live state from `unity status`: `ready`, `starting`, or `unreachable`.
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub frame_count: Option<u64>,
    #[serde(default)]
    pub player_loop_ticking: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl RunningEditor {
    pub fn in_safe_mode(&self) -> bool {
        match &self.safe_mode {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(Value::Object(object)) => object
                .get("detected")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            Some(_) => true,
        }
    }

    /// The blocking dialog's title, or its message when no title is reported.
    pub fn dialog_text(&self) -> Option<String> {
        dialog_text(self.dialog.as_ref())
    }
}

/// One Editor instance from `unity pipeline editors --format json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PipelineEditor {
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub editor_version: Option<String>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub is_running: Option<bool>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default, alias = "editorPhase")]
    pub phase: Option<String>,
    #[serde(default)]
    pub pipeline_version: Option<String>,
    #[serde(default)]
    pub update_available: bool,
    #[serde(default)]
    pub safe_mode: Option<Value>,
    #[serde(default)]
    pub dialog: Option<Value>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub frame_count: Option<u64>,
    #[serde(default)]
    pub player_loop_ticking: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl PipelineEditor {
    pub fn server_port(&self) -> Option<u16> {
        self.port
            .or_else(|| {
                self.extra
                    .get("serverPort")
                    .and_then(Value::as_u64)
                    .and_then(|port| u16::try_from(port).ok())
            })
            .or_else(|| {
                self.extra
                    .get("pipelineServer")
                    .and_then(|server| server.get("port"))
                    .and_then(Value::as_u64)
                    .and_then(|port| u16::try_from(port).ok())
            })
    }

    pub fn into_running_editor(self) -> RunningEditor {
        let port = self.server_port();
        RunningEditor {
            project_path: self.project_path,
            editor_version: self.editor_version,
            pid: self.pid,
            port,
            phase: self.phase,
            pipeline_version: self.pipeline_version,
            update_available: self.update_available,
            safe_mode: self.safe_mode,
            dialog: self.dialog,
            state: self.state,
            frame_count: self.frame_count,
            player_loop_ticking: self.player_loop_ticking,
            extra: self.extra,
        }
    }
}

/// The useful parts of `unity pipeline editors --format json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PipelineEditors {
    #[serde(default)]
    pub instances: Vec<PipelineEditor>,
    #[serde(default)]
    pub latest_version: Option<String>,
    #[serde(default)]
    pub summary: Option<Value>,
}

fn dialog_text(dialog: Option<&Value>) -> Option<String> {
    let dialog = dialog?;
    if let Some(array) = dialog.as_array() {
        return array.last().and_then(|value| dialog_text(Some(value)));
    }
    for key in ["title", "message", "label", "text"] {
        if let Some(value) = dialog.get(key).and_then(Value::as_str) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    dialog
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    #[serde(default)]
    pub logged_in: bool,
    #[serde(default)]
    pub session_state: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub credential_source: Option<String>,
    #[serde(default)]
    pub user: Option<AuthUser>,
    #[serde(default)]
    pub account: Option<AuthAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthAccount {
    #[serde(default)]
    pub foreign_key: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub signed_in: bool,
    #[serde(default)]
    pub auth_mode: Option<String>,
    #[serde(default)]
    pub licenses: Vec<License>,
    #[serde(default)]
    pub floating_server: Option<Value>,
    #[serde(default)]
    pub machine_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct License {
    #[serde(default)]
    pub product: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub expires: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawChangelog {
    pub version: String,
    #[serde(default)]
    pub changelog: String,
}

/// The useful parts of `unity install --dry-run` output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallPreview {
    #[serde(default)]
    pub already_installed: bool,
    #[serde(default)]
    pub editor: Option<PreviewEditor>,
    #[serde(default)]
    pub modules: Vec<PreviewModule>,
    #[serde(default)]
    pub total_download_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreviewEditor {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub download_size: u64,
    #[serde(default)]
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreviewModule {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub download_size: u64,
    #[serde(default)]
    pub installed_size: u64,
}

// ---------------------------------------------------------------------------
// Templates

/// The `--type` filter accepted by `unity templates`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemplateFilter {
    #[default]
    All,
    Core,
    Learning,
    Sample,
    Custom,
    New,
}

impl TemplateFilter {
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Core,
        Self::Learning,
        Self::Sample,
        Self::Custom,
        Self::New,
    ];

    pub fn cli_value(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Core => "core",
            Self::Learning => "learning",
            Self::Sample => "sample",
            Self::Custom => "custom",
            Self::New => "new",
        }
    }
}

/// One row from `unity templates --brief --format json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateBrief {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub status: String,
}

impl TemplateBrief {
    /// Whether this template is installed locally.
    pub fn is_installed(&self) -> bool {
        matches!(
            self.status.to_ascii_lowercase().as_str(),
            "ready" | "upgradable"
        )
    }

    /// Whether a newer installed version is available.
    pub fn update_available(&self) -> bool {
        self.status.eq_ignore_ascii_case("upgradable")
    }
}

/// One package pinned by a template, from `unity templates info`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePackage {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub package_name: String,
    #[serde(default)]
    pub version: String,
}

/// One earlier template version from `unity templates info`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateVersion {
    #[serde(default)]
    pub name: String,
}

/// The useful parts of `unity templates info <name> --format json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub status: String,
    /// Missing or blank for Universal 2D in CLI 1.0.0.
    #[serde(default)]
    pub render_pipeline: Option<String>,
    #[serde(default)]
    pub is_user_generated: bool,
    #[serde(default)]
    pub packages: Vec<TemplatePackage>,
    #[serde(default)]
    pub previous_versions: Vec<TemplateVersion>,
    #[serde(default)]
    pub tar: Option<String>,
}

impl TemplateInfo {
    pub fn render_pipeline_label(&self) -> Option<&str> {
        self.render_pipeline
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn is_installed(&self) -> bool {
        matches!(
            self.status.to_ascii_lowercase().as_str(),
            "ready" | "upgradable"
        )
    }

    pub fn previous_version_names(&self) -> Vec<String> {
        self.previous_versions
            .iter()
            .map(|version| version.name.clone())
            .filter(|name| !name.trim().is_empty())
            .collect()
    }
}

/// Options for `unity templates create`.
#[derive(Debug, Clone, Default)]
pub struct TemplateCreateOptions {
    pub project_path: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub output_dir: String,
    pub overwrite: bool,
}

/// Options for `unity templates pack`.
#[derive(Debug, Clone, Default)]
pub struct TemplatePackOptions {
    pub project_path: String,
    pub output: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub overwrite: bool,
}

// ---------------------------------------------------------------------------
// Project creation

/// Options for `unity projects create`. The GUI always passes `--no-cloud`.
#[derive(Debug, Clone, Default)]
pub struct CreateProjectOptions {
    pub name: String,
    pub output_dir: String,
    pub editor_version: String,
    pub template: String,
    pub architecture: String,
    pub open: bool,
    pub bare: bool,
}

/// One entry from the result's `steps[]`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectStep {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl CreateProjectStep {
    pub fn failed(&self) -> bool {
        self.status.eq_ignore_ascii_case("failed")
    }

    /// The most specific explanation the CLI supplied.
    pub fn detail(&self) -> Option<&str> {
        self.error
            .as_deref()
            .or(self.message.as_deref())
            .or(self.reason.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

/// The result frame of `unity projects create`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectResult {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub opened: bool,
    #[serde(default)]
    pub steps: Vec<CreateProjectStep>,
}

impl CreateProjectResult {
    pub fn failed_steps(&self) -> Vec<&CreateProjectStep> {
        self.steps.iter().filter(|step| step.failed()).collect()
    }
}

// ---------------------------------------------------------------------------
// `unity status`

/// One connected Editor from `unity status --format json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatusInstance {
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub editor_version: Option<String>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub port: Option<u16>,
    /// `ready`, `starting`, or `unreachable`.
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub frame_count: Option<u64>,
    #[serde(default)]
    pub player_loop_ticking: Option<bool>,
    #[serde(default)]
    pub safe_mode: Option<Value>,
    #[serde(default)]
    pub dialog: Option<Value>,
    /// Newer CLI builds report `blockedBy.dialogs`.
    #[serde(default)]
    pub blocked_by: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl StatusInstance {
    pub fn dialog_text(&self) -> Option<String> {
        dialog_text(self.dialog.as_ref()).or_else(|| {
            self.blocked_by
                .as_ref()
                .and_then(|value| value.get("dialogs"))
                .and_then(|value| dialog_text(Some(value)))
        })
    }

    pub fn in_safe_mode(&self) -> bool {
        match &self.safe_mode {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(Value::Object(object)) => object
                .get("detected")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            Some(_) => true,
        }
    }
}
