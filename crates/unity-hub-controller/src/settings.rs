use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use unity_core::UnityError;

const fn default_text_scale_percent() -> u16 {
    100
}

/// A text-size preset persisted as a percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSize(u16);

impl TextSize {
    pub const ALL: [Self; 4] = [Self(100), Self(110), Self(125), Self(150)];

    pub fn from_percent(percent: u16) -> Self {
        Self(normalize_text_scale_percent(percent))
    }

    pub fn percent(self) -> u16 {
        self.0
    }

    pub fn scale(self) -> f32 {
        self.0 as f32 / 100.0
    }
}

impl Default for TextSize {
    fn default() -> Self {
        Self(100)
    }
}

impl fmt::Display for TextSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}

pub fn normalize_text_scale_percent(value: u16) -> u16 {
    TextSize::ALL
        .iter()
        .map(|preset| preset.0)
        .min_by_key(|preset| preset.abs_diff(value))
        .unwrap_or(100)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub cli_path: Option<String>,
    #[serde(default = "default_text_scale_percent")]
    pub text_scale_percent: u16,
    /// Dismissed advisory ids per CLI version. An advisory stays dismissed for
    /// the life of one CLI version; a new version shows it once again because
    /// the key no longer matches.
    #[serde(default)]
    pub dismissed_advisories: BTreeMap<String, Vec<String>>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            cli_path: None,
            text_scale_percent: default_text_scale_percent(),
            dismissed_advisories: BTreeMap::new(),
        }
    }
}

impl AppSettings {
    pub fn text_size(&self) -> TextSize {
        TextSize::from_percent(self.text_scale_percent)
    }

    pub fn normalized(mut self) -> Self {
        self.text_scale_percent = normalize_text_scale_percent(self.text_scale_percent);
        self
    }

    pub fn is_advisory_dismissed(&self, cli_version: &str, id: &str) -> bool {
        self.dismissed_advisories
            .get(cli_version)
            .is_some_and(|ids| ids.iter().any(|dismissed| dismissed == id))
    }

    /// Records a dismissal for the current CLI version and forgets older
    /// versions' records so the settings file stays small.
    pub fn dismiss_advisory(&mut self, cli_version: &str, id: &str) {
        self.keep_advisories_for(cli_version);
        let ids = self
            .dismissed_advisories
            .entry(cli_version.to_string())
            .or_default();
        if !ids.iter().any(|dismissed| dismissed == id) {
            ids.push(id.to_string());
        }
    }

    /// Keeps only `cli_version`'s dismissals. Returns whether anything changed
    /// so callers can skip pointless writes.
    pub fn keep_advisories_for(&mut self, cli_version: &str) -> bool {
        let before = self.dismissed_advisories.len();
        self.dismissed_advisories
            .retain(|version, _| version == cli_version);
        before != self.dismissed_advisories.len()
    }
}

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join("unity-rust-hub"))
}

fn settings_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("settings.json"))
}

pub fn load() -> AppSettings {
    let Some(file) = settings_file() else {
        return AppSettings::default();
    };
    let Ok(contents) = std::fs::read_to_string(file) else {
        return AppSettings::default();
    };
    serde_json::from_str::<AppSettings>(&contents)
        .unwrap_or_default()
        .normalized()
}

pub fn save(settings: &AppSettings) -> Result<(), UnityError> {
    let dir = config_dir().ok_or_else(|| {
        UnityError::new(
            "CONFIG_DIR_UNAVAILABLE",
            "Could not resolve a config directory.",
        )
    })?;
    std::fs::create_dir_all(&dir).map_err(|err| {
        UnityError::new(
            "CONFIG_WRITE_FAILED",
            "Could not create the config directory.",
        )
        .with_detail(format!("{}: {err}", dir.display()))
    })?;
    let file = dir.join("settings.json");
    let contents = serde_json::to_string_pretty(&settings.clone().normalized()).map_err(|err| {
        UnityError::new("CONFIG_WRITE_FAILED", "Could not serialize settings.")
            .with_detail(err.to_string())
    })?;
    std::fs::write(&file, contents).map_err(|err| {
        UnityError::new("CONFIG_WRITE_FAILED", "Could not write settings.")
            .with_detail(format!("{}: {err}", file.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_text_scale_is_100_percent() {
        assert_eq!(AppSettings::default().text_scale_percent, 100);
    }

    #[test]
    fn normalizes_text_scale_to_nearest_preset() {
        assert_eq!(normalize_text_scale_percent(100), 100);
        assert_eq!(normalize_text_scale_percent(112), 110);
        assert_eq!(normalize_text_scale_percent(120), 125);
        assert_eq!(normalize_text_scale_percent(140), 150);
        assert_eq!(normalize_text_scale_percent(500), 150);
        assert_eq!(normalize_text_scale_percent(0), 100);
    }

    #[test]
    fn text_size_displays_percent_and_scale() {
        let size = TextSize::from_percent(125);
        assert_eq!(size.percent(), 125);
        assert!((size.scale() - 1.25).abs() < f32::EPSILON);
        assert_eq!(size.to_string(), "125%");
    }

    #[test]
    fn deserializes_and_normalizes_text_scale() {
        let settings: AppSettings = serde_json::from_str(r#"{"textScalePercent": 130}"#).unwrap();
        assert_eq!(settings.normalized().text_scale_percent, 125);

        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.text_scale_percent, 100);
        assert!(settings.dismissed_advisories.is_empty());
    }

    #[test]
    fn dismissal_is_scoped_to_the_cli_version() {
        let mut settings = AppSettings::default();
        settings.dismiss_advisory("1.0.0-beta.16", "notification:CLI_UPDATE_AVAILABLE");

        assert!(
            settings.is_advisory_dismissed("1.0.0-beta.16", "notification:CLI_UPDATE_AVAILABLE")
        );
        assert!(
            !settings.is_advisory_dismissed("1.0.0-beta.17", "notification:CLI_UPDATE_AVAILABLE")
        );
        assert!(!settings.is_advisory_dismissed("1.0.0-beta.16", "warning:Beta plugin."));
    }

    #[test]
    fn dismissing_prunes_older_cli_versions() {
        let mut settings = AppSettings::default();
        settings.dismiss_advisory("1.0.0-beta.16", "advisory-a");
        settings.dismiss_advisory("1.0.0-beta.16", "advisory-a");
        settings.dismiss_advisory("1.0.0-beta.17", "advisory-b");

        assert_eq!(settings.dismissed_advisories.len(), 1);
        let ids = settings.dismissed_advisories.get("1.0.0-beta.17").unwrap();
        assert_eq!(ids, &vec!["advisory-b".to_string()]);
    }

    #[test]
    fn keeping_a_version_reports_whether_it_changed_anything() {
        let mut settings = AppSettings::default();
        settings.dismiss_advisory("1.0.0-beta.16", "advisory-a");

        assert!(settings.keep_advisories_for("1.0.0-beta.17"));
        assert!(settings.dismissed_advisories.is_empty());
        assert!(!settings.keep_advisories_for("1.0.0-beta.17"));
    }

    #[test]
    fn dismissals_round_trip_through_settings_json() {
        let mut settings = AppSettings::default();
        settings.dismiss_advisory("1.0.0-beta.17", "warning:Beta plugin.");

        let json = serde_json::to_string(&settings).unwrap();
        let restored: AppSettings = serde_json::from_str(&json).unwrap();

        assert!(restored.is_advisory_dismissed("1.0.0-beta.17", "warning:Beta plugin."));
    }
}
