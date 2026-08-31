//! HTTP abstraction. The [`Fetcher`] trait lets the Xtream client be tested with
//! canned responses; the real reqwest implementation is compiled only under the
//! `net` feature (enabled by the app crate).

use async_trait::async_trait;

#[derive(Debug)]
pub enum HttpError {
    /// Transport/connection failure.
    Transport(String),
    /// Non-success HTTP status.
    Status(u16),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::Transport(m) => write!(f, "transport error: {m}"),
            HttpError::Status(c) => write!(f, "http status {c}"),
        }
    }
}

impl std::error::Error for HttpError {}

/// Fetch text from a URL. Implementors must be `Send + Sync` so the client can be
/// used from Tauri's async runtime.
#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn get_text(&self, url: &str) -> Result<String, HttpError>;
}

#[cfg(feature = "net")]
pub use reqwest_impl::ReqwestFetcher;

#[cfg(feature = "net")]
mod reqwest_impl {
    use super::*;
    use std::time::Duration;

    /// Real HTTP fetcher backed by a shared reqwest client.
    pub struct ReqwestFetcher {
        client: reqwest::Client,
    }

    impl ReqwestFetcher {
        pub fn new() -> Self {
            let client = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(45))
                .user_agent("fluxxx/0.1")
                .build()
                .expect("failed to build reqwest client");
            Self { client }
        }
    }

    impl Default for ReqwestFetcher {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl Fetcher for ReqwestFetcher {
        async fn get_text(&self, url: &str) -> Result<String, HttpError> {
            let resp = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| HttpError::Transport(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(HttpError::Status(resp.status().as_u16()));
            }
            resp.text()
                .await
                .map_err(|e| HttpError::Transport(e.to_string()))
        }
    }
}
