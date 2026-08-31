//! Async Xtream Codes client: builds `player_api.php` URLs, fetches via a
//! [`Fetcher`](crate::http::Fetcher), and parses with [`crate::xtream`].
//!
//! Network + parsing only — nothing here touches the database. The catalog is
//! persisted separately via [`crate::db::apply_catalog`], which keeps this layer
//! testable with canned responses and no SQLite.

use crate::http::{Fetcher, HttpError};
use crate::xtream::{
    self, AuthResult, ParseError, XtreamCategory, XtreamEpgEntry, XtreamStream,
};

/// Connection details for one provider (plaintext password, used only in memory).
#[derive(Debug, Clone)]
pub struct Creds {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug)]
pub enum ClientError {
    Http(HttpError),
    Parse(ParseError),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Http(e) => write!(f, "{e}"),
            ClientError::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<HttpError> for ClientError {
    fn from(e: HttpError) -> Self {
        ClientError::Http(e)
    }
}
impl From<ParseError> for ClientError {
    fn from(e: ParseError) -> Self {
        ClientError::Parse(e)
    }
}

/// Authenticate (bare `player_api.php` with no action).
pub async fn authenticate<F: Fetcher>(f: &F, c: &Creds) -> Result<AuthResult, ClientError> {
    let url = xtream::player_api_url(&c.base_url, &c.username, &c.password, "", &[]);
    let body = f.get_text(&url).await?;
    Ok(xtream::parse_auth(&body)?)
}

/// Fetch live categories.
pub async fn fetch_categories<F: Fetcher>(
    f: &F,
    c: &Creds,
) -> Result<Vec<XtreamCategory>, ClientError> {
    let url = xtream::player_api_url(&c.base_url, &c.username, &c.password, "get_live_categories", &[]);
    let body = f.get_text(&url).await?;
    Ok(xtream::parse_categories(&body)?)
}

/// Fetch live streams.
pub async fn fetch_streams<F: Fetcher>(f: &F, c: &Creds) -> Result<Vec<XtreamStream>, ClientError> {
    let url = xtream::player_api_url(&c.base_url, &c.username, &c.password, "get_live_streams", &[]);
    let body = f.get_text(&url).await?;
    Ok(xtream::parse_streams(&body)?)
}

/// Fetch the EPG table for a single stream.
pub async fn fetch_epg<F: Fetcher>(
    f: &F,
    c: &Creds,
    stream_id: i64,
) -> Result<Vec<XtreamEpgEntry>, ClientError> {
    let sid = stream_id.to_string();
    let url = xtream::player_api_url(
        &c.base_url,
        &c.username,
        &c.password,
        "get_simple_data_table",
        &[("stream_id", &sid)],
    );
    let body = f.get_text(&url).await?;
    Ok(xtream::parse_epg_table(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::Fetcher;
    use async_trait::async_trait;
    use std::collections::HashMap;

    /// Fetcher that returns canned bodies keyed by a substring of the URL.
    struct FakeFetcher {
        routes: HashMap<&'static str, String>,
    }

    #[async_trait]
    impl Fetcher for FakeFetcher {
        async fn get_text(&self, url: &str) -> Result<String, HttpError> {
            for (needle, body) in &self.routes {
                if url.contains(*needle) {
                    return Ok(body.clone());
                }
            }
            Err(HttpError::Status(404))
        }
    }

    fn creds() -> Creds {
        Creds {
            base_url: "http://h:8080".into(),
            username: "u".into(),
            password: "p".into(),
        }
    }

    #[tokio::test]
    async fn authenticates() {
        let mut routes = HashMap::new();
        routes.insert(
            "player_api.php?username=u&password=p",
            r#"{"user_info":{"auth":1,"status":"Active"}}"#.to_string(),
        );
        let f = FakeFetcher { routes };
        let auth = authenticate(&f, &creds()).await.unwrap();
        assert!(auth.authenticated);
    }

    #[tokio::test]
    async fn fetches_and_parses_catalog() {
        let mut routes = HashMap::new();
        routes.insert(
            "get_live_categories",
            r#"[{"category_id":"1","category_name":"UK | Sports"}]"#.to_string(),
        );
        routes.insert(
            "get_live_streams",
            r#"[{"stream_id":100,"name":"Sky","category_id":"1","epg_channel_id":"sky.uk"}]"#
                .to_string(),
        );
        let f = FakeFetcher { routes };
        let cats = fetch_categories(&f, &creds()).await.unwrap();
        assert_eq!(cats.len(), 1);
        let streams = fetch_streams(&f, &creds()).await.unwrap();
        assert_eq!(streams[0].stream_id, 100);
    }

    #[tokio::test]
    async fn surfaces_http_errors() {
        let f = FakeFetcher {
            routes: HashMap::new(),
        };
        let err = fetch_categories(&f, &creds()).await.unwrap_err();
        assert!(matches!(err, ClientError::Http(HttpError::Status(404))));
    }
}
