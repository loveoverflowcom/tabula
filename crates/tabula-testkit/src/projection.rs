//! Projection safety — the security test category. (doc 02 §7.3, doc 00 §4.2)
//!
//! # This is not a gameplay test
//!
//! If a secret can be derived from what a client receives, the projection is
//! broken and that is a **security defect**. It gets its own test category
//! precisely so it never gets triaged as a normal bug.
//!
//! # How the scan works
//!
//! Every game with `hidden_information = true` declares a [`SecretModel`]:
//! what is secret and to whom. The testkit then runs, over random states drawn
//! from random self-play games:
//!
//! ```text
//! for every secret S, for every viewer V not authorised for S:
//!     assert!( !encode(project(state, V)).contains_tokens(S) )
//!     assert!( events.all(|e| !encode(view_event(state, e, V)).contains_tokens(S)) )
//! ```
//!
//! Token-level containment scanning is **coarse but catches the real bugs**: a
//! whole card list leaking, a role map serialised wholesale. Those are the
//! failures that actually ship. It runs on every PR for every
//! hidden-information game.
//!
//! # What it deliberately does not catch
//!
//! *Derived* secrets — where two public values combine to reveal a hidden one
//! (exact deck count + all discards + your hand = the opponent's hand in a
//! 52-card game). No scanner finds that. The mitigation is documentation: games
//! with hidden information must write an **information model** in
//! `docs/games/<slug>.md` listing what is intentionally derivable, and a human
//! reviews it. (doc 02 §7.1)
//!
//! # Spectators are not "player 0"
//!
//! The most common projection bug in board-game platforms is spectators seeing
//! hidden hands. The scan checks `Viewer::Spectator(Live)` and
//! `Viewer::Spectator(Delayed)` explicitly, as separate viewers.

use tabula_core::Viewer;
use tabula_game_api::GameRules;

/// What a game keeps secret, and from whom.
///
/// Implemented by every game with `hidden_information = true`. Games without
/// hidden information do not implement it and the scan is skipped.
///
/// ```rust,ignore
/// impl SecretModel for CardsRules {
///     fn secrets(state: &State) -> Vec<Secret> {
///         let mut v = vec![Secret::nobody(state.deck.tokens())];
///         for (seat, hand) in state.hands.iter() {
///             v.push(Secret::authorized(hand.tokens(), Viewer::Seat(*seat)));
///         }
///         v
///     }
/// }
/// ```
pub trait SecretModel: GameRules {
    fn secrets(state: &Self::State) -> Vec<Secret>;
}

/// One secret: some tokens, and the viewers allowed to see them.
#[derive(Clone, Debug)]
pub struct Secret {
    /// Canonically-encoded fragments whose presence in a projection is a leak.
    ///
    /// TODO(phase 3): decide the token granularity when cards is written. A card
    /// is a token; a whole hand is a sequence of tokens. Too coarse and the scan
    /// misses single-card leaks; too fine and it false-positives on legitimately
    /// public cards. Cards is the game that will settle it.
    pub tokens: Vec<Vec<u8>>,

    /// Empty means **nobody** may see it — a deck order, or a salt before reveal.
    pub authorized: Vec<Viewer>,

    /// Human-readable, for the failure message: "seat 2's hand".
    pub label: String,
}

impl Secret {
    /// Secret from everyone. Deck order, unrevealed salts.
    #[must_use]
    pub fn nobody(label: &str, tokens: Vec<Vec<u8>>) -> Self {
        Self {
            tokens,
            authorized: Vec::new(),
            label: label.to_owned(),
        }
    }

    /// Secret from everyone except the listed viewers.
    #[must_use]
    pub fn authorized(label: &str, tokens: Vec<Vec<u8>>, who: Vec<Viewer>) -> Self {
        Self {
            tokens,
            authorized: who,
            label: label.to_owned(),
        }
    }
}

/// Run the scan for one state and its events. (doc 02 §11.1 `projection_hides_secrets`)
///
/// TODO(phase 3): implement alongside cards. The failure message must name the
/// secret label, the viewer, and the input index — "seat 2's hand appeared in
/// the Spectator(Live) projection at input 37" is actionable; "leak detected"
/// is not.
///
/// # Panics
/// On any leak.
pub fn assert_no_leaks<R: SecretModel>(_state: &R::State) {
    todo!("doc 02 §7.3: scan project() and view_event() for every unauthorized viewer")
}

/// Every canonical event must pass through `view_event` for every viewer. (I-6)
///
/// Catches the failure where a new `Event` variant is added and a `match` arm in
/// `view_event` silently falls through to a catch-all that returns `Some(..)`.
pub fn assert_no_event_bypasses_redaction<R: GameRules>(_state: &R::State, _events: &[R::Event]) {
    todo!("doc 02 §11.1 view_event_never_bypasses")
}
