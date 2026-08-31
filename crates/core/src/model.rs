//! Normalized domain model — what fluxxx stores and shows, independent of the
//! raw Xtream API shapes (which live in [`crate::xtream`]).

use serde::{Deserialize, Serialize};

/// A configured Xtream Codes provider. Credentials are held in plaintext only
/// in memory / at the API boundary; at rest they are encrypted by the app layer
/// (Windows DPAPI) — this struct never carries the encrypted blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provider {
    pub id: i64,
    pub name: String,
    /// Scheme + host, no trailing slash, e.g. `http://example.com`.
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Present only when the caller explicitly needs it (login, stream URL).
    /// Serialized out to the UI as `None`.
    #[serde(skip_serializing)]
    pub password: Option<String>,
    /// When false, the provider is fully dormant: no catalog refresh, no EPG.
    pub enabled: bool,
}

impl Provider {
    /// Base URL with no trailing slash, e.g. `http://example.com:8080`.
    pub fn base_url(&self) -> String {
        format!("{}:{}", self.host.trim_end_matches('/'), self.port)
    }
}

/// A live-stream category as exposed by the provider, annotated with an inferred
/// country and an enabled flag driven by curation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub provider_id: i64,
    /// The provider's own category id (string in the Xtream API).
    pub xtream_category_id: String,
    pub name: String,
    /// ISO 3166-1 alpha-2, inferred from the name; `None` when unknown ("Other").
    pub country_code: Option<String>,
    pub enabled: bool,
}

/// A single live channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub id: i64,
    pub provider_id: i64,
    pub stream_id: i64,
    pub name: String,
    pub category_id: Option<i64>,
    /// XMLTV channel id used to join against EPG rows; may be empty.
    pub epg_channel_id: Option<String>,
    pub logo: Option<String>,
    /// The provider's display order number.
    pub num: Option<i64>,
}

/// A cached EPG programme entry (times normalized to UTC unix seconds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpgProgram {
    pub provider_id: i64,
    pub epg_channel_id: String,
    pub start_utc: i64,
    pub stop_utc: i64,
    pub title: String,
    pub description: String,
}
