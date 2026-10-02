//! The Library's query state: the toolbar's constraints as URL parameters.
//!
//! Query state carries no token, identity, or draft payload. An unreadable
//! value is a visible, recoverable filter error — never a silently different
//! selection (docs/ui/screens/discovery.md, "Routes, aliases, and navigation").

use tabula_registry::{CatalogQuery, Category, Complexity, LaunchMode};

/// The result of reading the address bar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    pub query: CatalogQuery,
    /// Label keys of the axes whose value could not be read. Each is reported
    /// to the viewer and left unapplied.
    pub invalid: Vec<&'static str>,
}

/// Read the catalog constraints from `key=value` pairs.
///
/// An empty value removes its constraint, matching "empty values remove the
/// constraint" in the shared contract.
pub fn parse<'a>(pairs: impl Iterator<Item = (&'a str, &'a str)>) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    for (key, value) in pairs {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key {
            "q" => parsed.query.text = Some(value.to_owned()),
            "category" => match category(value) {
                Some(category) => parsed.query.category = Some(category),
                None => parsed.invalid.push("library.filter.category"),
            },
            "players" => match value.parse::<u8>() {
                Ok(players) if players > 0 => parsed.query.players = Some(players),
                _ => parsed.invalid.push("library.filter.players"),
            },
            "duration" => match value.parse::<u16>() {
                Ok(minutes) if minutes > 0 => parsed.query.max_minutes = Some(minutes),
                _ => parsed.invalid.push("library.filter.duration"),
            },
            "complexity" => match complexity(value) {
                Some(complexity) => parsed.query.complexity = Some(complexity),
                None => parsed.invalid.push("library.filter.complexity"),
            },
            "mode" => match LaunchMode::parse(value) {
                Some(mode) => parsed.query.mode = Some(mode),
                None => parsed.invalid.push("library.filter.mode"),
            },
            _ => {}
        }
    }
    parsed
}

/// Render the constraints back into a query string, without the leading `?`.
#[must_use]
pub fn to_query_string(query: &CatalogQuery) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(text) = &query.text {
        if !text.trim().is_empty() {
            parts.push(format!("q={}", encode(text.trim())));
        }
    }
    if let Some(category) = query.category {
        parts.push(format!("category={}", category_value(category)));
    }
    if let Some(players) = query.players {
        parts.push(format!("players={players}"));
    }
    if let Some(minutes) = query.max_minutes {
        parts.push(format!("duration={minutes}"));
    }
    if let Some(complexity) = query.complexity {
        parts.push(format!("complexity={}", complexity_value(complexity)));
    }
    if let Some(mode) = query.mode {
        parts.push(format!("mode={}", mode.as_str()));
    }
    parts.join("&")
}

/// `/games` with the current constraints applied.
#[must_use]
pub fn library_href(query: &CatalogQuery) -> String {
    let rendered = to_query_string(query);
    if rendered.is_empty() {
        "/games".to_owned()
    } else {
        format!("/games?{rendered}")
    }
}

pub const CATEGORIES: [Category; 5] = [
    Category::Abstract,
    Category::Cards, // xtask-allow-game-id: a catalog genre, not a game package
    Category::SocialDeduction,
    Category::TilePlacement,
    Category::Party,
];

pub const COMPLEXITIES: [Complexity; 3] =
    [Complexity::Light, Complexity::Medium, Complexity::Heavy];

#[must_use]
pub const fn category_value(category: Category) -> &'static str {
    match category {
        Category::Abstract => "abstract",
        Category::Cards => "cards", // xtask-allow-game-id: a catalog genre
        Category::SocialDeduction => "social_deduction",
        Category::TilePlacement => "tile_placement",
        Category::Party => "party",
    }
}

#[must_use]
pub const fn category_label_key(category: Category) -> &'static str {
    match category {
        Category::Abstract => "category.abstract",
        Category::Cards => "category.cards", // xtask-allow-game-id: a catalog genre
        Category::SocialDeduction => "category.social_deduction",
        Category::TilePlacement => "category.tile_placement",
        Category::Party => "category.party",
    }
}

