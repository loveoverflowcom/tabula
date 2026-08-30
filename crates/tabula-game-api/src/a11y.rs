//! The accessibility mirror — "Board Reader". (doc 04 §10.4)
//!
//! A game's `describe()` produces a text/tree description of the current view for
//! screen readers and for players who cannot use the visual board at all.
//!
//! ## Phasing
//!
//! - **Phase 5**: `status` + `actions`. Enough to announce state changes and
//!   drive a keyboard-only game.
//! - **Phase 9**: full `regions` navigation — "north of the monastery at C4".
//!   Tiles is the hard case that will shape it.
//!
//! Keyboard play is **mandatory, not optional** (doc 04 §10.3): every game must
//! be completable with Tab/arrows/Enter/Esc. Games without `describe()` are
//! flagged in CI and may not be marked accessible in the catalog.

/// A screen-reader-ready mirror of a view.
#[derive(Clone, Debug, Default)]
pub struct A11yDescription {
    /// One line, announced on change:
    /// "Your turn. White to move. 3:12 remaining."
    pub status: String,

    /// Structured, navigable regions. Empty until Phase 9 for most games.
    pub regions: Vec<A11yRegion>,

    /// What can be done right now, with the activation path.
    pub actions: Vec<A11yAction>,
}

impl A11yDescription {
    /// The honest default for a game that has not implemented `describe()`.
    ///
    /// Returning this is fine during development. Shipping it is not — see the
    /// per-game definition of done (doc 08 §7.1).
    #[must_use]
    pub fn unsupported() -> Self {
        Self::default()
    }
}

/// A navigable group: "Board", "Your hand", "Players".
#[derive(Clone, Debug)]
pub struct A11yRegion {
    pub label: String,
    pub items: Vec<A11yItem>,
}

#[derive(Clone, Debug)]
pub struct A11yItem {
    /// What it is: "White knight".
    pub label: String,
    /// Where it is, in the game's own coordinate language: "C4".
    pub position: String,
    /// What is true of it right now: "selected", "under attack".
    pub state: String,
    /// Activating this item maps to this action, if any.
    pub activates: Option<ActionId>,
}

#[derive(Clone, Debug)]
pub struct A11yAction {
    pub id: ActionId,
    pub label: String,
    /// A disabled action must still be *explained* — doc 04 §9.4. Silence is
    /// worse than a greyed-out control with a reason.
    pub enabled: bool,
}

/// Opaque handle the presentation layer maps back to a concrete `Command`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActionId(pub String);
