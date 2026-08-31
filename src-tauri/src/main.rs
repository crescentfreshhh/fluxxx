// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;

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

/// Preview of Phase 2 curation: infer a country from a category name. Wired now
/// so the core crate is exercised end-to-end through the Tauri boundary.
#[tauri::command]
fn infer_country(name: String) -> Option<CountryDto> {
    fluxxx_core::infer_country(&name).map(|c| CountryDto {
        code: c.code.to_string(),
        name: c.name.to_string(),
    })
}

#[derive(Serialize)]
struct CountryDto {
    code: String,
    name: String,
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info, infer_country])
        .run(tauri::generate_context!())
        .expect("error while running fluxxx");
}
