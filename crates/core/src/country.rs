//! Infer a country from an IPTV category name so channels can be rolled up and
//! toggled by country (e.g. "UK | Sports" and "United Kingdom Movies" → GB).
//!
//! Strategy, in order:
//!   1. A flag emoji anywhere in the name (🇬🇧 → GB), if it maps to a known country.
//!   2. The leading token before a delimiter (`|`, `:`, `-`, `/`, …) matched
//!      against an alias table.
//!   3. Any alias found as a whole word elsewhere in the name.
//! Unmatched names return `None` and are surfaced to the user as "Other".

/// A resolved country: ISO 3166-1 alpha-2 code plus a display name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Country {
    pub code: &'static str,
    pub name: &'static str,
}

/// `(code, display_name, aliases)`. Aliases are matched case-insensitively.
/// Order matters only for display; matching prefers the longest alias hit.
const TABLE: &[(&str, &str, &[&str])] = &[
    ("US", "United States", &["US", "USA", "U.S.", "UNITED STATES", "AMERICA", "AMERICAN"]),
    ("GB", "United Kingdom", &["UK", "GB", "U.K.", "UNITED KINGDOM", "BRITAIN", "BRITISH", "ENGLAND"]),
    ("CA", "Canada", &["CA", "CAN", "CANADA", "CANADIAN"]),
    ("IE", "Ireland", &["IE", "IRE", "IRELAND", "IRISH"]),
    ("AU", "Australia", &["AU", "AUS", "AUSTRALIA", "AUSTRALIAN"]),
    ("NZ", "New Zealand", &["NZ", "NEW ZEALAND"]),
    ("FR", "France", &["FR", "FRA", "FRANCE", "FRENCH", "FRANCAIS", "FRANÇAIS"]),
    ("DE", "Germany", &["DE", "GER", "DEU", "GERMANY", "GERMAN", "DEUTSCH", "DEUTSCHLAND"]),
    ("ES", "Spain", &["ES", "ESP", "SPAIN", "SPANISH", "ESPANA", "ESPAÑA"]),
    ("IT", "Italy", &["IT", "ITA", "ITALY", "ITALIAN", "ITALIA"]),
    ("PT", "Portugal", &["PT", "POR", "PORTUGAL", "PORTUGUESE"]),
    ("NL", "Netherlands", &["NL", "NLD", "NETHERLANDS", "DUTCH", "HOLLAND"]),
    ("BE", "Belgium", &["BE", "BEL", "BELGIUM", "BELGIQUE"]),
    ("CH", "Switzerland", &["CH", "CHE", "SWITZERLAND", "SWISS", "SUISSE"]),
    ("AT", "Austria", &["AT", "AUT", "AUSTRIA", "AUSTRIAN"]),
    ("SE", "Sweden", &["SE", "SWE", "SWEDEN", "SWEDISH"]),
    ("NO", "Norway", &["NO", "NOR", "NORWAY", "NORWEGIAN"]),
    ("DK", "Denmark", &["DK", "DEN", "DNK", "DENMARK", "DANISH"]),
    ("FI", "Finland", &["FI", "FIN", "FINLAND", "FINNISH"]),
    ("PL", "Poland", &["PL", "POL", "POLAND", "POLISH", "POLSKA"]),
    ("RO", "Romania", &["RO", "ROU", "ROMANIA", "ROMANIAN", "ROMANA"]),
    ("RU", "Russia", &["RU", "RUS", "RUSSIA", "RUSSIAN"]),
    ("UA", "Ukraine", &["UA", "UKR", "UKRAINE", "UKRAINIAN"]),
    ("GR", "Greece", &["GR", "GRE", "GREECE", "GREEK"]),
    ("TR", "Turkey", &["TR", "TUR", "TURKEY", "TURKISH", "TURKIYE", "TÜRKIYE"]),
    ("BR", "Brazil", &["BR", "BRA", "BRAZIL", "BRAZILIAN", "BRASIL"]),
    ("AR", "Argentina", &["ARG", "ARGENTINA"]),
    ("MX", "Mexico", &["MX", "MEX", "MEXICO", "MEXICAN"]),
    ("CO", "Colombia", &["CO", "COL", "COLOMBIA"]),
    ("CL", "Chile", &["CL", "CHL", "CHILE"]),
    ("IN", "India", &["IN", "IND", "INDIA", "INDIAN", "HINDI"]),
    ("PK", "Pakistan", &["PK", "PAK", "PAKISTAN"]),
    ("BD", "Bangladesh", &["BD", "BGD", "BANGLADESH", "BANGLA"]),
    ("PH", "Philippines", &["PH", "PHL", "PHILIPPINES", "FILIPINO", "PINOY"]),
    ("ID", "Indonesia", &["ID", "IDN", "INDONESIA", "INDONESIAN"]),
    ("MY", "Malaysia", &["MY", "MYS", "MALAYSIA", "MALAY"]),
    ("JP", "Japan", &["JP", "JPN", "JAPAN", "JAPANESE"]),
    ("KR", "South Korea", &["KR", "KOR", "KOREA", "KOREAN"]),
    ("CN", "China", &["CN", "CHN", "CHINA", "CHINESE"]),
    ("ZA", "South Africa", &["ZA", "ZAF", "SOUTH AFRICA"]),
    ("EG", "Egypt", &["EG", "EGY", "EGYPT"]),
    ("MA", "Morocco", &["MA", "MAR", "MOROCCO", "MAROC"]),
    ("SA", "Saudi Arabia", &["SA", "SAU", "SAUDI", "KSA"]),
    ("AE", "United Arab Emirates", &["AE", "UAE", "EMIRATES"]),
    // "ARABIC" is a language spanning many countries; map to a pseudo-grouping.
    ("AR-LANG", "Arabic", &["ARABIC", "ARAB", "OSN", "MBC"]),
    ("EX-YU", "Ex-Yugoslavia", &["EX-YU", "EXYU", "EX YU", "BALKAN", "SRBIJA", "SERBIA", "CROATIA", "HRVATSKA", "BOSNA", "BOSNIA"]),
];

