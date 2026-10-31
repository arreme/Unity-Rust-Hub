use tauri::State;
use unity_core::models::{AuthStatus, CliStatus, CliUpdateInfo, LicenseStatus, RunningEditors};
use unity_core::UnityError;

use super::blocking;
use crate::AppState;

#[tauri::command]
pub async fn get_cli_status(state: State<'_, AppState>) -> Result<CliStatus, UnityError> {
    let cli = state.cli.clone();
    blocking(move || {
        let location = match cli.locate() {
            Ok(location) => location,
            Err(error) => {
                return Ok(CliStatus {
                    installed: false,
                    error: Some(error),
                    ..CliStatus::default()
                });
            }
        };

        let mut status = CliStatus {
            installed: true,
            path: Some(location.path.display().to_string()),
            path_source: Some(location.source.to_string()),
            ..CliStatus::default()
        };

        match cli.version() {
            Ok(version) => status.version = Some(version),
            Err(error) => status.error = Some(error),
        }
        if let Ok(path) = cli.install_path() {
            status.install_path = Some(path);
        }
        Ok(status)
    })
    .await
}

#[tauri::command]
pub async fn check_cli_update(state: State<'_, AppState>) -> Result<CliUpdateInfo, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.self_update_check()).await
}

#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<AuthStatus, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.auth_status()).await
}

#[tauri::command]
pub async fn get_license_status(state: State<'_, AppState>) -> Result<LicenseStatus, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.license_status()).await
}

#[tauri::command]
pub async fn get_install_path(state: State<'_, AppState>) -> Result<String, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.install_path()).await
}

#[tauri::command]
pub async fn list_running_editors(
    state: State<'_, AppState>,
) -> Result<RunningEditors, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.editors_running()).await
}
