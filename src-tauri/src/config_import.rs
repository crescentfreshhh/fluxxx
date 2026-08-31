//! Discovery, import, and export of the external credentials file
//! (`fluxxx-providers.toml`). Parsing lives in `fluxxx_core::config`; this module
//! adds file location, DPAPI encryption, and DB insertion.
//!
//! Import is idempotent: a provider is added only if none already matches its
//! normalized host + username, so the file can stay in place across launches and
//! newly appended entries are picked up without creating duplicates.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fluxxx_core::config::{self, ProviderSeed};
use fluxxx_core::db::{self, NewProvider};
use fluxxx_core::rusqlite::Connection;
use fluxxx_core::xtream;

use crate::crypto;

const FILE_NAME: &str = "fluxxx-providers.toml";

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Locate the credentials file: next to the executable first, then the app data
/// dir. Returns the first that exists.
pub fn find_provider_file(app_data_dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = exe_dir() {
        candidates.push(dir.join(FILE_NAME));
    }
    candidates.push(app_data_dir.join(FILE_NAME));
    candidates.into_iter().find(|p| p.is_file())
}

/// Import providers from the credentials file, if present. Returns how many new
/// providers were added.
pub fn import_from_file(conn: &Connection, app_data_dir: &Path) -> Result<usize, String> {
    let Some(path) = find_provider_file(app_data_dir) else {
        return Ok(0);
    };
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let seeds = config::parse_providers_toml(&contents)?;
    import_seeds(conn, seeds)
}

fn import_seeds(conn: &Connection, seeds: Vec<ProviderSeed>) -> Result<usize, String> {
    let existing = db::list_providers(conn).map_err(|e| e.to_string())?;
    let mut present: HashSet<(String, String)> = existing
        .iter()
        .map(|p| (p.host.clone(), p.username.clone()))
        .collect();

    let mut added = 0usize;
    for seed in seeds {
        let host = xtream::normalize_base(&seed.host, seed.port);
        let key = (host.clone(), seed.username.clone());
        if present.contains(&key) {
            continue;
        }
        let password_enc = crypto::encrypt(seed.password.as_bytes())?;
        let id = db::insert_provider(
            conn,
            &NewProvider {
                name: seed.name,
                host,
                port: seed.port,
                username: seed.username.clone(),
                password_enc,
                created_at: now(),
            },
        )
        .map_err(|e| e.to_string())?;
        if !seed.enabled {
            db::set_provider_enabled(conn, id, false).map_err(|e| e.to_string())?;
        }
        present.insert(key);
        added += 1;
    }
    Ok(added)
}

/// Write all current providers (decrypted) to the credentials file. Prefers the
/// executable's directory; falls back to the app data dir if that is not
/// writable (e.g. an installed build under Program Files). Returns the path used.
pub fn export_to_file(conn: &Connection, app_data_dir: &Path) -> Result<PathBuf, String> {
    let providers = db::list_providers(conn).map_err(|e| e.to_string())?;
    let mut seeds = Vec::with_capacity(providers.len());
    for p in providers {
        let pw = crypto::decrypt(&p.password_enc)?;
        let password = String::from_utf8(pw).map_err(|_| "stored password is not valid UTF-8")?;
        seeds.push(ProviderSeed {
            name: p.name,
            host: p.host,
            port: p.port,
            username: p.username,
            password,
            enabled: p.enabled,
        });
    }
    let text = config::serialize_providers_toml(&seeds);

    let primary = exe_dir()
        .map(|d| d.join(FILE_NAME))
        .unwrap_or_else(|| app_data_dir.join(FILE_NAME));
    if std::fs::write(&primary, &text).is_ok() {
        return Ok(primary);
    }
    let fallback = app_data_dir.join(FILE_NAME);
    std::fs::write(&fallback, &text).map_err(|e| e.to_string())?;
    Ok(fallback)
}