/// Infer the country of an IPTV category/group name.
pub fn infer_country(name: &str) -> Option<Country> {
    if name.trim().is_empty() {
        return None;
    }

    // 1) Flag emoji anywhere.
    if let Some(code) = flag_to_code(name) {
        if let Some(c) = lookup(&code) {
            return Some(c);
        }
    }

    let upper = name.to_uppercase();

    // 2) Leading token before a delimiter.
    let lead = leading_token(&upper);
    if let Some(c) = match_alias(&lead) {
        return Some(c);
    }

    // 3) Longest whole-word alias anywhere in the string.
    match_anywhere(&upper)
}

/// Return the display name for a stored code, or the code itself if unknown.
pub fn name_for_code(code: &str) -> &str {
    lookup(code).map(|c| c.name).unwrap_or(code)
}

fn lookup(code: &str) -> Option<Country> {
    TABLE
        .iter()
        .find(|(c, _, _)| c.eq_ignore_ascii_case(code))
        .map(|(c, n, _)| Country { code: c, name: n })
}

/// Grab the leading token, stripping common IPTV prefixes and delimiters.
fn leading_token(upper: &str) -> String {
    let cleaned: String = upper
        .chars()
        .map(|ch| if is_delim(ch) { ' ' } else { ch })
        .collect();
    cleaned
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

fn is_delim(ch: char) -> bool {
    matches!(ch, '|' | ':' | '-' | '—' | '/' | '\\' | '.' | '[' | ']' | '(' | ')' | '*' | '>' )
        || ch.is_whitespace()
}

/// Exact alias match for a single token (e.g. "UK", "USA").
fn match_alias(token: &str) -> Option<Country> {
    if token.is_empty() {
        return None;
    }
    for (code, name, aliases) in TABLE {
        for a in *aliases {
            if a.eq_ignore_ascii_case(token) {
                return Some(Country { code, name });
            }
        }
    }
    None
}

/// Find the longest alias that appears as a whole word anywhere.
fn match_anywhere(upper: &str) -> Option<Country> {
    let mut best: Option<(usize, Country)> = None;
    for (code, name, aliases) in TABLE {
        for a in *aliases {
            // Only multi-char aliases here to avoid false positives from stray
            // 2-letter substrings mid-word; the leading-token pass already
            // handles short codes.
            if a.len() >= 4 && contains_word(upper, a) {
                let cand = Country { code, name };
                if best.map(|(len, _)| a.len() > len).unwrap_or(true) {
                    best = Some((a.len(), cand));
                }
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Whole-word containment (bounded by non-alphanumeric or string edges).
fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(needle) {
        let start = from + pos;
        let end = start + needle.len();
        let before_ok = start == 0
            || !haystack[..start]
                .chars()
                .next_back()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        let after_ok = end == haystack.len()
            || !haystack[end..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
        if from >= haystack.len() {
            break;
        }
    }
    false
}

/// Convert the first regional-indicator flag emoji found into an ISO code.
fn flag_to_code(s: &str) -> Option<String> {
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if let Some(a) = regional_indicator(c) {
            if let Some(&n) = it.peek() {
                if let Some(b) = regional_indicator(n) {
                    return Some(format!("{a}{b}"));
                }
            }
        }
    }
    None
}

fn regional_indicator(c: char) -> Option<char> {
    let u = c as u32;
    if (0x1F1E6..=0x1F1FF).contains(&u) {
        Some((b'A' + (u - 0x1F1E6) as u8) as char)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(name: &str) -> Option<&'static str> {
        infer_country(name).map(|c| c.code)
    }

    #[test]
    fn leading_prefix_pipe() {
        assert_eq!(code("UK | ENTERTAINMENT"), Some("GB"));
        assert_eq!(code("US | NEWS HD"), Some("US"));
        assert_eq!(code("FR - CINEMA"), Some("FR"));
        assert_eq!(code("DE: SPORT"), Some("DE"));
    }

    #[test]
    fn full_country_names() {
        assert_eq!(code("United Kingdom Movies"), Some("GB"));
        assert_eq!(code("GERMANY DOCUMENTARY"), Some("DE"));
        assert_eq!(code("Canal Portugal Desporto"), Some("PT"));
    }

    #[test]
    fn flag_emoji() {
        assert_eq!(code("🇬🇧 Sports"), Some("GB"));
        assert_eq!(code("Movies 🇫🇷 VIP"), Some("FR"));
    }

    #[test]
    fn unknown_is_none() {
        assert_eq!(code("VIP PREMIUM 24/7"), None);
        assert_eq!(code(""), None);
        assert_eq!(code("   "), None);
    }

    #[test]
    fn no_false_positive_substring() {
        // "USA" alias must not fire inside "MEDUSA".
        assert_eq!(code("MEDUSA CHANNEL"), None);
    }

    #[test]
    fn language_grouping() {
        assert_eq!(code("ARABIC | GENERAL"), Some("AR-LANG"));
    }

    #[test]
    fn name_lookup() {
        assert_eq!(name_for_code("GB"), "United Kingdom");
        assert_eq!(name_for_code("ZZ"), "ZZ");
    }
}
