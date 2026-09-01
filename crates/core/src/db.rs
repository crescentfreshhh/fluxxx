//! SQLite persistence for the catalog (providers, categories, channels) and
//! settings. Pure enough to run against an in-memory database in unit tests on
//! any platform.
//!
//! Credentials are stored as an opaque encrypted blob (`password_enc`) — this
//! layer never encrypts or decrypts; that is the app layer's job (Windows DPAPI).
//! Re-syncing a provider preserves user curation: existing categories keep their
//! `enabled` flag; only new categories default to enabled.

use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};

use crate::xtream::{XtreamCategory, XtreamEpgEntry, XtreamStream};

/// A stored provider row. `password_enc` is opaque ciphertext.
#[derive(Debug, Clone)]
pub struct ProviderRow {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password_enc: Vec<u8>,
    pub enabled: bool,
    pub created_at: i64,
    pub last_synced_at: Option<i64>,
}

/// Fields needed to create a provider.
#[derive(Debug, Clone)]
pub struct NewProvider {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password_enc: Vec<u8>,
    pub created_at: i64,
}

/// Result of applying a fetched catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogCounts {
    pub categories: usize,
    pub channels: usize,
}

pub type Result<T> = std::result::Result<T, rusqlite::Error>;

/// Open (or create) the database at `path` and initialize the schema.
pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    init_schema(&conn)?;
    Ok(conn)
}

/// Open an in-memory database (used by tests).
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    init_schema(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS providers (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            name           TEXT NOT NULL,
            host           TEXT NOT NULL,
            port           INTEGER NOT NULL,
            username       TEXT NOT NULL,
            password_enc   BLOB NOT NULL,
            enabled        INTEGER NOT NULL DEFAULT 1,
            created_at     INTEGER NOT NULL,
            last_synced_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS categories (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            provider_id        INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
            xtream_category_id TEXT NOT NULL,
            name               TEXT NOT NULL,
            country_code       TEXT,
            enabled            INTEGER NOT NULL DEFAULT 1,
            UNIQUE(provider_id, xtream_category_id)
        );

        CREATE TABLE IF NOT EXISTS channels (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            provider_id    INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
            stream_id      INTEGER NOT NULL,
            name           TEXT NOT NULL,
            category_id    INTEGER REFERENCES categories(id) ON DELETE SET NULL,
            epg_channel_id TEXT,
            logo           TEXT,
            num            INTEGER,
            UNIQUE(provider_id, stream_id)
        );

        CREATE INDEX IF NOT EXISTS idx_channels_provider ON channels(provider_id);
        CREATE INDEX IF NOT EXISTS idx_channels_category ON channels(category_id);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS favorites (
            provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
            stream_id   INTEGER NOT NULL,
            created_at  INTEGER NOT NULL,
            PRIMARY KEY (provider_id, stream_id)
        );

        CREATE TABLE IF NOT EXISTS recent (
            provider_id      INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
            stream_id        INTEGER NOT NULL,
            last_watched_utc INTEGER NOT NULL,
            PRIMARY KEY (provider_id, stream_id)
        );

        CREATE TABLE IF NOT EXISTS epg_programs (
            provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
            stream_id   INTEGER NOT NULL,
            start_utc   INTEGER NOT NULL,
            stop_utc    INTEGER NOT NULL,
            title       TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (provider_id, stream_id, start_utc)
        );

        CREATE INDEX IF NOT EXISTS idx_epg_window
            ON epg_programs(provider_id, stream_id, start_utc, stop_utc);
        "#,
    )?;
    Ok(())
}

// --- providers ---------------------------------------------------------------

pub fn insert_provider(conn: &Connection, p: &NewProvider) -> Result<i64> {
    conn.execute(
        "INSERT INTO providers (name, host, port, username, password_enc, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
        params![p.name, p.host, p.port, p.username, p.password_enc, p.created_at],
    )?;
    Ok(conn.last_insert_rowid())
}

fn row_to_provider(row: &rusqlite::Row) -> Result<ProviderRow> {
    Ok(ProviderRow {
        id: row.get("id")?,
        name: row.get("name")?,
        host: row.get("host")?,
        port: row.get::<_, i64>("port")? as u16,
        username: row.get("username")?,
        password_enc: row.get("password_enc")?,
        enabled: row.get::<_, i64>("enabled")? != 0,
        created_at: row.get("created_at")?,
        last_synced_at: row.get("last_synced_at")?,
    })
}

