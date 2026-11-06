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

/// The useful parts of `unity editors prune --format json`.
///
/// `editors` holds builds no registered project uses; `missing` holds registry
/// entries whose recorded install path no longer exists. The CLI only reports
/// the missing class when `--remove-missing` was passed, and removing it never
/// deletes files. `total` is the CLI's reclaimable total, never a local sum.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PruneReport {
    #[serde(default)]
    pub removed: bool,
    #[serde(default)]
    pub editors: Vec<PruneCandidate>,
    #[serde(default, deserialize_with = "default_u64")]
    pub total: u64,
    #[serde(default)]
    pub missing: Vec<PruneCandidate>,
}

/// One Editor build named by `unity editors prune`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PruneCandidate {
    #[serde(default, deserialize_with = "default_string")]
    pub version: String,
    #[serde(default, deserialize_with = "default_string")]
    pub architecture: String,
    #[serde(default, deserialize_with = "default_string")]
    pub path: String,
    /// On-disk size in bytes, or `0` when the CLI does not report one.
    #[serde(default, deserialize_with = "default_u64")]
    pub bytes: u64,
    /// A preformatted size some CLI versions send instead of `bytes`.
    #[serde(default, deserialize_with = "optional_text")]
    pub size: Option<String>,
    /// Why the CLI classified the row as removable, when it says so.
    #[serde(default, deserialize_with = "optional_text")]
    pub reason: Option<String>,
    /// The report's status column (`kept`, `removed`), when present.
    #[serde(default, deserialize_with = "optional_text")]
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The useful parts of `unity editors verify <version> --format json`.
///
/// `ok` is the CLI's own verdict. `components` carries one row per file or
/// module checked; `status` is `ok`, `missing`, or `skipped`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    #[serde(default, deserialize_with = "default_string")]
    pub version: String,
    #[serde(default, deserialize_with = "default_string")]
    pub architecture: String,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub components: Vec<VerifyComponent>,
    /// A top-level repair command, when a CLI version reports one.
    #[serde(default, deserialize_with = "optional_text")]
    pub command: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One component checked by `unity editors verify`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerifyComponent {
    #[serde(default, deserialize_with = "default_string")]
    pub component: String,
    /// `editor` or `module`.
    #[serde(default, deserialize_with = "default_string")]
    pub kind: String,
    /// `ok`, `missing`, or `skipped`.
    #[serde(default, deserialize_with = "default_string")]
    pub status: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub path: Option<String>,
    /// The exact `unity install-modules` command that repairs this component.
    #[serde(default, deserialize_with = "optional_text")]
    pub command: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
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

/// The useful parts of `unity doctor --format json`.
///
/// Every field is optional in practice: the CLI adds report sections between
/// betas, and a missing or `null` value must never fail the whole read. Fields
/// whose shape is still evolving (`auth`, `license`) stay as `Value` and are
/// summarized by the controller instead of guessed at here.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    #[serde(default, deserialize_with = "default_string")]
    pub platform: String,
    #[serde(default, deserialize_with = "default_string")]
    pub arch: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub runtime_version: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub node_version: Option<String>,
    /// A bare version string, or a version object (`{ "version": … }`).
    #[serde(default, deserialize_with = "optional_text")]
    pub cli_version: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub install_path: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub log_directory: Option<String>,
    #[serde(default, deserialize_with = "default_vec")]
    pub checks: Vec<DoctorCheck>,
    /// The CLI's auth section. The shape is still evolving, so it is carried
    /// verbatim and summarized defensively by the controller.
    #[serde(default)]
    pub auth: Option<Value>,
    /// The CLI's license section, carried for the same reason as `auth`.
    #[serde(default)]
    pub license: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One environment health check from `unity doctor`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
    #[serde(default, deserialize_with = "default_string")]
    pub id: String,
    /// `pass`, `warn`, `fail`, or `info`; an unknown value is kept as-is.
    #[serde(default, deserialize_with = "default_string")]
    pub status: String,
    /// A localization key. The app renders a humanized label first and keeps
    /// this as the fallback when the id is missing or unknown.
    #[serde(default, deserialize_with = "optional_text")]
    pub message_key: Option<String>,
    /// Values the CLI would interpolate into the localized message.
    #[serde(default, deserialize_with = "default_map")]
    pub params: Map<String, Value>,
    /// A pre-rendered message some CLI versions send alongside `messageKey`.
    #[serde(default, deserialize_with = "optional_text")]
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The useful parts of `unity env --format json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentInfo {
    #[serde(default, deserialize_with = "default_string")]
    pub user_data_path: String,
    #[serde(default, deserialize_with = "default_string")]
    pub editor_install_path: String,
    #[serde(default, deserialize_with = "default_string")]
    pub download_cache_path: String,
    #[serde(default, deserialize_with = "default_string")]
    pub config_path: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub cli_version: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub hub_version: Option<String>,
    #[serde(default)]
    pub proxy: Option<EnvironmentProxy>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The resolved proxy from `unity env`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentProxy {
    #[serde(default, deserialize_with = "optional_text")]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub source: Option<String>,
    #[serde(default, deserialize_with = "optional_text")]
    pub auth_source: Option<String>,
    /// The CLI sends an array or a comma/space separated string.
    #[serde(default, deserialize_with = "string_list")]
    pub no_proxy: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A scalar, number, or small object rendered as text. Used where the CLI has
/// shipped a string in one beta and an object in another.
fn text_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Object(object) => ["version", "value", "text", "name"]
            .iter()
            .find_map(|key| object.get(*key).and_then(text_from_value)),
        _ => None,
    }
}

fn optional_text<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| text_from_value(&value)))
}

fn default_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(optional_text(deserializer)?.unwrap_or_default())
}

fn default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

fn default_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u64>::deserialize(deserializer)?.unwrap_or_default())
}

fn default_map<'de, D>(deserializer: D) -> Result<Map<String, Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<Map<String, Value>>::deserialize(deserializer)?.unwrap_or_default())
}

fn string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    let values: Vec<String> = match value {
        Some(Value::Array(items)) => items.iter().filter_map(text_from_value).collect::<Vec<_>>(),
        Some(Value::String(text)) => text
            .split(|character: char| character == ',' || character.is_whitespace())
            .filter_map(|part| {
                let part = part.trim();
                (!part.is_empty()).then(|| part.to_string())
            })
            .collect(),
        _ => Vec::new(),
    };
    Ok(values)
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

/// One tracked agent-skill install from `unity skills --format json`.
///
/// Every field is a plain string because the CLI sends the install path, the
/// client id, the skill name, and a status for older and newer betas alike. The
/// unknown fields are kept so a newer status the app does not model yet is never
/// silently dropped.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackedSkillInstall {
    #[serde(default, deserialize_with = "default_string")]
    pub path: String,
    #[serde(default, deserialize_with = "default_string")]
    pub client: String,
    #[serde(default, deserialize_with = "default_string")]
    pub skill: String,
    #[serde(default, deserialize_with = "default_string")]
    pub status: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One supported client from `unity skill install --list --format json`.
///
/// `installed` is optional because some betas only send a `status` string and
/// others only send the boolean. [`SkillClientInfo::is_installed`] accepts both
/// without guessing: an unrecognized status stays absent.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillClientInfo {
    #[serde(default, alias = "id", deserialize_with = "default_string")]
    pub client: String,
    #[serde(default, deserialize_with = "default_string")]
    pub path: String,
    #[serde(default, deserialize_with = "default_string")]
    pub status: String,
    #[serde(default)]
    pub installed: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl SkillClientInfo {
    /// The client's install path, as the CLI reported it.
    pub fn install_path(&self) -> &str {
        &self.path
    }

    /// Whether the CLI reports this client's skill as installed. The explicit
    /// boolean wins; otherwise the documented status words do.
    pub fn is_installed(&self) -> bool {
        if let Some(installed) = self.installed {
            return installed;
        }
        matches!(
            self.status.trim().to_ascii_lowercase().as_str(),
            "installed" | "present" | "current" | "up-to-date"
        )
    }
}

/// One row of the terminal result frame of `unity skill refresh` or
/// `unity skill install`.
///
/// `status` is the CLI's own outcome word (`refreshed`, `up-to-date`,
/// `pruned`, `skipped`, `conflict`, or a `would-*` preview). `outcome()`
/// recognizes exactly those documented words and keeps anything else verbatim.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillRefreshRow {
    #[serde(default, deserialize_with = "default_string")]
    pub path: String,
    #[serde(default, deserialize_with = "default_string")]
    pub client: String,
    #[serde(default, deserialize_with = "default_string")]
    pub skill: String,
    /// The outcome word. `outcome` is accepted as an alias for older betas.
    #[serde(default, alias = "outcome", deserialize_with = "default_string")]
    pub status: String,
    /// Why a row was left alone or pruned. `reason` is accepted as an alias.
    #[serde(default, alias = "reason", deserialize_with = "optional_text")]
    pub skip_reason: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl SkillRefreshRow {
    pub fn outcome(&self) -> SkillOutcome {
        SkillOutcome::parse(&self.status)
    }

    /// The CLI's own skip reason, kept verbatim. An empty string is treated as
    /// "the CLI gave no reason" rather than shown as a blank line.
    pub fn reason(&self) -> Option<&str> {
        self.skip_reason
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
    }
}

/// The documented beta.17 skill outcomes, including its dry-run previews.
///
/// The adapter deliberately models these as an enum: the view layer must never
/// collapse `skipped` or `conflict` into a success state, and a future outcome
/// word stays visible through [`SkillOutcome::Unknown`] instead of failing the
/// whole result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillOutcome {
    Refreshed,
    UpToDate,
    Pruned,
    Skipped,
    Conflict,
    WouldUpdate,
    WouldSkip,
    WouldPrune,
    Unknown(String),
}

impl SkillOutcome {
    /// Parses one outcome word. Separators are normalized so `up_to_date` and
    /// `up-to-date` agree; matching is otherwise exact.
    pub fn parse(value: &str) -> Self {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "refreshed" => Self::Refreshed,
            "up-to-date" | "uptodate" => Self::UpToDate,
            "pruned" => Self::Pruned,
            "skipped" => Self::Skipped,
            "conflict" => Self::Conflict,
            "would-update" => Self::WouldUpdate,
            "would-skip" => Self::WouldSkip,
            "would-prune" => Self::WouldPrune,
            _ => Self::Unknown(value.trim().to_string()),
        }
    }

    /// The CLI's own word, suitable for a verbatim row label.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Refreshed => "refreshed",
            Self::UpToDate => "up-to-date",
            Self::Pruned => "pruned",
            Self::Skipped => "skipped",
            Self::Conflict => "conflict",
            Self::WouldUpdate => "would-update",
            Self::WouldSkip => "would-skip",
            Self::WouldPrune => "would-prune",
            Self::Unknown(value) => value,
        }
    }

    /// True for the three `--dry-run` preview words.
    pub fn is_preview(&self) -> bool {
        matches!(self, Self::WouldUpdate | Self::WouldSkip | Self::WouldPrune)
    }

    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_skill_installs_parse_items_and_tolerate_nulls() {
        let installs: Vec<TrackedSkillInstall> = serde_json::from_value(serde_json::json!([
            {
                "path": "/home/ada/.claude/skills/unity",
                "client": "claude-code",
                "skill": "unity-cli",
                "status": "installed"
            },
            {
                "path": null,
                "client": "cursor",
                "skill": null,
                "status": null,
                "futureField": true
            }
        ]))
        .expect("tracked installs parse");

        assert_eq!(installs.len(), 2);
        assert_eq!(installs[0].client, "claude-code");
        assert_eq!(installs[0].status, "installed");
        assert_eq!(installs[1].path, "");
        assert_eq!(installs[1].skill, "");
        assert_eq!(installs[1].status, "");
        assert!(installs[1].extra.contains_key("futureField"));
    }

    #[test]
    fn skill_clients_accept_boolean_or_status_and_keep_the_path() {
        let clients: Vec<SkillClientInfo> = serde_json::from_value(serde_json::json!([
            { "client": "claude-code", "path": "/home/ada/.claude/skills", "installed": true },
            { "id": "cursor", "path": "/home/ada/.cursor/skills", "status": "absent" },
            { "client": "codex", "path": "/home/ada/.codex/skills", "status": "installed" },
            { "client": "grok", "path": null, "installed": null }
        ]))
        .expect("clients parse");

        assert!(clients[0].is_installed());
        assert_eq!(clients[0].install_path(), "/home/ada/.claude/skills");
        assert!(!clients[1].is_installed());
        assert_eq!(clients[1].client, "cursor");
        assert!(clients[2].is_installed());
        assert!(!clients[3].is_installed());
        assert_eq!(clients[3].path, "");
    }

    #[test]
    fn refresh_rows_parse_every_documented_outcome_verbatim() {
        let rows: Vec<SkillRefreshRow> = serde_json::from_value(serde_json::json!([
            { "path": "/a", "client": "claude-code", "skill": "unity-cli", "status": "refreshed" },
            { "path": "/b", "client": "cursor", "skill": "unity-cli", "status": "up-to-date" },
            { "path": "/c", "client": "codex", "skill": "unity-cli", "status": "pruned", "skipReason": "The skill no longer exists upstream." },
            { "path": "/d", "client": "windsurf", "skill": "unity-cli", "status": "skipped", "skipReason": "The copy is older and --yes was not passed." },
            { "path": "/e", "client": "vscode", "skill": "unity-cli", "status": "conflict", "reason": "The CLI did not write this copy." },
            { "path": "/f", "client": "cline", "skill": "unity-cli", "status": "would-update" },
            { "path": "/g", "client": "grok", "skill": "unity-cli", "status": "would_skip" },
            { "path": "/h", "client": "opencode", "skill": "unity-cli", "outcome": "would-prune" }
        ]))
        .expect("refresh rows parse");

        let outcomes: Vec<SkillOutcome> = rows.iter().map(SkillRefreshRow::outcome).collect();
        assert_eq!(outcomes[0], SkillOutcome::Refreshed);
        assert_eq!(outcomes[1], SkillOutcome::UpToDate);
        assert_eq!(outcomes[2], SkillOutcome::Pruned);
        assert_eq!(outcomes[3], SkillOutcome::Skipped);
        assert_eq!(outcomes[4], SkillOutcome::Conflict);
        assert_eq!(outcomes[5], SkillOutcome::WouldUpdate);
        assert_eq!(outcomes[6], SkillOutcome::WouldSkip);
        assert_eq!(outcomes[7], SkillOutcome::WouldPrune);

        assert_eq!(
            rows[2].reason(),
            Some("The skill no longer exists upstream.")
        );
        assert_eq!(
            rows[3].reason(),
            Some("The copy is older and --yes was not passed.")
        );
        assert_eq!(rows[4].reason(), Some("The CLI did not write this copy."));
        assert_eq!(rows[0].reason(), None);
    }

    #[test]
    fn unknown_skill_outcomes_stay_verbatim_and_are_never_previews() {
        let outcome = SkillOutcome::parse("reinstalled");
        assert_eq!(outcome, SkillOutcome::Unknown("reinstalled".to_string()));
        assert_eq!(outcome.as_str(), "reinstalled");
        assert!(!outcome.is_preview());
        assert!(!outcome.is_conflict());

        assert_eq!(SkillOutcome::parse("").as_str(), "");
        assert!(SkillOutcome::WouldUpdate.is_preview());
        assert!(SkillOutcome::Conflict.is_conflict());
    }

    #[test]
    fn doctor_report_keeps_checks_and_tolerates_evolving_shapes() {
        let report: DoctorReport = serde_json::from_value(serde_json::json!({
            "platform": "linux",
            "arch": "x86_64",
            "runtimeVersion": "8.0.4",
            "nodeVersion": null,
            "cliVersion": { "version": "1.0.0-beta.17", "channel": "beta" },
            "installPath": "/home/ada/.unity",
            "logDirectory": "/home/ada/.unity/logs",
            "checks": [
                {
                    "id": "path-available",
                    "status": "pass",
                    "messageKey": "doctor.checks.pathAvailable",
                    "params": { "directory": "/home/ada/.local/bin" }
                },
                {
                    "id": "git-credential-helper",
                    "status": "warn",
                    "messageKey": null,
                    "params": null,
                    "futureField": true
                },
                { "status": "fail" }
            ],
            "auth": { "loggedIn": true },
            "license": { "active": false },
            "futureSection": { "anything": true }
        }))
        .expect("doctor report parses");

        assert_eq!(report.platform, "linux");
        assert_eq!(report.arch, "x86_64");
        assert_eq!(report.runtime_version.as_deref(), Some("8.0.4"));
        assert_eq!(report.node_version, None);
        assert_eq!(report.cli_version.as_deref(), Some("1.0.0-beta.17"));
        assert_eq!(report.checks.len(), 3);
        assert_eq!(report.checks[0].id, "path-available");
        assert_eq!(report.checks[0].status, "pass");
        assert_eq!(
            report.checks[0].message_key.as_deref(),
            Some("doctor.checks.pathAvailable")
        );
        assert_eq!(
            report.checks[0]
                .params
                .get("directory")
                .and_then(Value::as_str),
            Some("/home/ada/.local/bin")
        );
        assert!(report.checks[1].params.is_empty());
        assert_eq!(report.checks[1].status, "warn");
        // A check with no id is kept, so the UI can still show its status.
        assert!(report.checks[2].id.is_empty());
        assert!(report.auth.is_some());
        assert!(report.license.is_some());
        assert!(report.extra.contains_key("futureSection"));
    }

    #[test]
    fn doctor_report_accepts_a_bare_version_string() {
        let report: DoctorReport = serde_json::from_value(serde_json::json!({
            "cliVersion": "1.0.0-beta.17"
        }))
        .unwrap();
        assert_eq!(report.cli_version.as_deref(), Some("1.0.0-beta.17"));
        assert_eq!(report.platform, "");
        assert!(report.checks.is_empty());
    }

    #[test]
    fn prune_report_keeps_unused_and_missing_candidates() {
        let report: PruneReport = serde_json::from_value(serde_json::json!({
            "removed": false,
            "editors": [
                {
                    "version": "6000.3.27f2",
                    "architecture": "x86_64",
                    "path": "/home/ada/Unity/6000.3.27f2",
                    "bytes": 8688108322u64,
                    "status": "kept"
                },
                {
                    "version": null,
                    "architecture": null,
                    "path": null,
                    "bytes": null,
                    "reason": "No registered project uses this build."
                }
            ],
            "total": 8688108322u64,
            "missing": [
                {
                    "version": "2022.3.10f1",
                    "architecture": "arm64",
                    "path": "/gone/2022.3.10f1",
                    "size": "4.2 GB"
                }
            ]
        }))
        .expect("prune report parses");

        assert!(!report.removed);
        assert_eq!(report.editors.len(), 2);
        assert_eq!(report.editors[0].bytes, 8688108322);
        assert_eq!(report.editors[0].status.as_deref(), Some("kept"));
        assert_eq!(report.editors[1].version, "");
        assert_eq!(report.editors[1].bytes, 0);
        assert_eq!(
            report.editors[1].reason.as_deref(),
            Some("No registered project uses this build.")
        );
        assert_eq!(report.total, 8688108322);
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].size.as_deref(), Some("4.2 GB"));
    }

    #[test]
    fn verify_report_keeps_components_and_remediation() {
        let report: VerifyReport = serde_json::from_value(serde_json::json!({
            "version": "6000.6.6f1",
            "architecture": "arm64",
            "ok": false,
            "components": [
                {
                    "component": "editor",
                    "kind": "editor",
                    "status": "ok",
                    "path": "/home/ada/Unity/6000.6.6f1/Editor/Unity"
                },
                {
                    "component": "windows-mono",
                    "kind": "module",
                    "status": "missing",
                    "path": "/home/ada/Unity/6000.6.6f1/Editor/Data/PlaybackEngines/WindowsStandaloneSupport",
                    "command": "unity install-modules --editor-version 6000.6.6f1 --architecture arm64 -m windows-mono --yes"
                },
                {
                    "component": "iOS",
                    "status": "skipped",
                    "path": null
                }
            ]
        }))
        .expect("verify report parses");

        assert!(!report.ok);
        assert_eq!(report.components.len(), 3);
        assert_eq!(report.components[1].status, "missing");
        assert!(report.components[1].command.is_some());
        assert_eq!(report.components[2].status, "skipped");
        assert_eq!(report.components[2].path, None);
    }

    #[test]
    fn environment_reads_proxy_and_no_proxy_lists() {
        let info: EnvironmentInfo = serde_json::from_value(serde_json::json!({
            "userDataPath": "/home/ada/.config/Unity",
            "editorInstallPath": "/home/ada/Unity/Editors",
            "downloadCachePath": "/home/ada/Unity/Cache",
            "configPath": "/home/ada/.unity",
            "cliVersion": "1.0.0-beta.17",
            "hubVersion": "0.2.1",
            "proxy": {
                "url": "http://proxy.example:8080",
                "source": "env",
                "authSource": "keyring",
                "noProxy": "localhost,127.0.0.1 .unity.local"
            }
        }))
        .expect("env parses");

        assert_eq!(info.user_data_path, "/home/ada/.config/Unity");
        assert_eq!(info.hub_version.as_deref(), Some("0.2.1"));
        let proxy = info.proxy.expect("proxy");
        assert_eq!(proxy.url.as_deref(), Some("http://proxy.example:8080"));
        assert_eq!(proxy.source.as_deref(), Some("env"));
        assert_eq!(proxy.auth_source.as_deref(), Some("keyring"));
        assert_eq!(
            proxy.no_proxy,
            vec!["localhost", "127.0.0.1", ".unity.local"]
        );
    }

    #[test]
    fn environment_accepts_a_no_proxy_array_and_no_proxy() {
        let info: EnvironmentInfo = serde_json::from_value(serde_json::json!({
            "proxy": { "url": null, "noProxy": ["localhost", "*.local"] }
        }))
        .unwrap();
        let proxy = info.proxy.expect("proxy");
        assert_eq!(proxy.url, None);
        assert_eq!(proxy.no_proxy, vec!["localhost", "*.local"]);

        let info: EnvironmentInfo = serde_json::from_value(serde_json::json!({
            "proxy": null
        }))
        .unwrap();
        assert!(info.proxy.is_none());
        assert_eq!(info.user_data_path, "");
    }
}
