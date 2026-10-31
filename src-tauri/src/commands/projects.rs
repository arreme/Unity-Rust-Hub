use tauri::State;
use unity_core::models::Project;
use unity_core::UnityError;

use super::blocking;
use crate::AppState;

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, UnityError> {
    let cli = state.cli.clone();
    blocking(move || cli.projects()).await
}
