use tauri::{AppHandle, State};
use unity_core::bootstrap;
use unity_core::UnityError;

use crate::jobs::JobStarted;
use crate::AppState;

#[tauri::command]
pub fn install_cli(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Option<String>,
) -> Result<JobStarted, UnityError> {
    let spec = bootstrap::install_cli_spec(channel.as_deref());
    state.jobs.start(app, "cli-install", &spec)
}

#[tauri::command]
pub fn update_cli(app: AppHandle, state: State<'_, AppState>) -> Result<JobStarted, UnityError> {
    let spec = state.cli.self_update_spec()?;
    state.jobs.start(app, "cli-update", &spec)
}

#[tauri::command]
pub fn install_editor(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
    modules: Vec<String>,
    accept_eula: bool,
) -> Result<JobStarted, UnityError> {
    let spec = state
        .cli
        .install_editor_spec(&version, &modules, accept_eula)?;
    state.jobs.start(app, "editor-install", &spec)
}

#[tauri::command]
pub fn install_modules(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
    modules: Vec<String>,
    accept_eula: bool,
) -> Result<JobStarted, UnityError> {
    let spec = state
        .cli
        .install_modules_spec(&version, &modules, accept_eula)?;
    state.jobs.start(app, "module-install", &spec)
}

#[tauri::command]
pub fn upgrade_project(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    version: String,
) -> Result<JobStarted, UnityError> {
    let spec = state.cli.upgrade_project_spec(&path, &version)?;
    state.jobs.start(app, "project-upgrade", &spec)
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>, job_id: String) -> Result<(), UnityError> {
    state.jobs.cancel(&job_id)
}
