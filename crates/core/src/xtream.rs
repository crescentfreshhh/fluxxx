//! Xtream Codes `player_api.php` URL construction and response parsing.
//!
//! No network here — callers fetch the bytes (app layer) and hand the text to
//! the `parse_*` functions. This keeps every wire-format quirk unit-testable.
//!
//! Field-shape notes learned from real panels:
//!   * `category_id` is a *string*; on streams it can be null/empty.
//!   * `stream_id` is an integer.
//!   * `epg_channel_id` can be null or empty.
//!   * numeric-ish fields (`num`, timestamps) may arrive as strings or numbers.
//!   * EPG `title`/`description` are base64-encoded; times come as unix-second
//!     strings in `start_timestamp`/`stop_timestamp`.

use base64::Engine;
use serde::Deserialize;

/// Result of authenticating against a panel.
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub authenticated: bool,
    pub status: Option<String>,
    pub expires_at: Option<i64>,
}

/// A category row from `action=get_live_categories`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XtreamCategory {
    pub category_id: String,
    pub category_name: String,
}

/// A live stream row from `action=get_live_streams`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XtreamStream {
    pub stream_id: i64,
    pub name: String,
    pub category_id: Option<String>,
    pub epg_channel_id: Option<String>,
    pub icon: Option<String>,
    pub num: Option<i64>,
}

/// A decoded EPG entry from `action=get_simple_data_table`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XtreamEpgEntry {
    pub start_utc: i64,
    pub stop_utc: i64,
    pub title: String,
    pub description: String,
    pub channel_id: Option<String>,
}

/// Build a `player_api.php` URL for the given action.
///
/// `base` is scheme+host+port with no trailing slash, e.g. `http://host:8080`.
/// `extra` are additional `(key, value)` query params (already unencoded); the
/// values are minimally percent-encoded for the characters that matter here.
pub fn player_api_url(
    base: &str,
    username: &str,
    password: &str,
    action: &str,
    extra: &[(&str, &str)],
) -> String {
    let mut url = format!(
        "{}/player_api.php?username={}&password={}",
        base.trim_end_matches('/'),
        encode(username),
        encode(password),
    );
    if !action.is_empty() {
        url.push_str("&action=");
        url.push_str(action);
    }
    for (k, v) in extra {
        url.push('&');
        url.push_str(k);
        url.push('=');
        url.push_str(&encode(v));
    }
    url
}

/// Normalize a user-entered host into scheme + host (no trailing slash, no port).
///
/// If the host already carries an `http(s)://` scheme it is kept as-is; otherwise
/// the scheme is inferred from the port — `https` for 443/8443, `http` otherwise.
/// Callers append `:{port}` themselves to form the base URL.
pub fn normalize_base(host: &str, port: u16) -> String {
    let trimmed = host.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        let scheme = if port == 443 || port == 8443 { "https" } else { "http" };
        format!("{scheme}://{trimmed}")
    }
}

/// Build the live stream playback URL.
///
/// `container` is the extension without a dot, typically `ts` or `m3u8`.
pub fn stream_url(
    base: &str,
    username: &str,
    password: &str,
    stream_id: i64,
    container: &str,
) -> String {
    format!(
        "{}/live/{}/{}/{}.{}",
        base.trim_end_matches('/'),
        encode(username),
        encode(password),
        stream_id,
        container,
    )
}

/// Parse the `user_info` block from a bare `player_api.php` (no action) call.
pub fn parse_auth(json: &str) -> Result<AuthResult, ParseError> {
    #[derive(Deserialize)]
    struct Root {
        user_info: Option<UserInfo>,
    }
    #[derive(Deserialize)]
    struct UserInfo {
        #[serde(default)]
        auth: JsonNum,
        status: Option<String>,
        exp_date: Option<StringOrNum>,
    }
    let root: Root = serde_json::from_str(json).map_err(ParseError::from)?;
    let ui = root.user_info.ok_or(ParseError::MissingField("user_info"))?;
    Ok(AuthResult {
        authenticated: ui.auth.0 == 1 || matches!(ui.status.as_deref(), Some("Active")),
        status: ui.status,
        expires_at: ui.exp_date.and_then(|e| e.as_i64()),
    })
}

/// Parse `action=get_live_categories`.
pub fn parse_categories(json: &str) -> Result<Vec<XtreamCategory>, ParseError> {
    #[derive(Deserialize)]
    struct Raw {
        category_id: StringOrNum,
        category_name: Option<String>,
    }
    let raw: Vec<Raw> = serde_json::from_str(json).map_err(ParseError::from)?;
    Ok(raw
        .into_iter()
        .filter_map(|r| {
            let id = r.category_id.into_string();
            if id.is_empty() {
                return None;
            }
            Some(XtreamCategory {
                category_id: id,
                category_name: r.category_name.unwrap_or_default(),
            })
        })
        .collect())
}

