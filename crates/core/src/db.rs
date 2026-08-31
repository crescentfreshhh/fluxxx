//! SQLite persistence for the catalog (providers, categories, channels) and
//! settings. Pure enough to run against an in-memory database in unit tests on
//! any platform.
//!
//! Credentials are stored as an opaque encrypted blob (`password_enc`) — this
//! layer never encrypts or decrypts; that is the app layer's job (Windows DPAPI).
//! Re-syncing a provider preserves user curation: existing categories keep their
//! `enabled` flag; only new categories default to enabled.

use rusqlite::{params, Connection, OptionalExtension};

use crate::xtream::{XtreamCategory, XtreamStream};

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
    fn settings_roundtrip() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap(), None);
        set_setting(&conn, "k", "v1").unwrap();
        set_setting(&conn, "k", "v2").unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap().as_deref(), Some("v2"));
    }
}
