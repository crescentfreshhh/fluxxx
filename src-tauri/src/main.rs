// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config_import;
mod crypto;
mod state;

use std::sync::Mutex;

use fluxxx_core::http::ReqwestFetcher;
use serde::Serialize;
use tauri::Manager;

use state::AppState;

/// Basic app identity, surfaced to the UI to prove the IPC bridge is alive.
#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "fluxxx",
        version: env!("CARGO_PKG_VERSION"),
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Open (or create) the catalog database under the per-user app data dir.
            let dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&dir).ok();
            let db_path = dir.join("fluxxx.db");
            let conn = fluxxx_core::db::open(db_path.to_str().expect("db path not utf-8"))
                .expect("failed to open database");

            // Preload providers from an optional credentials file next to the exe
            // (or in the app data dir). Idempotent and best-effort.
            match config_import::import_from_file(&conn, &dir) {
                Ok(0) => {}
                Ok(n) => eprintln!("fluxxx: imported {n} provider(s) from credentials file"),
                Err(e) => eprintln!("fluxxx: credentials-file import failed: {e}"),
            }

            app.manage(AppState {
                db: Mutex::new(conn),
                fetcher: ReqwestFetcher::new(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::list_providers,
            commands::add_provider,
            commands::set_provider_enabled,
            commands::delete_provider,
            commands::test_connection,
            commands::sync_provider,
            commands::list_categories,
            commands::curation_summary,
            commands::curation_stats,
            commands::set_category_enabled,
            commands::set_all_categories_enabled,
            commands::set_country_enabled,
            commands::set_categories_enabled,
            commands::list_channels,
            commands::list_active_categories,
            commands::list_recent,
            commands::set_favorite,
            commands::record_recent,
            commands::get_setting,
            commands::set_setting,
            commands::sync_epg,
            commands::get_epg,
            commands::stream_url,
            commands::launch_external,
            commands::import_providers_file,
            commands::export_providers_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running fluxxx");
}
