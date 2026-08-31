//! Parsing of the external credentials file (`fluxxx-providers.toml`) that can
//! sit next to the executable to pre-load IPTV providers. Pure parsing only —
//! file discovery, DPAPI encryption, and DB insertion live in the app layer.
//!
//! Format (array-of-tables, reads like an INI):
//! ```toml
//! [[provider]]
//! name = "My IPTV"     # optional; defaults to the host
//! host = "example.com" # scheme optional (inferred from port)
//! port = 443
//! username = "user"
//! password = "pass"
//! enabled = true       # optional; defaults to true
//! ```

use serde::Deserialize;

/// One provider entry from the credentials file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSeed {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub enabled: bool,
}

/// Parse the credentials file contents into provider seeds.
pub fn parse_providers_toml(contents: &str) -> Result<Vec<ProviderSeed>, String> {
    #[derive(Deserialize)]
    struct Root {
        #[serde(default)]
        provider: Vec<Raw>,
    }
    #[derive(Deserialize)]
    struct Raw {
        name: Option<String>,
        host: String,
        port: u16,
        username: String,
        password: String,
        #[serde(default = "default_true")]
        enabled: bool,
    }

    let root: Root = toml::from_str(contents).map_err(|e| e.to_string())?;
    Ok(root
        .provider
        .into_iter()
        .map(|r| ProviderSeed {
            name: r
                .name
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| r.host.trim().to_string()),
            host: r.host.trim().to_string(),
            port: r.port,
            username: r.username.trim().to_string(),
            password: r.password,
            enabled: r.enabled,
        })
        .collect())
}

/// Serialize provider seeds back to the credentials-file format (used by export).
pub fn serialize_providers_toml(seeds: &[ProviderSeed]) -> String {
    let mut out = String::from(
        "# fluxxx providers — place next to fluxxx.exe to preload these on launch.\n\
         # WARNING: passwords are stored here in plaintext.\n\n",
    );
    for s in seeds {
        out.push_str("[[provider]]\n");
        out.push_str(&format!("name = {}\n", toml_str(&s.name)));
        out.push_str(&format!("host = {}\n", toml_str(&s.host)));
        out.push_str(&format!("port = {}\n", s.port));
        out.push_str(&format!("username = {}\n", toml_str(&s.username)));
        out.push_str(&format!("password = {}\n", toml_str(&s.password)));
        out.push_str(&format!("enabled = {}\n\n", s.enabled));
    }
    out
}

/// Quote a string as a TOML basic string, escaping backslashes and quotes.
fn toml_str(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_providers() {
        let toml = r#"
            [[provider]]
            name = "Main"
            host = "http://a.example.com"
            port = 8080
            username = "u1"
            password = "p1"
            enabled = false

            [[provider]]
            host = "b.example.com"
            port = 443
            username = "u2"
            password = "p2"
        "#;
        let seeds = parse_providers_toml(toml).unwrap();
        assert_eq!(seeds.len(), 2);
        assert_eq!(seeds[0].name, "Main");
        assert!(!seeds[0].enabled);
        // Defaults: name falls back to host, enabled defaults true.
        assert_eq!(seeds[1].name, "b.example.com");
        assert_eq!(seeds[1].port, 443);
        assert!(seeds[1].enabled);
    }

    #[test]
    fn empty_or_missing_table_is_ok() {
        assert!(parse_providers_toml("").unwrap().is_empty());
        assert!(parse_providers_toml("# just a comment\n").unwrap().is_empty());
    }

    #[test]
    fn malformed_is_error() {
        // Missing required `password`.
        let toml = r#"
            [[provider]]
            host = "x"
            port = 80
            username = "u"
        "#;
        assert!(parse_providers_toml(toml).is_err());
        // Not even valid TOML.
        assert!(parse_providers_toml("[[provider]] host=").is_err());
    }

    #[test]
    fn round_trips_through_serialize() {
        let seeds = vec![ProviderSeed {
            name: "My \"IPTV\"".into(),
            host: "example.com".into(),
            port: 443,
            username: "user".into(),
            password: "p@ss\\word".into(),
            enabled: true,
        }];
        let text = serialize_providers_toml(&seeds);
        let back = parse_providers_toml(&text).unwrap();
        assert_eq!(back, seeds);
    }
}
