//! Curation logic: decide what is "enabled" — and therefore fetched and cached —
//! and roll categories up into country groups for the setup wizard.
//!
//! The guiding rule (a locked design decision): a disabled provider or category
//! is *fully excluded*. [`epg_fetch_targets`] is the single source of truth for
//! "which channels do we spend network/cache on", so callers can never
//! accidentally pull EPG for something the user switched off.

use std::collections::HashMap;

use crate::model::{Category, Channel, Provider};

/// A country roll-up shown in the curation wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountryGroup {
    /// ISO code or pseudo-code; `None` for "Other" (uninferred).
    pub code: Option<String>,
    pub name: String,
    pub category_ids: Vec<i64>,
    pub channel_count: usize,
    pub enabled_categories: usize,
    pub total_categories: usize,
}

impl CountryGroup {
    /// True when every category in the group is enabled.
    pub fn fully_enabled(&self) -> bool {
        self.total_categories > 0 && self.enabled_categories == self.total_categories
    }
}

/// Group categories by inferred country, summing channel counts.
///
/// `channel_counts` maps a category's `id` to how many channels it holds.
/// Groups are returned sorted by descending channel count, with "Other" last.
pub fn group_by_country(
    categories: &[Category],
    channel_counts: &HashMap<i64, usize>,
) -> Vec<CountryGroup> {
    // key: Option<code> -> aggregate
    let mut map: HashMap<Option<String>, CountryGroup> = HashMap::new();

    for cat in categories {
        let key = cat.country_code.clone();
        let name = match &cat.country_code {
            Some(code) => crate::country::name_for_code(code).to_string(),
            None => "Other".to_string(),
        };
        let count = channel_counts.get(&cat.id).copied().unwrap_or(0);
        let entry = map.entry(key.clone()).or_insert_with(|| CountryGroup {
            code: key,
            name,
            category_ids: Vec::new(),
            channel_count: 0,
            enabled_categories: 0,
            total_categories: 0,
        });
        entry.category_ids.push(cat.id);
        entry.channel_count += count;
        entry.total_categories += 1;
        if cat.enabled {
            entry.enabled_categories += 1;
        }
    }

    let mut groups: Vec<CountryGroup> = map.into_values().collect();
    groups.sort_by(|a, b| {
        // "Other" always sinks to the bottom.
        match (a.code.is_none(), b.code.is_none()) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => b
                .channel_count
                .cmp(&a.channel_count)
                .then_with(|| a.name.cmp(&b.name)),
        }
    });
    groups
}

/// The channels whose EPG should be fetched/cached right now: those belonging to
/// an enabled provider and an enabled category. Channels with no category are
/// included as long as their provider is enabled (they can't be curated away by
/// country, but they still ride the provider toggle).
pub fn epg_fetch_targets<'a>(
    providers: &[Provider],
    categories: &[Category],
    channels: &'a [Channel],
) -> Vec<&'a Channel> {
    let enabled_providers: std::collections::HashSet<i64> =
        providers.iter().filter(|p| p.enabled).map(|p| p.id).collect();
    let enabled_categories: std::collections::HashSet<i64> = categories
        .iter()
        .filter(|c| c.enabled)
        .map(|c| c.id)
        .collect();

    channels
        .iter()
        .filter(|ch| enabled_providers.contains(&ch.provider_id))
        .filter(|ch| match ch.category_id {
            Some(cid) => enabled_categories.contains(&cid),
            None => true,
        })
        .filter(|ch| {
            ch.epg_channel_id
                .as_deref()
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: i64, enabled: bool) -> Provider {
        Provider {
            id,
            name: format!("p{id}"),
            host: "http://h".into(),
            port: 80,
            username: "u".into(),
            password: None,
            enabled,
        }
    }

    fn category(id: i64, provider_id: i64, code: Option<&str>, enabled: bool) -> Category {
        Category {
            id,
            provider_id,
            xtream_category_id: id.to_string(),
            name: format!("cat{id}"),
            country_code: code.map(|s| s.to_string()),
            enabled,
        }
    }

    fn channel(id: i64, provider_id: i64, category_id: Option<i64>, epg: Option<&str>) -> Channel {
        Channel {
            id,
            provider_id,
            stream_id: id,
            name: format!("ch{id}"),
            category_id,
            epg_channel_id: epg.map(|s| s.to_string()),
            logo: None,
            num: None,
        }
    }

    #[test]
    fn groups_countries_and_sorts() {
        let cats = vec![
            category(1, 1, Some("GB"), true),
            category(2, 1, Some("GB"), false),
            category(3, 1, Some("US"), true),
            category(4, 1, None, true),
        ];
        let mut counts = HashMap::new();
        counts.insert(1, 10);
        counts.insert(2, 5);
        counts.insert(3, 50);
        counts.insert(4, 3);

        let groups = group_by_country(&cats, &counts);
        // US (50) first, GB (15) next, Other last.
        assert_eq!(groups[0].code.as_deref(), Some("US"));
        assert_eq!(groups[0].channel_count, 50);
        assert_eq!(groups[1].code.as_deref(), Some("GB"));
        assert_eq!(groups[1].channel_count, 15);
        assert_eq!(groups[1].enabled_categories, 1);
        assert_eq!(groups[1].total_categories, 2);
        assert!(!groups[1].fully_enabled());
        assert_eq!(groups.last().unwrap().name, "Other");
    }

    #[test]
    fn fetch_targets_respect_toggles() {
        let providers = vec![provider(1, true), provider(2, false)];
        let cats = vec![
            category(10, 1, Some("GB"), true),
            category(11, 1, Some("US"), false),
        ];
        let channels = vec![
            channel(100, 1, Some(10), Some("a.uk")), // enabled path -> included
            channel(101, 1, Some(11), Some("b.us")), // category disabled -> excluded
            channel(102, 1, Some(10), Some("")),      // empty epg id -> excluded
            channel(103, 1, None, Some("c.uk")),      // no category, provider on -> included
            channel(104, 2, Some(10), Some("d.uk")),  // provider disabled -> excluded
        ];
        let targets = epg_fetch_targets(&providers, &cats, &channels);
        let ids: Vec<i64> = targets.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![100, 103]);
    }
}
