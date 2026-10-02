//! The runtime catalog: the audience-visible set of games and the query the
//! Library screen runs against it (docs/ui/screens/01-library.md).
//!
//! The catalog holds erased games only. It never exposes a typed game, so the
//! shell cannot acquire one (I-9).

use std::{fmt, sync::Arc};

use tabula_game_api::{
    metadata::{Category, Complexity, I18nKey},
    GameId,
};

use crate::{
    availability::{LaunchMode, ModeSupport},
    erased::ErasedGame,
};

/// Resolves an [`I18nKey`] for the viewer's current locale.
///
/// Search runs over *localized* text, so the catalog cannot do it without the
/// shell's message table; injecting the lookup keeps the policy here and the
/// wording there.
pub trait Localizer {
    fn text(&self, key: &I18nKey) -> Option<&str>;
}

/// One registered game together with the facts the Library lists.
#[derive(Clone)]
pub struct CatalogEntry {
    game: Arc<dyn ErasedGame>,
}

impl fmt::Debug for CatalogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CatalogEntry")
            .field("id", &self.id().as_str())
            .finish()
    }
}

impl CatalogEntry {
    #[must_use]
    pub fn game(&self) -> &dyn ErasedGame {
        self.game.as_ref()
    }

    #[must_use]
    pub fn id(&self) -> &'static GameId {
        self.game.metadata().id()
    }

    /// Modes this build can actually start, in declaration order.
    #[must_use]
    pub fn modes(&self) -> &'static [ModeSupport] {
        self.game.modes()
    }

    /// Whether any mode can start a match for this game right now.
    #[must_use]
    pub fn startable(&self) -> bool {
        self.modes().iter().any(ModeSupport::is_available)
    }
}

/// The registered games, ordered for display.
#[derive(Clone, Default)]
pub struct Catalog {
    entries: Vec<CatalogEntry>,
}

impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.entries).finish()
    }
}

/// Structural and textual constraints from the Library toolbar.
///
/// Every field is an independent axis; constraints combine with AND and an
/// empty value removes the constraint.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogQuery {
    pub text: Option<String>,
    pub category: Option<Category>,
    /// Exact seat count the game must support, including sets with gaps.
    pub players: Option<u8>,
    /// Estimated *maximum* minutes must not exceed this budget. Not a timer.
    pub max_minutes: Option<u16>,
    pub complexity: Option<Complexity>,
    pub mode: Option<LaunchMode>,
}

impl CatalogQuery {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl Catalog {
    /// The catalog this build links, ordered by localized title then game id.
    ///
    /// Ordering is resolved once, against the supplied locale, so the list does
    /// not reorder while the viewer types.
    #[must_use]
    pub fn new(games: Vec<Arc<dyn ErasedGame>>, localizer: &dyn Localizer) -> Self {
        let mut entries: Vec<CatalogEntry> = games
            .into_iter()
            .map(|game| CatalogEntry { game })
            .collect();
        entries.sort_by(|left, right| {
            let left_title = localized(localizer, left.game.metadata().name_key());
            let right_title = localized(localizer, right.game.metadata().name_key());
            fold(&left_title)
                .cmp(&fold(&right_title))
                .then_with(|| left.id().as_str().cmp(right.id().as_str()))
        });
        Self { entries }
    }

    /// Every message this build needs for one locale: the platform's own copy
    /// followed by each linked game's table.
    ///
    /// The shell merges this with its own navigation and task copy. It never
    /// holds a game's keys itself, which is what keeps game identity out of
    /// shell source (I-9).
    #[must_use]
    pub fn messages(&self, locale: crate::i18n::Locale) -> Vec<(&'static str, &'static str)> {
        let mut table: Vec<(&'static str, &'static str)> =
            crate::i18n::platform_messages(locale).to_vec();
        for entry in &self.entries {
            table.extend_from_slice(entry.game.messages(locale));
        }
        table
    }

    #[must_use]
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look a game up by its validated full identifier.
    ///
    /// An unknown but well-formed id is a not-found state, never a fallback
    /// game (docs/ui/screens/discovery.md).
    #[must_use]
    pub fn get(&self, id: &GameId) -> Option<&CatalogEntry> {
        self.entries.iter().find(|entry| entry.id() == id)
    }

    /// Apply the Library constraints, preserving catalog order.
    #[must_use]
    pub fn query(&self, query: &CatalogQuery, localizer: &dyn Localizer) -> Vec<&CatalogEntry> {
        self.entries
            .iter()
            .filter(|entry| matches_query(entry, query, localizer))
            .collect()
    }
}

fn matches_query(entry: &CatalogEntry, query: &CatalogQuery, localizer: &dyn Localizer) -> bool {
    let metadata = entry.game.metadata();
    let capabilities = entry.game.capabilities();

    if let Some(category) = query.category {
        if !metadata.categories().contains(&category) {
            return false;
        }
    }
    if let Some(players) = query.players {
        if !capabilities.seats().allowed().contains(players) {
            return false;
        }
    }
    if let Some(budget) = query.max_minutes {
        if metadata.estimated_minutes().max() > budget {
            return false;
        }
    }
    if let Some(complexity) = query.complexity {
        if metadata.complexity() != complexity {
            return false;
        }
    }
    if let Some(mode) = query.mode {
        if !entry
            .modes()
            .iter()
            .any(|support| support.mode == mode && support.is_available())
        {
            return false;
        }
    }
    if let Some(text) = &query.text {
        let needle = fold(text);
        if !needle.is_empty() && !matches_text(entry, &needle, localizer) {
            return false;
        }
    }
    true
}

/// Search policy: localized name and tagline, plus the module's declared tags.
///
/// Matching is case-insensitive and diacritic-insensitive so a Vietnamese
/// viewer typing without tone marks still finds the game; see [`fold`].
fn matches_text(entry: &CatalogEntry, needle: &str, localizer: &dyn Localizer) -> bool {
    let metadata = entry.game.metadata();
    let haystacks = [
        localized(localizer, metadata.name_key()),
        localized(localizer, metadata.tagline_key()),
    ];
    haystacks.iter().any(|text| fold(text).contains(needle))
        || metadata.tags().iter().any(|tag| fold(tag).contains(needle))
}

fn localized(localizer: &dyn Localizer, key: &I18nKey) -> String {
    localizer
        .text(key)
        .map_or_else(|| key.as_str().to_owned(), ToOwned::to_owned)
}

/// Lowercase and strip Vietnamese tone marks for search and ordering.
///
/// Implemented here rather than with a Unicode normalization dependency: the
/// folded set is exactly the Latin and Vietnamese letters the catalog's
/// localized titles use, and adding a dependency to a contract-tier crate needs
/// an ADR (doc 00 §8).
#[must_use]
pub fn fold(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ă' | 'ạ' | 'ả' | 'ấ' | 'ầ' | 'ẩ' | 'ẫ' | 'ậ'
            | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'ẹ' | 'ẻ' | 'ẽ' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => {
                'e'
            }
            'ì' | 'í' | 'î' | 'ï' | 'ị' | 'ỉ' | 'ĩ' => 'i',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ơ' | 'ọ' | 'ỏ' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ớ'
            | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ư' | 'ụ' | 'ủ' | 'ũ' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => {
                'u'
            }
            'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
            'đ' => 'd',
            other => other,
        })
        .collect()
}
