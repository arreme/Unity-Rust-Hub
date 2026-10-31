use serde::Serialize;
use tauri::State;
use unity_core::changelog::{
    parse_changelog, BreakingChange, ChangelogProvider, ChangelogRequest, ChangelogSection,
    CliChangelogProvider,
};
use unity_core::UnityError;

use super::blocking;
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogView {
    pub version: String,
    pub sections: Vec<ChangelogSection>,
    pub breaking_changes: Vec<BreakingChange>,
    pub entry_count: usize,
    pub markdown: String,
    /// Active provider id. v1 is always `unity-changelog`; a remote JSON
    /// provider can be added without changing the UI contract.
    pub source: String,
    pub source_detail: String,
}

#[tauri::command]
pub async fn get_changelog(
    state: State<'_, AppState>,
    target: Option<String>,
) -> Result<ChangelogView, UnityError> {
    let cli = state.cli.clone();
    blocking(move || {
        let provider = CliChangelogProvider::new(cli);
        let request = ChangelogRequest { target };
        let raw = provider.fetch(&request)?;
        let parsed = parse_changelog(&raw.changelog);
        let version = parsed
            .version
            .clone()
            .unwrap_or_else(|| raw.version.clone());
        let entry_count = parsed.entry_count();
        let source_detail = provider.source_detail(&request, &raw);
        Ok(ChangelogView {
            version,
            sections: parsed.sections,
            breaking_changes: parsed.breaking_changes,
            entry_count,
            markdown: raw.changelog,
            source: provider.source_id().to_string(),
            source_detail,
        })
    })
    .await
}
