use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};
use unity_core::UnityError;

use crate::settings as app_settings;
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub cli_path_override: Option<String>,
    pub config_dir: Option<String>,
    pub resolved_path: Option<String>,
    pub resolved_source: Option<String>,
}

fn view(app: &AppHandle, state: &AppState) -> SettingsView {
    let settings = app_settings::load(app);
    let config_dir = app_settings::config_dir(app)
        .ok()
        .map(|path| path.display().to_string());
    let (resolved_path, resolved_source) = match state.cli.locate() {
        Ok(location) => (
            Some(location.path.display().to_string()),
            Some(location.source.to_string()),
        ),
        Err(_) => (None, None),
    };
    SettingsView {
        cli_path_override: settings.cli_path,
        config_dir,
        resolved_path,
        resolved_source,
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<'_, AppState>) -> SettingsView {
    view(&app, &state)
}

#[tauri::command]
pub fn set_cli_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<SettingsView, UnityError> {
    let cleaned = path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut settings = app_settings::load(&app);
    match cleaned {
        Some(value) => {
            let path = PathBuf::from(&value);
            if !path.is_file() {
                return Err(UnityError::invalid_input(format!(
                    "`{value}` is not a file. Choose the `unity` executable."
                )));
            }
            let canonical = path.canonicalize().unwrap_or(path);
            settings.cli_path = Some(canonical.display().to_string());
            state.cli.set_override_path(Some(canonical));
        }
        None => {
            settings.cli_path = None;
            state.cli.set_override_path(None);
        }
    }

    app_settings::save(&app, &settings)?;
    Ok(view(&app, &state))
}