pub fn list_providers(conn: &Connection) -> Result<Vec<ProviderRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password_enc, enabled, created_at, last_synced_at
         FROM providers ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([], row_to_provider)?;
    rows.collect()
}

pub fn get_provider(conn: &Connection, id: i64) -> Result<Option<ProviderRow>> {
    conn.query_row(
        "SELECT id, name, host, port, username, password_enc, enabled, created_at, last_synced_at
         FROM providers WHERE id = ?1",
        params![id],
        row_to_provider,
    )
    .optional()
}

pub fn set_provider_enabled(conn: &Connection, id: i64, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE providers SET enabled = ?2 WHERE id = ?1",
        params![id, enabled as i64],
    )?;
    Ok(())
}

pub fn set_last_synced(conn: &Connection, id: i64, ts: i64) -> Result<()> {
    conn.execute(
        "UPDATE providers SET last_synced_at = ?2 WHERE id = ?1",
        params![id, ts],
    )?;
    Ok(())
}

pub fn delete_provider(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
    Ok(())
}

// --- categories --------------------------------------------------------------

/// A category as stored, with its inferred country and enabled flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryRow {
    pub id: i64,
    pub provider_id: i64,
    pub xtream_category_id: String,
    pub name: String,
    pub country_code: Option<String>,
    pub enabled: bool,
    pub channel_count: i64,
}

pub fn list_categories(conn: &Connection, provider_id: i64) -> Result<Vec<CategoryRow>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.provider_id, c.xtream_category_id, c.name, c.country_code, c.enabled,
                (SELECT COUNT(*) FROM channels ch WHERE ch.category_id = c.id) AS channel_count
         FROM categories c
         WHERE c.provider_id = ?1
         ORDER BY c.name ASC",
    )?;
    let rows = stmt.query_map(params![provider_id], |row| {
        Ok(CategoryRow {
            id: row.get("id")?,
            provider_id: row.get("provider_id")?,
            xtream_category_id: row.get("xtream_category_id")?,
            name: row.get("name")?,
            country_code: row.get("country_code")?,
            enabled: row.get::<_, i64>("enabled")? != 0,
            channel_count: row.get("channel_count")?,
        })
    })?;
    rows.collect()
}

pub fn set_category_enabled(conn: &Connection, category_id: i64, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE categories SET enabled = ?2 WHERE id = ?1",
        params![category_id, enabled as i64],
    )?;
    Ok(())
}

/// Enable/disable every category of a provider at once (bulk wizard action).
/// Returns the number of categories affected.
pub fn set_all_categories_enabled(
    conn: &Connection,
    provider_id: i64,
    enabled: bool,
) -> Result<usize> {
    let n = conn.execute(
        "UPDATE categories SET enabled = ?2 WHERE provider_id = ?1",
        params![provider_id, enabled as i64],
    )?;
    Ok(n)
}

/// Aggregate curation stats for one provider, used by the wizard header. A
/// channel counts as "enabled" when its category is enabled or it has no
/// category (matching [`crate::curation::epg_fetch_targets`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CurationStats {
    pub total_categories: i64,
    pub enabled_categories: i64,
    pub total_channels: i64,
    pub enabled_channels: i64,
}

pub fn curation_stats(conn: &Connection, provider_id: i64) -> Result<CurationStats> {
    let total_categories: i64 = conn.query_row(
        "SELECT COUNT(*) FROM categories WHERE provider_id = ?1",
        params![provider_id],
        |r| r.get(0),
    )?;
    let enabled_categories: i64 = conn.query_row(
        "SELECT COUNT(*) FROM categories WHERE provider_id = ?1 AND enabled = 1",
        params![provider_id],
        |r| r.get(0),
    )?;
    let total_channels: i64 = conn.query_row(
        "SELECT COUNT(*) FROM channels WHERE provider_id = ?1",
        params![provider_id],
        |r| r.get(0),
    )?;
    let enabled_channels: i64 = conn.query_row(
        "SELECT COUNT(*) FROM channels ch
         WHERE ch.provider_id = ?1
           AND (ch.category_id IS NULL
                OR EXISTS (SELECT 1 FROM categories c
                           WHERE c.id = ch.category_id AND c.enabled = 1))",
        params![provider_id],
        |r| r.get(0),
    )?;
    Ok(CurationStats {
        total_categories,
        enabled_categories,
        total_channels,
        enabled_channels,
    })
}

