use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use unity_core::UnityError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub cli_path: Option<String>,
}

pub fn config_dir(app: &AppHandle) -> Result<PathBuf, UnityError> {
    app.path().app_config_dir().map_err(|err| {
        UnityError::new(
            "CONFIG_DIR_UNAVAILABLE",
            "Could not resolve the app config directory.",
        )
        .with_detail(err.to_string())
    })
}

fn settings_file(app: &AppHandle) -> Result<PathBuf, UnityError> {
    Ok(config_dir(app)?.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let Ok(file) = settings_file(app) else {
        return Settings::default();
    };
    let Ok(contents) = std::fs::read_to_string(file) else {
        return Settings::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), UnityError> {
    let dir = config_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(|err| {
        UnityError::new(
            "CONFIG_WRITE_FAILED",
            "Could not create the app config directory.",
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

/// The saved override, but only if the file still exists.
pub fn load_cli_path(app: &AppHandle) -> Option<PathBuf> {
    let settings = load(app);
    let path = PathBuf::from(settings.cli_path?);
    path.is_file().then_some(path)
}
