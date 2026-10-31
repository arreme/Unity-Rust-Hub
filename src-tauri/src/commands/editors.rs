use serde_json::Value;
use tauri::State;
use unity_core::models::{Editor, EditorInfo, Module, Release};
use unity_core::UnityError;

use super::blocking;
use crate::AppState;

#[tauri::command]
pub async fn list_editors(state: State<'_, AppState>) -> Result<Vec<Editor>, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.editors_installed()).await
}

#[tauri::command]
pub async fn list_releases(
    state: State<'_, AppState>,
    limit: Option<u32>,
    skip: Option<u32>,
    stream: Option<String>,
) -> Result<Vec<Release>, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.releases(limit.unwrap_or(25), skip.unwrap_or(0), stream.as_deref())).await
}

#[tauri::command]
pub async fn list_modules(
    state: State<'_, AppState>,
    version: String,
) -> Result<Vec<Module>, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.modules(&version)).await
}

#[tauri::command]
pub async fn get_editor_info(
    state: State<'_, AppState>,
    version: String,
) -> Result<EditorInfo, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.editor_info(&version)).await
}

#[tauri::command]
pub async fn dry_run_install(
    state: State<'_, AppState>,
    version: String,
    modules: Vec<String>,
) -> Result<Value, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.dry_run_install(&version, &modules)).await
}