/// Toggle every category of a provider that maps to `country_code` (use `None`
/// for the "Other" bucket). Returns the number of categories affected.
pub fn set_country_enabled(
    conn: &Connection,
    provider_id: i64,
    country_code: Option<&str>,
    enabled: bool,
) -> Result<usize> {
    let n = match country_code {
        Some(code) => conn.execute(
            "UPDATE categories SET enabled = ?3 WHERE provider_id = ?1 AND country_code = ?2",
            params![provider_id, code, enabled as i64],
        )?,
        None => conn.execute(
            "UPDATE categories SET enabled = ?2 WHERE provider_id = ?1 AND country_code IS NULL",
            params![provider_id, enabled as i64],
        )?,
    };
    Ok(n)
}

// --- catalog apply -----------------------------------------------------------

/// Apply a freshly fetched catalog for one provider inside a single transaction.
///
/// Upserts categories (inferring country from the name) and channels, mapping
/// each channel to its local category id. Existing categories keep their
/// `enabled` flag; brand-new ones default to enabled. Channels no longer present
/// upstream are removed. This is the synchronous half of a sync — the network
/// fetch happens in [`crate::client`].
pub fn apply_catalog(
    conn: &mut Connection,
    provider_id: i64,
    categories: &[XtreamCategory],
    streams: &[XtreamStream],
) -> Result<CatalogCounts> {
    let tx = conn.transaction()?;

    // Upsert categories; keep a map from the provider's category id -> local id.
    let mut cat_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for cat in categories {
        let country = crate::country::infer_country(&cat.category_name).map(|c| c.code.to_string());
        tx.execute(
            "INSERT INTO categories (provider_id, xtream_category_id, name, country_code, enabled)
             VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT(provider_id, xtream_category_id)
             DO UPDATE SET name = excluded.name, country_code = excluded.country_code",
            params![provider_id, cat.category_id, cat.category_name, country],
        )?;
        let local_id: i64 = tx.query_row(
            "SELECT id FROM categories WHERE provider_id = ?1 AND xtream_category_id = ?2",
            params![provider_id, cat.category_id],
            |r| r.get(0),
        )?;
        cat_map.insert(cat.category_id.clone(), local_id);
    }

    // Remove categories that vanished upstream (cascades channel category to NULL).
    {
        let keep: Vec<String> = categories.iter().map(|c| c.category_id.clone()).collect();
        let mut stmt =
            tx.prepare("SELECT xtream_category_id FROM categories WHERE provider_id = ?1")?;
        let existing: Vec<String> = stmt
            .query_map(params![provider_id], |r| r.get::<_, String>(0))?
            .collect::<Result<_>>()?;
        drop(stmt);
        for xid in existing {
            if !keep.contains(&xid) {
                tx.execute(
                    "DELETE FROM categories WHERE provider_id = ?1 AND xtream_category_id = ?2",
                    params![provider_id, xid],
                )?;
            }
        }
    }

    // Upsert channels.
    let mut channel_count = 0usize;
    for s in streams {
        let local_cat = s
            .category_id
            .as_ref()
            .and_then(|cid| cat_map.get(cid).copied());
        tx.execute(
            "INSERT INTO channels (provider_id, stream_id, name, category_id, epg_channel_id, logo, num)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(provider_id, stream_id)
             DO UPDATE SET name = excluded.name, category_id = excluded.category_id,
                           epg_channel_id = excluded.epg_channel_id, logo = excluded.logo,
                           num = excluded.num",
            params![
                provider_id,
                s.stream_id,
                s.name,
                local_cat,
                s.epg_channel_id,
                s.icon,
                s.num
            ],
        )?;
        channel_count += 1;
    }

    // Remove channels that vanished upstream.
    {
        let keep: Vec<i64> = streams.iter().map(|s| s.stream_id).collect();
        let mut stmt = tx.prepare("SELECT stream_id FROM channels WHERE provider_id = ?1")?;
        let existing: Vec<i64> = stmt
            .query_map(params![provider_id], |r| r.get::<_, i64>(0))?
            .collect::<Result<_>>()?;
        drop(stmt);
        for sid in existing {
            if !keep.contains(&sid) {
                tx.execute(
                    "DELETE FROM channels WHERE provider_id = ?1 AND stream_id = ?2",
                    params![provider_id, sid],
                )?;
            }
        }
    }

    tx.commit()?;
    Ok(CatalogCounts {
        categories: categories.len(),
        channels: channel_count,
    })
}