/// Parse `action=get_live_streams`.
pub fn parse_streams(json: &str) -> Result<Vec<XtreamStream>, ParseError> {
    #[derive(Deserialize)]
    struct Raw {
        stream_id: StringOrNum,
        name: Option<String>,
        category_id: Option<StringOrNum>,
        epg_channel_id: Option<String>,
        stream_icon: Option<String>,
        num: Option<StringOrNum>,
    }
    let raw: Vec<Raw> = serde_json::from_str(json).map_err(ParseError::from)?;
    Ok(raw
        .into_iter()
        .filter_map(|r| {
            let stream_id = r.stream_id.as_i64()?;
            Some(XtreamStream {
                stream_id,
                name: r.name.unwrap_or_default(),
                category_id: r
                    .category_id
                    .map(|c| c.into_string())
                    .filter(|s| !s.is_empty()),
                epg_channel_id: r.epg_channel_id.filter(|s| !s.is_empty()),
                icon: r.stream_icon.filter(|s| !s.is_empty()),
                num: r.num.and_then(|n| n.as_i64()),
            })
        })
        .collect())
}

/// Parse `action=get_simple_data_table`, decoding base64 titles/descriptions and
/// normalizing times to UTC unix seconds. Entries with unusable times are dropped.
pub fn parse_epg_table(json: &str) -> Result<Vec<XtreamEpgEntry>, ParseError> {
    #[derive(Deserialize)]
    struct Root {
        #[serde(default)]
        epg_listings: Vec<Raw>,
    }
    #[derive(Deserialize)]
    struct Raw {
        title: Option<String>,
        description: Option<String>,
        channel_id: Option<String>,
        start_timestamp: Option<StringOrNum>,
        stop_timestamp: Option<StringOrNum>,
    }
    let root: Root = serde_json::from_str(json).map_err(ParseError::from)?;
    Ok(root
        .epg_listings
        .into_iter()
        .filter_map(|r| {
            let start = r.start_timestamp.and_then(|s| s.as_i64())?;
            let stop = r.stop_timestamp.and_then(|s| s.as_i64())?;
            if stop < start {
                return None;
            }
            Some(XtreamEpgEntry {
                start_utc: start,
                stop_utc: stop,
                title: decode_b64(r.title.as_deref()),
                description: decode_b64(r.description.as_deref()),
                channel_id: r.channel_id.filter(|s| !s.is_empty()),
            })
        })
        .collect())
}

/// Decode a possibly-base64 field. If it isn't valid base64/UTF-8, fall back to
/// the raw string so we never lose a title outright.
fn decode_b64(s: Option<&str>) -> String {
    let Some(s) = s else { return String::new() };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match base64::engine::general_purpose::STANDARD.decode(trimmed) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => trimmed.to_string(),
        },
        Err(_) => trimmed.to_string(),
    }
}

