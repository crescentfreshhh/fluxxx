//! Tauri commands exposed to the frontend: provider management, connection
//! testing, catalog sync, and curation toggles.
//!
//! Network calls (test/sync) fetch first, then take the DB lock for a short
//! synchronous write — the `Mutex<Connection>` guard never crosses an `.await`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fluxxx_core::client::{self, Creds};
use fluxxx_core::curation::{self, CountryGroup};
use fluxxx_core::db::{self, CategoryRow, NewProvider, ProviderRow};
use fluxxx_core::model::Category;
use fluxxx_core::xtream::XtreamEpgEntry;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::crypto;
use crate::state::AppState;

/// Max concurrent EPG requests in flight during a sync.
const EPG_CONCURRENCY: usize = 12;

type CmdResult<T> = Result<T, String>;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Ensure the host carries a scheme and no trailing slash.
fn normalize_host(input: &str) -> String {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

// --- DTOs --------------------------------------------------------------------

#[derive(Serialize)]
pub struct ProviderDto {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub enabled: bool,
    pub created_at: i64,
    pub last_synced_at: Option<i64>,
}

impl From<ProviderRow> for ProviderDto {
    fn from(p: ProviderRow) -> Self {
        ProviderDto {
            id: p.id,
            name: p.name,
            host: p.host,
            port: p.port,
            username: p.username,
            enabled: p.enabled,
            created_at: p.created_at,
            last_synced_at: p.last_synced_at,
        }
    }
}

#[derive(Deserialize)]
pub struct AddProviderInput {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub status: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct SyncResult {
    pub categories: usize,
    pub channels: usize,
}

#[derive(Serialize)]
pub struct CategoryDto {
    pub id: i64,
    pub name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub enabled: bool,
    pub channel_count: i64,
}

impl From<CategoryRow> for CategoryDto {
    fn from(c: CategoryRow) -> Self {
        let country_name = c
            .country_code
            .as_deref()
            .map(|code| fluxxx_core::country::name_for_code(code).to_string());
        CategoryDto {
            id: c.id,
            name: c.name,
            country_code: c.country_code,
            country_name,
            enabled: c.enabled,
            channel_count: c.channel_count,
        }
    }
}

#[derive(Serialize)]
pub struct CountryGroupDto {
    pub code: Option<String>,
    pub name: String,
    pub channel_count: usize,
    pub enabled_categories: usize,
    pub total_categories: usize,
    pub fully_enabled: bool,
}

impl From<CountryGroup> for CountryGroupDto {
    fn from(g: CountryGroup) -> Self {
        CountryGroupDto {
            fully_enabled: g.fully_enabled(),
            code: g.code,
            name: g.name,
            channel_count: g.channel_count,
            enabled_categories: g.enabled_categories,
            total_categories: g.total_categories,
        }
    }
}

// --- helpers -----------------------------------------------------------------

/// Build plaintext creds from a stored provider row (decrypting the password).
fn creds_from_row(p: &ProviderRow) -> CmdResult<Creds> {
    let pw = crypto::decrypt(&p.password_enc)?;
    let password = String::from_utf8(pw).map_err(|_| "stored password is not valid UTF-8")?;
    Ok(Creds {
        base_url: format!("{}:{}", p.host, p.port),
        username: p.username.clone(),
        password,
    })
}

// --- provider CRUD -----------------------------------------------------------

#[tauri::command]
pub fn list_providers(state: tauri::State<'_, AppState>) -> CmdResult<Vec<ProviderDto>> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    let rows = db::list_providers(&conn).map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(ProviderDto::from).collect())
}

#[tauri::command]
pub fn add_provider(
    state: tauri::State<'_, AppState>,
    input: AddProviderInput,
) -> CmdResult<ProviderDto> {
    let enc = crypto::encrypt(input.password.as_bytes())?;
    let new = NewProvider {
        name: input.name.trim().to_string(),
        host: normalize_host(&input.host),
        port: input.port,
        username: input.username.trim().to_string(),
        password_enc: enc,
        created_at: now(),
    };
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    let id = db::insert_provider(&conn, &new).map_err(|e| e.to_string())?;
    let row = db::get_provider(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or("provider vanished after insert")?;
    Ok(row.into())
}

#[tauri::command]
pub fn set_provider_enabled(
    state: tauri::State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> CmdResult<()> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::set_provider_enabled(&conn, id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_provider(state: tauri::State<'_, AppState>, id: i64) -> CmdResult<()> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::delete_provider(&conn, id).map_err(|e| e.to_string())
}

// --- network: test + sync ----------------------------------------------------

#[tauri::command]
pub async fn test_connection(
    state: tauri::State<'_, AppState>,
    input: AddProviderInput,
) -> CmdResult<TestResult> {
    let creds = Creds {
        base_url: format!("{}:{}", normalize_host(&input.host), input.port),
        username: input.username.trim().to_string(),
        password: input.password.clone(),
    };
    match client::authenticate(&state.fetcher, &creds).await {
        Ok(auth) => Ok(TestResult {
            ok: auth.authenticated,
            status: auth.status.clone(),
            message: if auth.authenticated {
                "Connected".to_string()
            } else {
                format!("Rejected{}", auth.status.map(|s| format!(" ({s})")).unwrap_or_default())
            },
        }),
        Err(e) => Ok(TestResult {
            ok: false,
            status: None,
            message: e.to_string(),
        }),
    }
}

#[tauri::command]
pub async fn sync_provider(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> CmdResult<SyncResult> {
    // 1) Read creds under a short lock, then release before any network I/O.
    let creds = {
        let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
        let row = db::get_provider(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or("provider not found")?;
        creds_from_row(&row)?
    };

    // 2) Fetch catalog (no lock held).
    let categories = client::fetch_categories(&state.fetcher, &creds)
        .await
        .map_err(|e| format!("categories: {e}"))?;
    let streams = client::fetch_streams(&state.fetcher, &creds)
        .await
        .map_err(|e| format!("streams: {e}"))?;

    // 3) Write under the lock.
    let counts = {
        let mut conn = state.db.lock().map_err(|_| "db lock poisoned")?;
        let counts = db::apply_catalog(&mut *conn, id, &categories, &streams)
            .map_err(|e| e.to_string())?;
        db::set_last_synced(&conn, id, now()).map_err(|e| e.to_string())?;
        counts
    };

    Ok(SyncResult {
        categories: counts.categories,
        channels: counts.channels,
    })
}

// --- curation ----------------------------------------------------------------

#[tauri::command]
pub fn list_categories(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
) -> CmdResult<Vec<CategoryDto>> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    let rows = db::list_categories(&conn, provider_id).map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(CategoryDto::from).collect())
}

#[tauri::command]
pub fn curation_summary(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
) -> CmdResult<Vec<CountryGroupDto>> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    let rows = db::list_categories(&conn, provider_id).map_err(|e| e.to_string())?;
    drop(conn);

    let mut counts = std::collections::HashMap::new();
    let cats: Vec<Category> = rows
        .into_iter()
        .map(|r| {
            counts.insert(r.id, r.channel_count as usize);
            Category {
                id: r.id,
                provider_id: r.provider_id,
                xtream_category_id: r.xtream_category_id,
                name: r.name,
                country_code: r.country_code,
                enabled: r.enabled,
            }
        })
        .collect();

    let groups = curation::group_by_country(&cats, &counts);
    Ok(groups.into_iter().map(CountryGroupDto::from).collect())
}

#[tauri::command]
pub fn set_category_enabled(
    state: tauri::State<'_, AppState>,
    category_id: i64,
    enabled: bool,
) -> CmdResult<()> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::set_category_enabled(&conn, category_id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_country_enabled(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
    country_code: Option<String>,
    enabled: bool,
) -> CmdResult<usize> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::set_country_enabled(&conn, provider_id, country_code.as_deref(), enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_all_categories_enabled(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
    enabled: bool,
) -> CmdResult<usize> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::set_all_categories_enabled(&conn, provider_id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn curation_stats(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
) -> CmdResult<db::CurationStats> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::curation_stats(&conn, provider_id).map_err(|e| e.to_string())
}

// --- channels: browse, favorites, recent, resume -----------------------------

#[tauri::command]
pub fn list_channels(
    state: tauri::State<'_, AppState>,
    provider_id: Option<i64>,
    search: Option<String>,
    favorites_only: bool,
    limit: Option<i64>,
) -> CmdResult<Vec<db::ChannelRow>> {
    let query = db::ChannelQuery {
        provider_id,
        search,
        favorites_only,
        limit: limit.unwrap_or(500),
        offset: 0,
    };
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::list_channels(&conn, &query).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_recent(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> CmdResult<Vec<db::ChannelRow>> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::list_recent(&conn, limit.unwrap_or(20)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_favorite(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
    stream_id: i64,
    favorite: bool,
) -> CmdResult<()> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::set_favorite(&conn, provider_id, stream_id, favorite, now()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn record_recent(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
    stream_id: i64,
) -> CmdResult<()> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::record_recent(&conn, provider_id, stream_id, now()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_setting(state: tauri::State<'_, AppState>, key: String) -> CmdResult<Option<String>> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::get_setting(&conn, &key).map_err(|e| e.to_string())
}

// --- EPG ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct EpgSyncResult {
    pub channels_fetched: usize,
    pub programs: usize,
}

#[derive(Serialize, Clone)]
struct EpgProgress {
    done: usize,
    total: usize,
}

/// Fetch EPG for a provider's active channels (curation-gated) with bounded
/// concurrency, emitting `epg-progress` events, then replace the cache. Only
/// channels that carry an `epg_channel_id` are fetched — the rest have no guide.
#[tauri::command]
pub async fn sync_epg(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    provider_id: i64,
) -> CmdResult<EpgSyncResult> {
    // 1) Creds + the active channels worth fetching (short lock).
    let (creds, channels): (Creds, Vec<i64>) = {
        let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
        let row = db::get_provider(&conn, provider_id)
            .map_err(|e| e.to_string())?
            .ok_or("provider not found")?;
        let creds = creds_from_row(&row)?;
        let chans = db::list_channels(
            &conn,
            &db::ChannelQuery {
                provider_id: Some(provider_id),
                limit: 0,
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        let ids = chans
            .into_iter()
            .filter(|c| c.epg_channel_id.as_deref().map(|s| !s.is_empty()).unwrap_or(false))
            .map(|c| c.stream_id)
            .collect();
        (creds, ids)
    };

    let total = channels.len();
    let done = AtomicUsize::new(0);

    // 2) Concurrent fetch (no lock held). buffer_unordered keeps N in flight.
    let results: Vec<(i64, Vec<XtreamEpgEntry>)> = stream::iter(channels.into_iter())
        .map(|stream_id| {
            let creds = &creds;
            let fetcher = &state.fetcher;
            let done = &done;
            let app = &app;
            async move {
                let entries = client::fetch_epg(fetcher, creds, stream_id)
                    .await
                    .unwrap_or_default();
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 25 == 0 || n == total {
                    let _ = app.emit("epg-progress", EpgProgress { done: n, total });
                }
                (stream_id, entries)
            }
        })
        .buffer_unordered(EPG_CONCURRENCY)
        .collect()
        .await;

    // 3) Replace cache under the lock.
    let programs = {
        let mut conn = state.db.lock().map_err(|_| "db lock poisoned")?;
        let n = db::replace_epg_for_streams(&mut *conn, provider_id, &results)
            .map_err(|e| e.to_string())?;
        db::set_setting(&conn, &format!("last_epg_sync:{provider_id}"), &now().to_string())
            .map_err(|e| e.to_string())?;
        n
    };

    Ok(EpgSyncResult {
        channels_fetched: total,
        programs,
    })
}

#[tauri::command]
pub fn get_epg(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
    from: i64,
    to: i64,
) -> CmdResult<Vec<db::EpgProgramRow>> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::get_epg_window(&conn, provider_id, from, to).map_err(|e| e.to_string())
}

// --- playback ----------------------------------------------------------------

/// Build a playable live stream URL for a channel. `container` defaults to
/// `m3u8` (HLS, played in-webview via hls.js); pass `ts` for the raw transport
/// stream. Credentials are decrypted only to assemble the URL.
#[tauri::command]
pub fn stream_url(
    state: tauri::State<'_, AppState>,
    provider_id: i64,
    stream_id: i64,
    container: Option<String>,
) -> CmdResult<String> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    let row = db::get_provider(&conn, provider_id)
        .map_err(|e| e.to_string())?
        .ok_or("provider not found")?;
    drop(conn);
    let creds = creds_from_row(&row)?;
    let container = container.unwrap_or_else(|| "m3u8".to_string());
    Ok(fluxxx_core::xtream::stream_url(
        &creds.base_url,
        &creds.username,
        &creds.password,
        stream_id,
        &container,
    ))
}

#[tauri::command]
pub fn set_setting(state: tauri::State<'_, AppState>, key: String, value: String) -> CmdResult<()> {
    let conn = state.db.lock().map_err(|_| "db lock poisoned")?;
    db::set_setting(&conn, &key, &value).map_err(|e| e.to_string())
}