// --- settings ----------------------------------------------------------------

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()
}

// --- channels: browsing, favorites, recent -----------------------------------

/// A channel row enriched for browsing (provider + category names, favorite).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChannelRow {
    pub id: i64,
    pub provider_id: i64,
    pub provider_name: String,
    pub stream_id: i64,
    pub name: String,
    pub category_name: Option<String>,
    pub country_code: Option<String>,
    pub epg_channel_id: Option<String>,
    pub logo: Option<String>,
    pub num: Option<i64>,
    pub favorite: bool,
}

/// Filters for [`list_channels`]. Only "active" channels are returned — those on
/// an enabled provider whose category is enabled (or absent).
#[derive(Debug, Clone, Default)]
pub struct ChannelQuery {
    pub provider_id: Option<i64>,
    pub category_id: Option<i64>,
    pub search: Option<String>,
    pub favorites_only: bool,
    pub limit: i64,
    pub offset: i64,
}

fn map_channel_row(row: &rusqlite::Row) -> Result<ChannelRow> {
    Ok(ChannelRow {
        id: row.get("id")?,
        provider_id: row.get("provider_id")?,
        provider_name: row.get("provider_name")?,
        stream_id: row.get("stream_id")?,
        name: row.get("name")?,
        category_name: row.get("category_name")?,
        country_code: row.get("country_code")?,
        epg_channel_id: row.get("epg_channel_id")?,
        logo: row.get("logo")?,
        num: row.get("num")?,
        favorite: row.get::<_, i64>("favorite")? != 0,
    })
}

const CHANNEL_SELECT: &str = "
    SELECT ch.id, ch.provider_id, p.name AS provider_name, ch.stream_id, ch.name,
           c.name AS category_name, c.country_code, ch.epg_channel_id, ch.logo, ch.num,
           EXISTS(SELECT 1 FROM favorites f
                  WHERE f.provider_id = ch.provider_id AND f.stream_id = ch.stream_id) AS favorite
    FROM channels ch
    JOIN providers p ON p.id = ch.provider_id AND p.enabled = 1
    LEFT JOIN categories c ON c.id = ch.category_id
    WHERE (ch.category_id IS NULL OR c.enabled = 1)";

/// List active channels matching the query, ordered by provider then channel
/// number/name. `limit` <= 0 means no limit.
pub fn list_channels(conn: &Connection, query: &ChannelQuery) -> Result<Vec<ChannelRow>> {
    let mut sql = String::from(CHANNEL_SELECT);
    let mut args: Vec<Value> = Vec::new();

    if let Some(pid) = query.provider_id {
        sql.push_str(" AND ch.provider_id = ?");
        args.push(Value::Integer(pid));
    }
    if let Some(cid) = query.category_id {
        sql.push_str(" AND ch.category_id = ?");
        args.push(Value::Integer(cid));
    }
    if let Some(s) = query.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" AND ch.name LIKE ? ESCAPE '\\'");
        args.push(Value::Text(format!("%{}%", escape_like(s))));
    }
    if query.favorites_only {
        sql.push_str(
            " AND EXISTS(SELECT 1 FROM favorites f2
                         WHERE f2.provider_id = ch.provider_id AND f2.stream_id = ch.stream_id)",
        );
    }
    sql.push_str(" ORDER BY p.name COLLATE NOCASE, ch.num IS NULL, ch.num, ch.name COLLATE NOCASE");
    if query.limit > 0 {
        sql.push_str(" LIMIT ? OFFSET ?");
        args.push(Value::Integer(query.limit));
        args.push(Value::Integer(query.offset.max(0)));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter()), map_channel_row)?;
    rows.collect()
}

/// Escape `%`, `_`, and `\` for a LIKE pattern (paired with `ESCAPE '\'`).
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

pub fn set_favorite(
    conn: &Connection,
    provider_id: i64,
    stream_id: i64,
    favorite: bool,
    now: i64,
) -> Result<()> {
    if favorite {
        conn.execute(
            "INSERT OR IGNORE INTO favorites (provider_id, stream_id, created_at)
             VALUES (?1, ?2, ?3)",
            params![provider_id, stream_id, now],
        )?;
    } else {
        conn.execute(
            "DELETE FROM favorites WHERE provider_id = ?1 AND stream_id = ?2",
            params![provider_id, stream_id],
        )?;
    }
    Ok(())
}

