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
//!
//! # Noninterference — the stronger, complementary property
//!
//! [`assert_no_leaks`] is a **containment** scan: it asks whether a secret's
//! own bytes appear in an unauthorized projection. That misses a *derived*
//! leak — a length, a count, or a checksum that changes only because hidden
//! data changed, without ever copying that data verbatim (§8.3 of the
//! `develop` architecture-verification audit calls these "Class 3" leaks and
//! notes no containment scanner can find them).
//!
//! [`assert_projection_noninterference`] and its positive-control counterpart
//! [`assert_projection_differs`] ask the stronger question directly: does an
//! unauthorized viewer's projection depend on hidden data **at all**? The
//! caller proves *by construction* that two canonical states differ only in
//! data a given [`Viewer`] is not authorized to see (this module has no way to
//! discover that on its own — see the module docs on [`SecretModel`] for why a
//! generic scanner cannot infer secrecy from a struct), and the assertion
//! checks that the viewer's canonically-encoded projection is byte-identical
//! (or, for the positive control, that it differs) between the two states.
//!
//! This is a property over **pairs of reachable states**, not a replacement
//! for [`assert_no_leaks`]: a wholesale leak and a derived leak are different
//! failure modes and this module keeps both oracles rather than merging them.

use tabula_core::{canonical_encode, Viewer};
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

/// Short diagnostic for a canonically-encoded projection: length plus a
/// content digest, so two failing runs of the same property test can be
/// compared without requiring `R::View: Debug` — see the note on
/// [`assert_projection_noninterference`] on why that bound is deliberately
/// absent from this module's public API.
fn digest(bytes: &[u8]) -> String {
    format!(
        "{} bytes, blake3 {}",
        bytes.len(),
        blake3::hash(bytes).to_hex()
    )
}

/// Assert that `viewer` cannot distinguish `state_a` from `state_b`.
///
/// This is the projection **noninterference** property (doc 00 §4.2): if two
/// canonical states differ only in facts `viewer` is not authorized to
/// observe, `project` must return an observably identical result for both.
/// "Observably identical" means the same canonical encoding
/// ([`canonical_encode`]) of the projected [`GameRules::View`] — this is the
/// oracle, and the only oracle; nothing here compares `Debug` output.
///
/// # Why there is no `R::View: Debug` bound
///
/// `GameRules::View` is declared as `Clone + Serialize + Send + Sync +
/// 'static` — no `Debug`. Requiring it here would mean a future
/// hidden-information game must add `#[derive(Debug)]` to its `View` for no
/// reason but this verification helper, which is exactly the kind of
/// incidental trait-widening this PR's own instructions warn against. The
/// failure message below reports byte length and a content digest of each
/// side instead of a structural dump — enough to tell two failures apart and
/// to compare against a re-run, without constraining every future `View`.
///
/// # What this does NOT verify on its own
///
/// This function trusts its caller completely: it has no way to know which
/// fields of `R::State` are secret, so it cannot check that `state_a` and
/// `state_b` actually differ only in unauthorized information. That proof is
/// the caller's job, normally discharged **by construction** — build both
/// states from the same public facts and only vary the part `viewer` may not
/// see, via legal transitions through `GameRules::create`/`apply` so the
/// states are actually reachable rather than merely representable (see
/// `crates/tabula-testkit/tests/projection_noninterference.rs` for the
/// worked example). A pair that differs in something `viewer` legitimately
/// would be authorized to see makes this assertion fail for the right
/// reason: use [`assert_projection_differs`] to state that case instead.
///
/// A passing call is one data point, not a theorem: see
/// [`crate::projection`] module docs for the residual gap this leaves
/// (derived leaks this exact pair of states does not happen to exercise) and
/// pair single examples with a property test that generates many pairs, as
/// the sibling `rust-property-testing` skill describes.
///
/// # Panics
/// If the two projections encode to different canonical bytes. The message
/// names `case`, the viewer, and each projection's byte length and digest.
pub fn assert_projection_noninterference<R: GameRules>(
    case: &str,
    state_a: &R::State,
    state_b: &R::State,
    viewer: Viewer,
) {
    let bytes_a = canonical_encode(&R::project(state_a, viewer))
        .expect("a View must be canonically encodable");
    let bytes_b = canonical_encode(&R::project(state_b, viewer))
        .expect("a View must be canonically encodable");

    assert!(
        bytes_a == bytes_b,
        "projection noninterference violated [{case}]: viewer {viewer:?} received two \
         different projections from a pair of states that were constructed to differ only \
         in information this viewer is not authorized to see. If that is wrong — the states \
         also differ in something {viewer:?} legitimately may observe — fix the test's state \
         pair, not this assertion.\n\
         \n  projection of state_a: {}\n  projection of state_b: {}",
        digest(&bytes_a),
        digest(&bytes_b)
    );
}

/// The positive control for [`assert_projection_noninterference`]: assert that
/// `viewer` **can** distinguish `state_a` from `state_b`.
///
/// A noninterference oracle that always passes is worthless — it would pass
/// just as happily against a `project` that returns a constant `View`, or
/// against a state pair that (by a test-authoring mistake) does not actually
/// differ. This function exists so every use of
/// [`assert_projection_noninterference`] can be paired with at least one case
/// proving the same harness *can* observe a difference when one is supposed
/// to be visible — the sanity/control property named P3 in this PR's
/// verification ledger.
///
/// Like its sibling, this has no `R::View: Debug` bound: the oracle is the
/// canonical-byte comparison, and the failure message reports length and
/// digest rather than a structural dump.
///
/// # Panics
/// If the two projections encode to identical canonical bytes.
pub fn assert_projection_differs<R: GameRules>(
    case: &str,
    state_a: &R::State,
    state_b: &R::State,
    viewer: Viewer,
) {
    let bytes_a = canonical_encode(&R::project(state_a, viewer))
        .expect("a View must be canonically encodable");
    let bytes_b = canonical_encode(&R::project(state_b, viewer))
        .expect("a View must be canonically encodable");

    assert!(
        bytes_a != bytes_b,
        "positive control failed [{case}]: viewer {viewer:?} was expected to observe a \
         difference between state_a and state_b, but projected the same canonical bytes from \
         both. Either the state pair does not actually differ in anything {viewer:?} can see, \
         or `project` is ignoring information it should expose — both are bugs the control is \
         designed to catch.\n\
         \n  projection of state_a: {}\n  projection of state_b: {}",
        digest(&bytes_a),
        digest(&bytes_b)
    );
}