/// Minimal percent-encoding for query-string values (space and the reserved
/// characters that actually break Xtream URLs). Credentials are usually
/// alphanumeric; this covers the occasional symbol.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Errors from parsing Xtream responses.
#[derive(Debug)]
pub enum ParseError {
    Json(serde_json::Error),
    MissingField(&'static str),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Json(e) => write!(f, "json error: {e}"),
            ParseError::MissingField(name) => write!(f, "missing field: {name}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<serde_json::Error> for ParseError {
    fn from(e: serde_json::Error) -> Self {
        ParseError::Json(e)
    }
}

// --- helpers for the loose typing Xtream panels use --------------------------

/// Accepts a JSON number or numeric string; defaults to 0.
#[derive(Default)]
struct JsonNum(i64);

impl<'de> Deserialize<'de> for JsonNum {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(JsonNum(
            StringOrNum::deserialize(d)?.as_i64().unwrap_or(0),
        ))
    }
}

/// Accepts either a JSON string or a JSON number.
#[derive(Debug, Clone)]
enum StringOrNum {
    S(String),
    N(i64),
}

impl StringOrNum {
    fn as_i64(&self) -> Option<i64> {
        match self {
            StringOrNum::N(n) => Some(*n),
            StringOrNum::S(s) => s.trim().parse::<i64>().ok(),
        }
    }
    fn into_string(self) -> String {
        match self {
            StringOrNum::N(n) => n.to_string(),
            StringOrNum::S(s) => s.trim().to_string(),
        }
    }
}

impl<'de> Deserialize<'de> for StringOrNum {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde_json::Value;
        match Value::deserialize(d)? {
            Value::String(s) => Ok(StringOrNum::S(s)),
            Value::Number(n) => Ok(StringOrNum::N(n.as_i64().unwrap_or(0))),
            Value::Null => Ok(StringOrNum::S(String::new())),
            other => Ok(StringOrNum::S(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_player_api_url() {
        let u = player_api_url("http://h:8080/", "user", "pa ss", "get_live_streams", &[]);
        assert_eq!(
            u,
            "http://h:8080/player_api.php?username=user&password=pa%20ss&action=get_live_streams"
        );
    }

    #[test]
    fn builds_player_api_url_with_extra() {
        let u = player_api_url(
            "http://h:8080",
            "u",
            "p",
            "get_simple_data_table",
            &[("stream_id", "42")],
        );
        assert!(u.ends_with("&action=get_simple_data_table&stream_id=42"));
    }

    #[test]
    fn builds_stream_url() {
        let u = stream_url("http://h:8080", "u", "p", 12345, "ts");
        assert_eq!(u, "http://h:8080/live/u/p/12345.ts");
    }

    #[test]
    fn normalize_base_infers_scheme() {
        assert_eq!(normalize_base("example.com", 443), "https://example.com");
        assert_eq!(normalize_base("example.com", 8443), "https://example.com");
        assert_eq!(normalize_base("example.com", 80), "http://example.com");
        assert_eq!(normalize_base("example.com/", 8080), "http://example.com");
        // Explicit scheme is preserved regardless of port.
        assert_eq!(normalize_base("http://example.com", 443), "http://example.com");
        assert_eq!(normalize_base("https://example.com/", 80), "https://example.com");
    }

    #[test]
    fn parses_categories() {
        let json = r#"[
            {"category_id":"1","category_name":"UK | ENTERTAINMENT","parent_id":0},
            {"category_id":"2","category_name":"US | NEWS","parent_id":0},
            {"category_id":"","category_name":"broken"}
        ]"#;
        let cats = parse_categories(json).unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0].category_id, "1");
        assert_eq!(cats[1].category_name, "US | NEWS");
    }

    #[test]
    fn parses_streams_with_mixed_types() {
        // stream_id as number, num as string, missing/empty epg + category.
        let json = r#"[
            {"num":1,"name":"BBC One","stream_id":100,"epg_channel_id":"bbc1.uk","category_id":"1","stream_icon":"http://x/i.png"},
            {"num":"2","name":"No EPG","stream_id":"101","epg_channel_id":"","category_id":null}
        ]"#;
        let s = parse_streams(json).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].stream_id, 100);
        assert_eq!(s[0].epg_channel_id.as_deref(), Some("bbc1.uk"));
        assert_eq!(s[0].category_id.as_deref(), Some("1"));
        assert_eq!(s[1].num, Some(2));
        assert_eq!(s[1].epg_channel_id, None);
        assert_eq!(s[1].category_id, None);
    }

    #[test]
    fn parses_epg_with_base64_and_timestamps() {
        // title "News at Ten" and description "Headlines" base64-encoded.
        let json = r#"{"epg_listings":[
            {"title":"TmV3cyBhdCBUZW4=","description":"SGVhZGxpbmVz","channel_id":"bbc1.uk","start_timestamp":"1700000000","stop_timestamp":"1700001800"},
            {"title":"bad","description":"","start_timestamp":"10","stop_timestamp":"5"}
        ]}"#;
        let epg = parse_epg_table(json).unwrap();
        assert_eq!(epg.len(), 1); // second dropped: stop < start
        assert_eq!(epg[0].title, "News at Ten");
        assert_eq!(epg[0].description, "Headlines");
        assert_eq!(epg[0].start_utc, 1700000000);
        assert_eq!(epg[0].stop_utc, 1700001800);
    }

    #[test]
    fn epg_falls_back_when_not_base64() {
        let json = r#"{"epg_listings":[
            {"title":"Plain Title !!","description":"x","start_timestamp":1,"stop_timestamp":2}
        ]}"#;
        let epg = parse_epg_table(json).unwrap();
        assert_eq!(epg[0].title, "Plain Title !!");
    }

    #[test]
    fn parses_auth() {
        let json = r#"{"user_info":{"auth":1,"status":"Active","exp_date":"1800000000"},"server_info":{}}"#;
        let a = parse_auth(json).unwrap();
        assert!(a.authenticated);
        assert_eq!(a.status.as_deref(), Some("Active"));
        assert_eq!(a.expires_at, Some(1800000000));
    }
}