/// Record a channel as recently watched (upserting the timestamp).
pub fn record_recent(
    conn: &Connection,
    provider_id: i64,
    stream_id: i64,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO recent (provider_id, stream_id, last_watched_utc) VALUES (?1, ?2, ?3)
         ON CONFLICT(provider_id, stream_id) DO UPDATE SET last_watched_utc = excluded.last_watched_utc",
        params![provider_id, stream_id, now],
    )?;
    Ok(())
}

/// Most-recently-watched active channels, newest first.
pub fn list_recent(conn: &Connection, limit: i64) -> Result<Vec<ChannelRow>> {
    let sql = format!(
        "{CHANNEL_SELECT}
         AND EXISTS(SELECT 1 FROM recent r WHERE r.provider_id = ch.provider_id AND r.stream_id = ch.stream_id)
         ORDER BY (SELECT r.last_watched_utc FROM recent r
                   WHERE r.provider_id = ch.provider_id AND r.stream_id = ch.stream_id) DESC
         LIMIT ?"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit.max(0)], map_channel_row)?;
    rows.collect()
}

// --- active categories (for the Live TV group filter) ------------------------

/// An enabled category on an enabled provider, with its channel count.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ActiveCategoryRow {
    pub id: i64,
    pub name: String,
    pub country_code: Option<String>,
    pub provider_name: String,
    pub channel_count: i64,
}

/// List enabled categories across enabled providers, for the Live TV group
/// filter (ordered by provider then name).
pub fn list_active_categories(conn: &Connection) -> Result<Vec<ActiveCategoryRow>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.country_code, p.name AS provider_name,
                (SELECT COUNT(*) FROM channels ch WHERE ch.category_id = c.id) AS channel_count
         FROM categories c
         JOIN providers p ON p.id = c.provider_id AND p.enabled = 1
         WHERE c.enabled = 1
         ORDER BY p.name COLLATE NOCASE, c.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ActiveCategoryRow {
            id: row.get("id")?,
            name: row.get("name")?,
            country_code: row.get("country_code")?,
            provider_name: row.get("provider_name")?,
            channel_count: row.get("channel_count")?,
        })
    })?;
    rows.collect()
}

/// Enable/disable a set of categories in one statement (batch group toggle).
pub fn set_categories_enabled(conn: &Connection, ids: &[i64], enabled: bool) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("UPDATE categories SET enabled = ? WHERE id IN ({placeholders})");
    let mut args: Vec<Value> = Vec::with_capacity(ids.len() + 1);
    args.push(Value::Integer(enabled as i64));
    for id in ids {
        args.push(Value::Integer(*id));
    }
    let n = conn.execute(&sql, params_from_iter(args.iter()))?;
    Ok(n)
}

// --- EPG ---------------------------------------------------------------------

/// A cached EPG programme for the grid (times are UTC unix seconds).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EpgProgramRow {
    pub stream_id: i64,
    pub start_utc: i64,
    pub stop_utc: i64,
    pub title: String,
    pub description: String,
}

/// Replace cached EPG for the given streams in one transaction. Each tuple is a
/// stream id and its fresh programme list; the stream's old rows are cleared
/// first. Returns the total number of programmes written.
pub fn replace_epg_for_streams(
    conn: &mut Connection,
    provider_id: i64,
    data: &[(i64, Vec<XtreamEpgEntry>)],
) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut total = 0usize;
    for (stream_id, entries) in data {
        tx.execute(
            "DELETE FROM epg_programs WHERE provider_id = ?1 AND stream_id = ?2",
            params![provider_id, stream_id],
        )?;
        for e in entries {
            tx.execute(
                "INSERT OR REPLACE INTO epg_programs
                 (provider_id, stream_id, start_utc, stop_utc, title, description)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![provider_id, stream_id, e.start_utc, e.stop_utc, e.title, e.description],
            )?;
            total += 1;
        }
    }
    tx.commit()?;
    Ok(total)
}

