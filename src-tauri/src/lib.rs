mod commands;
mod jobs;
mod settings;

use tauri::{Manager, WindowEvent};
use unity_core::UnityCli;

use jobs::JobManager;

pub struct AppState {
    pub cli: UnityCli,
    pub jobs: JobManager,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = AppState {
                cli: UnityCli::new(),
                jobs: JobManager::new(),
            };
            if let Some(path) = settings::load_cli_path(app.handle()) {
                state.cli.set_override_path(Some(path));
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::cli::get_cli_status,
            commands::cli::check_cli_update,
            commands::cli::get_auth_status,
            commands::cli::get_license_status,
            commands::cli::get_install_path,
            commands::cli::list_running_editors,
            commands::editors::list_editors,
            commands::editors::list_releases,
            commands::editors::list_modules,
            commands::editors::get_editor_info,
            commands::editors::dry_run_install,
            commands::projects::list_projects,
            commands::changelog::get_changelog,
            commands::jobs::install_cli,
            commands::jobs::update_cli,
            commands::jobs::install_editor,
            commands::jobs::install_modules,
            commands::jobs::upgrade_project,
            commands::jobs::cancel_job,
            commands::settings::get_settings,
            commands::settings::set_cli_path,
        ])
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                if let Some(state) = window.try_state::<AppState>() {
                    state.jobs.cancel_all();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Unity Rust Hub");
}
