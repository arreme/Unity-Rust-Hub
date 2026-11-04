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
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl PipelineEditor {
    pub fn server_port(&self) -> Option<u16> {
        self.port.or_else(|| {
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
