//! Shared application state managed by Tauri.

use std::sync::Mutex;

use fluxxx_core::http::ReqwestFetcher;
use fluxxx_core::rusqlite::Connection;

/// Held in Tauri's managed state. The SQLite connection is guarded by a `Mutex`
/// (rusqlite `Connection` is `Send` but not `Sync`); critical sections are short
/// and never span an `.await`.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub fetcher: ReqwestFetcher,
}
