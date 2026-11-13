use std::fmt;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use unity_core::UnityError;

const fn default_text_scale_percent() -> u16 {
    100
}

pub fn default_font_family() -> String {
    "Tahoma".to_string()
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
    #[serde(default = "default_font_family")]
    pub font_family: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            cli_path: None,
            text_scale_percent: default_text_scale_percent(),
            font_family: default_font_family(),
        }
    }
}

impl AppSettings {
    pub fn text_size(&self) -> TextSize {
        TextSize::from_percent(self.text_scale_percent)
    }

    pub fn normalized(mut self) -> Self {
        self.text_scale_percent = normalize_text_scale_percent(self.text_scale_percent);
        self.font_family = normalize_font_family(&self.font_family);
        self
    }
}

fn normalize_font_family(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        default_font_family()
    } else {
        value.to_string()
    }
}

/// Family names installed on this PC, sorted for a stable pick list.
///
/// iced resolves a `Font::with_name` family against the same operating-system
/// font database, so a name listed here is a font the app can actually draw.
pub fn font_families() -> Vec<String> {
    static FAMILIES: OnceLock<Vec<String>> = OnceLock::new();

    FAMILIES
        .get_or_init(|| {
            let mut database = fontdb::Database::new();
            database.load_system_fonts();

            let mut names: Vec<String> = database
                .faces()
                .flat_map(|face| face.families.iter())
                .map(|(name, _)| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .collect();

            names.sort_by_key(|name| name.to_lowercase());
            names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
            names
        })
        .clone()
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
    }
}