/// Programmes overlapping the window `[from, to)` for a provider's active
/// channels (enabled provider + enabled/absent category), ordered for the grid.
pub fn get_epg_window(
    conn: &Connection,
    provider_id: i64,
    from: i64,
    to: i64,
) -> Result<Vec<EpgProgramRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.stream_id, e.start_utc, e.stop_utc, e.title, e.description
         FROM epg_programs e
         JOIN channels ch ON ch.provider_id = e.provider_id AND ch.stream_id = e.stream_id
         JOIN providers p ON p.id = ch.provider_id AND p.enabled = 1
         LEFT JOIN categories c ON c.id = ch.category_id
         WHERE e.provider_id = ?1
           AND (ch.category_id IS NULL OR c.enabled = 1)
           AND e.stop_utc > ?2 AND e.start_utc < ?3
         ORDER BY e.stream_id, e.start_utc",
    )?;
    let rows = stmt.query_map(params![provider_id, from, to], |row| {
        Ok(EpgProgramRow {
            stream_id: row.get("stream_id")?,
            start_utc: row.get("start_utc")?,
            stop_utc: row.get("stop_utc")?,
            title: row.get("title")?,
            description: row.get("description")?,
        })
    })?;
    rows.collect()
}

/// Count cached EPG programmes for a provider (diagnostics / UI).
pub fn epg_program_count(conn: &Connection, provider_id: i64) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM epg_programs WHERE provider_id = ?1",
        params![provider_id],
        |r| r.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(id: &str, name: &str) -> XtreamCategory {
        XtreamCategory {
            category_id: id.to_string(),
            category_name: name.to_string(),
        }
    }

    fn stream(id: i64, name: &str, cat: Option<&str>, epg: Option<&str>) -> XtreamStream {
        XtreamStream {
            stream_id: id,
            name: name.to_string(),
            category_id: cat.map(|s| s.to_string()),
            epg_channel_id: epg.map(|s| s.to_string()),
            icon: None,
            num: None,
        }
    }

    fn new_provider() -> NewProvider {
        NewProvider {
            name: "Test".into(),
            host: "http://h".into(),
            port: 8080,
            username: "u".into(),
            password_enc: vec![1, 2, 3],
            created_at: 1000,
        }
    }

    #[test]
    fn provider_crud_and_toggle() {
        let conn = open_in_memory().unwrap();
        let id = insert_provider(&conn, &new_provider()).unwrap();
        let all = list_providers(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].enabled);
        assert_eq!(all[0].password_enc, vec![1, 2, 3]);

        set_provider_enabled(&conn, id, false).unwrap();
        assert!(!get_provider(&conn, id).unwrap().unwrap().enabled);

        delete_provider(&conn, id).unwrap();
        assert!(list_providers(&conn).unwrap().is_empty());
    }

    #[test]
    fn apply_catalog_maps_and_counts() {
        let mut conn = open_in_memory().unwrap();
        let pid = insert_provider(&conn, &new_provider()).unwrap();

        let cats = vec![cat("1", "UK | Sports"), cat("2", "US | News")];
        let streams = vec![
            stream(100, "Sky Sports", Some("1"), Some("sky.uk")),
            stream(101, "CNN", Some("2"), Some("cnn.us")),
            stream(102, "Orphan", Some("999"), None), // unknown category -> NULL
        ];
        let counts = apply_catalog(&mut conn, pid, &cats, &streams).unwrap();
        assert_eq!(counts.categories, 2);
        assert_eq!(counts.channels, 3);

        let listed = list_categories(&conn, pid).unwrap();
        assert_eq!(listed.len(), 2);
        let uk = listed.iter().find(|c| c.name == "UK | Sports").unwrap();
        assert_eq!(uk.country_code.as_deref(), Some("GB"));
        assert_eq!(uk.channel_count, 1);
    }

    #[test]
    fn resync_preserves_enabled_and_prunes() {
        let mut conn = open_in_memory().unwrap();
        let pid = insert_provider(&conn, &new_provider()).unwrap();

        apply_catalog(
            &mut conn,
            pid,
            &[cat("1", "UK | Sports"), cat("2", "US | News")],
            &[stream(100, "A", Some("1"), None), stream(101, "B", Some("2"), None)],
        )
        .unwrap();

        // User disables the UK category.
        let uk_id = list_categories(&conn, pid)
            .unwrap()
            .into_iter()
            .find(|c| c.country_code.as_deref() == Some("GB"))
            .unwrap()
            .id;
        set_category_enabled(&conn, uk_id, false).unwrap();

        // Re-sync: US category dropped upstream, UK renamed.
        apply_catalog(
            &mut conn,
            pid,
            &[cat("1", "UK | Sports HD")],
            &[stream(100, "A2", Some("1"), None)],
        )
        .unwrap();

        let listed = list_categories(&conn, pid).unwrap();
        assert_eq!(listed.len(), 1, "US category should be pruned");
        assert_eq!(listed[0].name, "UK | Sports HD", "rename applied");
        assert!(!listed[0].enabled, "user's disable must survive re-sync");

        // Channel 101 (US) pruned; 100 renamed.
        let chan_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM channels WHERE provider_id=?1", [pid], |r| r.get(0))
            .unwrap();
        assert_eq!(chan_count, 1);
    }

    #[test]
    fn country_toggle_affects_group() {
        let mut conn = open_in_memory().unwrap();
        let pid = insert_provider(&conn, &new_provider()).unwrap();
        apply_catalog(
            &mut conn,
            pid,
            &[cat("1", "UK | Sports"), cat("2", "UK | Movies"), cat("3", "US | News")],
            &[],
        )
        .unwrap();

        let n = set_country_enabled(&conn, pid, Some("GB"), false).unwrap();
        assert_eq!(n, 2);
        let listed = list_categories(&conn, pid).unwrap();
        assert_eq!(listed.iter().filter(|c| !c.enabled).count(), 2);
    }

    #[test]
    fn stats_and_bulk_toggle() {
        let mut conn = open_in_memory().unwrap();
        let pid = insert_provider(&conn, &new_provider()).unwrap();
        apply_catalog(
            &mut conn,
            pid,
            &[cat("1", "UK | Sports"), cat("2", "US | News")],
            &[
                stream(100, "A", Some("1"), None),
                stream(101, "B", Some("2"), None),
                stream(102, "Uncat", None, None), // no category -> always enabled
            ],
        )
        .unwrap();

        let s = curation_stats(&conn, pid).unwrap();
        assert_eq!(s.total_categories, 2);
        assert_eq!(s.enabled_categories, 2);
        assert_eq!(s.total_channels, 3);
        assert_eq!(s.enabled_channels, 3);

        // Disable everything: the uncategorized channel stays enabled.
        let n = set_all_categories_enabled(&conn, pid, false).unwrap();
        assert_eq!(n, 2);
        let s = curation_stats(&conn, pid).unwrap();
        assert_eq!(s.enabled_categories, 0);
        assert_eq!(s.enabled_channels, 1); // only the uncategorized one
    }

    #[test]
    fn list_channels_respects_curation_and_favorites() {
        let mut conn = open_in_memory().unwrap();
        let pid = insert_provider(&conn, &new_provider()).unwrap();
        apply_catalog(
            &mut conn,
            pid,
            &[cat("1", "UK | Sports"), cat("2", "US | News")],
            &[
                stream(100, "Sky Sports", Some("1"), Some("sky.uk")),
                stream(101, "CNN", Some("2"), Some("cnn.us")),
                stream(102, "Freebie", None, None),
            ],
        )
        .unwrap();

        // All three active initially.
        let all = list_channels(&conn, &ChannelQuery::default()).unwrap();
        assert_eq!(all.len(), 3);

        // Disable the US category -> CNN drops out.
        let us_id = list_categories(&conn, pid)
            .unwrap()
            .into_iter()
            .find(|c| c.country_code.as_deref() == Some("US"))
            .unwrap()
            .id;
        set_category_enabled(&conn, us_id, false).unwrap();
        let active = list_channels(&conn, &ChannelQuery::default()).unwrap();
        assert_eq!(active.len(), 2);
        assert!(active.iter().all(|c| c.name != "CNN"));

        // Search.
        let found = list_channels(
            &conn,
            &ChannelQuery {
                search: Some("sky".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Sky Sports");
        assert!(!found[0].favorite);

        // Favorite Sky, then favorites-only.
        set_favorite(&conn, pid, 100, true, 10).unwrap();
        let favs = list_channels(
            &conn,
            &ChannelQuery {
                favorites_only: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(favs.len(), 1);
        assert!(favs[0].favorite);

        // Provider disabled -> nothing active.
        set_provider_enabled(&conn, pid, false).unwrap();
        assert!(list_channels(&conn, &ChannelQuery::default()).unwrap().is_empty());
    }

    #[test]
    fn category_filter_and_batch_toggle() {
        let mut conn = open_in_memory().unwrap();
        let pid = insert_provider(&conn, &new_provider()).unwrap();
        apply_catalog(
            &mut conn,
            pid,
            &[cat("1", "UK | Sports"), cat("2", "4K UHD"), cat("3", "MUSIC")],
            &[
                stream(100, "Sky Sports", Some("1"), None),
                stream(101, "4K One", Some("2"), None),
                stream(102, "MTV", Some("3"), None),
            ],
        )
        .unwrap();

        // Active categories: all three enabled, across the one provider.
        let active = list_active_categories(&conn).unwrap();
        assert_eq!(active.len(), 3);
        assert!(active.iter().any(|c| c.name == "4K UHD" && c.channel_count == 1));

        // Filter channels by the 4K category.
        let uhd_id = active.iter().find(|c| c.name == "4K UHD").unwrap().id;
        let filtered = list_channels(
            &conn,
            &ChannelQuery {
                category_id: Some(uhd_id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "4K One");

        // Batch-disable 4K + Music.
        let music_id = active.iter().find(|c| c.name == "MUSIC").unwrap().id;
        let n = set_categories_enabled(&conn, &[uhd_id, music_id], false).unwrap();
        assert_eq!(n, 2);
        assert_eq!(list_active_categories(&conn).unwrap().len(), 1); // only UK Sports left
        // Their channels drop out of the active list.
        assert_eq!(list_channels(&conn, &ChannelQuery::default()).unwrap().len(), 1);
    }

    #[test]
    fn recent_orders_newest_first() {
        let mut conn = open_in_memory().unwrap();
        let pid = insert_provider(&conn, &new_provider()).unwrap();
        apply_catalog(
            &mut conn,
            pid,
            &[cat("1", "UK | Sports")],
            &[stream(100, "A", Some("1"), None), stream(101, "B", Some("1"), None)],
        )
        .unwrap();

        record_recent(&conn, pid, 100, 500).unwrap();
        record_recent(&conn, pid, 101, 900).unwrap();
        record_recent(&conn, pid, 100, 1000).unwrap(); // A watched again, newest

        let recent = list_recent(&conn, 10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].stream_id, 100); // most recent
        assert_eq!(recent[1].stream_id, 101);
    }

    #[test]
    fn epg_replace_and_window() {
        let mut conn = open_in_memory().unwrap();
        let pid = insert_provider(&conn, &new_provider()).unwrap();
        apply_catalog(
            &mut conn,
            pid,
            &[cat("1", "UK | Sports"), cat("2", "US | News")],
            &[
                stream(100, "A", Some("1"), Some("a.uk")),
                stream(101, "B", Some("2"), Some("b.us")),
            ],
        )
        .unwrap();

        let epg = |start: i64, stop: i64, t: &str| XtreamEpgEntry {
            start_utc: start,
            stop_utc: stop,
            title: t.into(),
            description: String::new(),
            channel_id: None,
        };
        let written = replace_epg_for_streams(
            &mut conn,
            pid,
            &[
                (100, vec![epg(1000, 2000, "A1"), epg(2000, 3000, "A2")]),
                (101, vec![epg(1500, 2500, "B1")]),
            ],
        )
        .unwrap();
        assert_eq!(written, 3);
        assert_eq!(epg_program_count(&conn, pid).unwrap(), 3);

        // Window [1800, 2200) overlaps A1, A2, B1.
        let w = get_epg_window(&conn, pid, 1800, 2200).unwrap();
        assert_eq!(w.len(), 3);

        // Window [2600, 3000) overlaps only A2.
        let w = get_epg_window(&conn, pid, 2600, 3000).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].title, "A2");

        // Disable US category -> B's EPG excluded from the window.
        let us = list_categories(&conn, pid)
            .unwrap()
            .into_iter()
            .find(|c| c.country_code.as_deref() == Some("US"))
            .unwrap()
            .id;
        set_category_enabled(&conn, us, false).unwrap();
        let w = get_epg_window(&conn, pid, 1800, 2200).unwrap();
        assert!(w.iter().all(|p| p.stream_id != 101));

        // Re-replacing a stream's EPG clears the old rows.
        replace_epg_for_streams(&mut conn, pid, &[(100, vec![epg(5000, 6000, "A-new")])]).unwrap();
        let all = get_epg_window(&conn, pid, 0, 100000).unwrap();
        let a_titles: Vec<_> = all.iter().filter(|p| p.stream_id == 100).map(|p| &p.title).collect();
        assert_eq!(a_titles, vec!["A-new"]);
    }

    #[test]
    fn settings_roundtrip() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap(), None);
        set_setting(&conn, "k", "v1").unwrap();
        set_setting(&conn, "k", "v2").unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap().as_deref(), Some("v2"));
    }
}