#[must_use]
pub const fn complexity_value(complexity: Complexity) -> &'static str {
    match complexity {
        Complexity::Light => "light",
        Complexity::Medium => "medium",
        Complexity::Heavy => "heavy",
    }
}

#[must_use]
pub const fn complexity_label_key(complexity: Complexity) -> &'static str {
    match complexity {
        Complexity::Light => "complexity.light",
        Complexity::Medium => "complexity.medium",
        Complexity::Heavy => "complexity.heavy",
    }
}

fn category(value: &str) -> Option<Category> {
    CATEGORIES
        .into_iter()
        .find(|candidate| category_value(*candidate) == value)
}

fn complexity(value: &str) -> Option<Complexity> {
    COMPLEXITIES
        .into_iter()
        .find(|candidate| complexity_value(*candidate) == value)
}

fn encode(value: &str) -> String {
    use core::fmt::Write as _;

    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            other => {
                let _ = write!(out, "%{other:02X}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{library_href, parse, to_query_string, ParsedQuery};
    use tabula_registry::{CatalogQuery, Category, Complexity, LaunchMode};

    fn read(pairs: &[(&str, &str)]) -> ParsedQuery {
        parse(pairs.iter().copied())
    }

    #[test]
    fn every_axis_round_trips_through_the_address() {
        let query = CatalogQuery {
            text: Some("cờ".to_owned()),
            category: Some(Category::Abstract),
            players: Some(2),
            max_minutes: Some(45),
            complexity: Some(Complexity::Heavy),
            mode: Some(LaunchMode::LocalBots),
        };
        let rendered = to_query_string(&query);
        let pairs: Vec<(&str, &str)> = rendered
            .split('&')
            .filter_map(|part| part.split_once('='))
            .collect();
        let back = parse(pairs.into_iter());
        assert!(back.invalid.is_empty());
        // The text arrives percent-encoded; every structural axis survives.
        assert_eq!(back.query.category, query.category);
        assert_eq!(back.query.players, query.players);
        assert_eq!(back.query.max_minutes, query.max_minutes);
        assert_eq!(back.query.complexity, query.complexity);
        assert_eq!(back.query.mode, query.mode);
    }

    #[test]
    fn an_unreadable_value_is_reported_and_left_unapplied() {
        let parsed = read(&[
            ("category", "roguelike"),
            ("players", "zero"),
            ("duration", "-5"),
            ("complexity", "brutal"),
            ("mode", "telepathy"),
        ]);
        assert_eq!(parsed.query, CatalogQuery::default());
        assert_eq!(
            parsed.invalid,
            vec![
                "library.filter.category",
                "library.filter.players",
                "library.filter.duration",
                "library.filter.complexity",
                "library.filter.mode",
            ]
        );
    }

    #[test]
    fn a_zero_count_is_an_error_rather_than_a_silent_removal() {
        let parsed = read(&[("players", "0"), ("duration", "0")]);
        assert_eq!(parsed.query.players, None);
        assert_eq!(parsed.query.max_minutes, None);
        assert_eq!(parsed.invalid.len(), 2);
    }

    #[test]
    fn an_empty_value_removes_its_constraint_without_an_error() {
        let parsed = read(&[("q", "   "), ("players", ""), ("category", "")]);
        assert!(parsed.query.is_empty());
        assert!(parsed.invalid.is_empty());
    }

    #[test]
    fn an_unknown_key_is_ignored() {
        let parsed = read(&[("sort", "rating"), ("players", "2")]);
        assert_eq!(parsed.query.players, Some(2));
        assert!(parsed.invalid.is_empty());
    }

    #[test]
    fn an_empty_query_links_to_the_bare_library_route() {
        assert_eq!(library_href(&CatalogQuery::default()), "/games");
        assert_eq!(
            library_href(&CatalogQuery {
                players: Some(3),
                ..CatalogQuery::default()
            }),
            "/games?players=3"
        );
    }
}
