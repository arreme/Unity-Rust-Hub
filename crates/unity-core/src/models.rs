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
    #[serde(flatten)]
    pub extra: Map<String, Value>,
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
