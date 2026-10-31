use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use unity_core::UnityError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub cli_path: Option<String>,
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
    serde_json::from_str(&contents).unwrap_or_default()
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
    let contents = serde_json::to_string_pretty(settings).map_err(|err| {
        UnityError::new("CONFIG_WRITE_FAILED", "Could not serialize settings.")
            .with_detail(err.to_string())
    })?;
    std::fs::write(&file, contents).map_err(|err| {
        UnityError::new("CONFIG_WRITE_FAILED", "Could not write settings.")
            .with_detail(format!("{}: {err}", file.display()))
    })
}
