// Theme inference for categories that aren't tied to a country (4K, Sports,
// Music, PPV, …). Keeps non-country groups from collapsing into one "Other"
// bucket. Pure string matching; used by the curator and the Live TV filter.

export type Theme =
  | "4K / UHD"
  | "Sports"
  | "Movies"
  | "Music"
  | "News"
  | "Kids"
  | "PPV / Events"
  | "Documentary"
  | "Entertainment"
  | "24/7"
  | "Adult"
  | "Ungrouped";

// Order matters: earlier matches win (e.g. "4K SPORTS" → 4K/UHD).
const RULES: [Theme, RegExp][] = [
  ["4K / UHD", /\b(4k|uhd|2160p?|ultra\s*hd)\b/i],
  ["PPV / Events", /\b(ppv|pay[\s-]*per[\s-]*view|event|fight night|boxing|ufc|wwe|wrestling)\b/i],
  ["Sports", /\b(sport|sports|espn|dazn|sky\s*sport|bein|nfl|nba|mlb|nhl|soccer|football|f1|golf|tennis|rugby|cricket)\b/i],
  ["Music", /\b(music|mtv|vevo|vh1|kiss|trace|clubbing|radio)\b/i],
  ["News", /\b(news|cnn|bbc\s*news|fox\s*news|msnbc|sky\s*news|al\s*jazeera)\b/i],
  ["Kids", /\b(kids|cartoon|disney|nick|nickelodeon|baby|junior|boomerang)\b/i],
  ["Documentary", /\b(documentar(y|ies)|docu|discovery|history|nat\s*geo|national\s*geographic|animal\s*planet)\b/i],
  ["Movies", /\b(movie|movies|cinema|film|films|hbo|cinemax)\b/i],
  ["24/7", /\b(24[\s/._-]?7|24hrs?)\b/i],
  ["Adult", /\b(adult|xxx|18\+|\+18|porn|brazzers)\b/i],
  ["Entertainment", /\b(entertainment|general|vip|premium|comedy|lifestyle|reality|drama)\b/i],
];

/** Infer a theme from a category name, or "Ungrouped" if nothing matches. */
export function inferTheme(name: string): Theme {
  for (const [theme, re] of RULES) {
    if (re.test(name)) return theme;
  }
  return "Ungrouped";
}

/**
 * Heuristic "is this an HDR stream?" — Xtream gives no HDR flag and we don't
 * probe the stream, so we match the channel/category text against user-tunable
 * keywords (default HDR/DV/Dolby Vision). Word-ish, case-insensitive.
 */
export function isHdr(text: string, keywords: string[]): boolean {
  if (!text) return false;
  const hay = text.toLowerCase();
  return keywords.some((k) => {
    const kw = k.trim().toLowerCase();
    return kw.length > 0 && hay.includes(kw);
  });
}
