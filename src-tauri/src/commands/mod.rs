pub mod changelog;
pub mod cli;
pub mod editors;
pub mod jobs;
pub mod projects;
pub mod settings;

use unity_core::UnityError;

/// Runs a synchronous CLI call off the async runtime so network-bound reads do
/// not block the UI thread.
pub(crate) async fn blocking<T, F>(work: F) -> Result<T, UnityError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, UnityError> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(work).await {
        Ok(result) => result,
        Err(err) => Err(UnityError::new("INTERNAL", "A background CLI task failed.")
            .with_detail(err.to_string())),
    }
}
