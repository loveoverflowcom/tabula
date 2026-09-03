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
//! what is secret and to whom. [`assert_no_leaks`] and
//! [`assert_no_event_bypasses_redaction`] then scan, over a **reachable**
//! trace (see `crate::determinism::run_typed_trace`) and an explicit,
//! trusted list of real client [`Viewer`]s the caller supplies:
//!
//! ```text
//! for every secret S, for every viewer V not authorised for S:
//!     assert!( !canonical_encode(project(state, V)).contains_any_token_of(S) )
//!     for every canonical event E:
//!         if let Some(ve) = view_event(state_after, E, V):
//!             assert!( !canonical_encode(ve).contains_any_token_of(S) )
//! ```
//!
//! Token-level containment scanning is **coarse but catches the real bugs**: a
//! whole card list leaking, a role map serialised wholesale. Those are the
//! failures that actually ship. This module cannot itself decide *which*
//! viewers are "real" for a game — that requires roster and capability facts
//! it does not have (see [`crate::conformance::security`], which derives the
//! viewer universe and drives the reachable-trace loop for hidden-information
//! games).
//!
//! # What it deliberately does not catch
//!
//! *Derived* secrets — where two public values combine to reveal a hidden one
//! (exact deck count + all discards + your hand = the opponent's hand in a
//! 52-card game), or where a value is computed *from* a secret without
//! copying it verbatim (a checksum, a length that happens to leak more than a
//! legitimately public count). Byte containment cannot find either. That is
//! exactly the gap [`assert_projection_noninterference`] and
//! [`assert_view_event_noninterference`] exist to close (see below) — and
//! beyond both oracles, games with hidden information must still write an
//! **information model** in `docs/games/<slug>.md` listing what is
//! intentionally derivable, for a human to review. (doc 02 §7.1)
//!
//! # Spectators are not "player 0"
//!
//! The most common projection bug in board-game platforms is spectators seeing
//! hidden hands. The scan checks `Viewer::Spectator(Live)` and
//! `Viewer::Spectator(Delayed)` explicitly, as separate viewers, whenever the
//! caller's viewer list includes them.
//!
//! # `Viewer::Audit` is not a client
//!
//! `Viewer::Audit` is privileged support/replay/audit tooling and is
//! **never reachable from a game client session** (doc 00 §9.4). It
//! legitimately sees canonical information, so it must never appear in the
//! viewer list passed to [`assert_no_leaks`] or
//! [`assert_no_event_bypasses_redaction`] — doing so would fail the scan on
//! the documented exception instead of a real leak. `Viewer::Audit` remains a
//! valid *positive*-control viewer for [`assert_projection_differs`] /
//! [`assert_view_event_differs`], which assert the opposite: that authorized
//! information really is observable.
//!
//! # Noninterference — the stronger, complementary property
//!
//! [`assert_no_leaks`] and [`assert_no_event_bypasses_redaction`] are
//! **containment** scans: they ask whether a secret's own bytes appear in an
//! unauthorized `View`/`ViewEvent`. That misses a *derived* leak — a length, a
//! count, or a checksum that changes only because hidden data changed,
//! without ever copying that data verbatim (§8.3 of the `develop`
//! architecture-verification audit calls these "Class 3" leaks and notes no
//! containment scanner can find them).
//!
//! [`assert_projection_noninterference`] / [`assert_projection_differs`] and
//! their event-shaped siblings [`assert_view_event_noninterference`] /
//! [`assert_view_event_differs`] ask the stronger question directly: does an
//! unauthorized viewer's `View` or `Option<ViewEvent>` depend on hidden data
//! **at all**? The caller proves *by construction* that two canonical states
//! (and, for the event pair, the events produced from them) differ only in
//! data a given [`Viewer`] is not authorized to see (this module has no way to
//! discover that on its own — see the module docs on [`SecretModel`] for why a
//! generic scanner cannot infer secrecy from a struct), and the assertion
//! checks that the viewer's canonically-encoded output is byte-identical (or,
//! for the positive control, that it differs) between the two states.
//!
//! This is a property over **pairs of reachable states**, not a replacement
//! for the containment scans: a wholesale leak and a derived leak are
//! different failure modes and this module keeps both oracles rather than
//! merging them. Containment and noninterference also detect different
//! things by construction — neither is "stronger" in general; §15 of this
//! PR's own instructions and the negative controls in
//! `tests/projection_noninterference.rs` demonstrate a derived leak that
//! containment misses and noninterference catches.

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

/// A malformed [`Secret`] token, and where it was found.
///
/// A zero-length token is a needle every haystack "contains" — scanning for
/// it would either report a leak on every single viewer (useless noise) or,
/// worse, be implemented in a way that treats "found at position 0 of
/// nothing" as "not found" and silently passes. Neither is the scanner
/// working; both are a malformed [`SecretModel`], not a real result, so this
/// is checked and reported *before* any containment comparison runs (item 13
/// of this PR's own instructions).
fn validate_secret(case: &str, secret: &Secret) {
    for (token_index, token) in secret.tokens.iter().enumerate() {
        assert!(
            !token.is_empty(),
            "malformed SecretModel [{case}]: secret \"{}\" declares empty token #{token_index}. \
             An empty token matches every byte string, which would make this scan meaningless \
             rather than strict. Fix the `SecretModel::secrets` implementation to supply only \
             non-empty tokens.",
            secret.label
        );
    }
}

/// The first token of `secret` that appears verbatim inside `haystack`, if any.
///
/// Returns the token's index in [`Secret::tokens`] for the caller's failure
/// message — never the token bytes themselves (doc: no raw secret material in
/// diagnostics, item 14).
fn find_leaked_token(haystack: &[u8], secret: &Secret) -> Option<usize> {
    secret.tokens.iter().position(|token| {
        haystack
            .windows(token.len())
            .any(|window| window == token.as_slice())
    })
}

/// Containment scan over `project()`: **Oracle A** (doc 00 §4.2, doc 02 §7.3,
/// `projection_hides_secrets`).
///
/// For every [`Secret`] the game declares over `state`, and for every
/// `viewer` in the caller-supplied list not in that secret's
/// [`Secret::authorized`] list, asserts the secret's tokens do not appear
/// verbatim in `canonical_encode(project(state, viewer))`.
///
/// `viewers` must be the complete set of **real client** viewers for this
/// game — every roster seat, plus whatever the game's `SpectatorPolicy`
/// allows — and must never include [`Viewer::Audit`] (see the module docs).
/// This function has no way to construct or validate that list itself: doing
/// so from `state` alone would silently miss any seat a `SecretModel` did not
/// happen to mention (this PR's item 4) — see
/// `crate::conformance::security::client_viewer_universe` for the function
/// that derives it correctly, from roster and capability facts instead.
///
/// # Panics
/// - If any [`Secret`] declares an empty token (a malformed declaration, not
///   a leak — see [`validate_secret`]).
/// - If a secret's tokens appear in an unauthorized viewer's `View`. The
///   message names `case`, the secret's label, the viewer, and the encoded
///   output's length and digest — never the raw secret bytes or the full
///   encoded `View` (item 14).
pub fn assert_no_leaks<R: SecretModel>(case: &str, state: &R::State, viewers: &[Viewer]) {
    let secrets = R::secrets(state);
    for secret in &secrets {
        validate_secret(case, secret);
    }

    for secret in &secrets {
        for &viewer in viewers {
            assert!(
                !matches!(viewer, Viewer::Audit),
                "assert_no_leaks [{case}] was given Viewer::Audit in its client viewer list; \
                 Audit legitimately sees canonical information and must never be treated as an \
                 unauthorized client (see the module docs)."
            );
            if secret.authorized.contains(&viewer) {
                continue;
            }

            let bytes = canonical_encode(&R::project(state, viewer))
                .expect("a View must be canonically encodable");
            if let Some(token_index) = find_leaked_token(&bytes, secret) {
                panic!(
                    "projection secrecy violated [{case}]: secret \"{label}\" (token #{token_index}) \
                     appeared verbatim in {viewer:?}'s View — surface: project.\n  \
                     encoded projection: {}",
                    digest(&bytes),
                    label = secret.label,
                );
            }
        }
    }
}

/// Containment scan over `view_event()`: **Oracle A** for events (doc 00
/// §4.2, I-6, `view_event_never_bypasses`).
///
/// For every canonical `event` in `events` (a reachable trace step's
/// emissions — see `crate::determinism::run_typed_trace`) and every `viewer`
/// in the caller-supplied list, calls `R::view_event(state_after, event,
/// viewer)`. Every `Some(view_event)` result is then checked, per
/// [`Secret`] declared over `state_after`, exactly like [`assert_no_leaks`]:
/// an unauthorized viewer's redacted event must not contain that secret's
/// tokens.
///
/// The property this proves is not "the function can be called" — a call
/// that always returns `Some`/`None` regardless of input would pass a naive
/// smoke test and prove nothing (this PR's item 9). It is: **every canonical
/// event this reachable trace actually produced was routed through
/// `view_event` for every real client viewer, and none of the results leak.**
/// Calling this once per reachable-trace step, for the step's own emitted
/// events, is what makes "every event" true across a whole match rather than
/// one hand-picked example — see
/// `crate::conformance::security::check` for the loop that does that.
///
/// See `assert_view_event_noninterference` for the complementary *derived*
/// leak this containment scan cannot see — a value computed from an event's
/// hidden fields (a checksum, e.g.) without ever copying them verbatim.
///
/// # Panics
/// - If any [`Secret`] declares an empty token.
/// - If a secret's tokens appear in an unauthorized viewer's `ViewEvent`. The
///   message names `case`, the event's index, the secret's label, the
///   viewer, and the encoded output's length and digest — never raw secret
///   bytes or the full encoded event.
pub fn assert_no_event_bypasses_redaction<R: SecretModel>(
    case: &str,
    state_after: &R::State,
    events: &[R::Event],
    viewers: &[Viewer],
) {
    let secrets = R::secrets(state_after);
    for secret in &secrets {
        validate_secret(case, secret);
    }

    for (event_index, event) in events.iter().enumerate() {
        for &viewer in viewers {
            assert!(
                !matches!(viewer, Viewer::Audit),
                "assert_no_event_bypasses_redaction [{case}] was given Viewer::Audit in its \
                 client viewer list; Audit legitimately sees canonical information and must \
                 never be treated as an unauthorized client (see the module docs)."
            );
            let Some(view_event) = R::view_event(state_after, event, viewer) else {
                continue;
            };
            let bytes =
                canonical_encode(&view_event).expect("a ViewEvent must be canonically encodable");

            for secret in &secrets {
                if secret.authorized.contains(&viewer) {
                    continue;
                }
                if let Some(token_index) = find_leaked_token(&bytes, secret) {
                    panic!(
                        "event secrecy violated [{case}]: secret \"{label}\" (token \
                         #{token_index}) appeared verbatim in {viewer:?}'s ViewEvent for event \
                         #{event_index} — surface: view_event.\n  encoded view_event: {}",
                        digest(&bytes),
                        label = secret.label,
                    );
                }
            }
        }
    }
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

/// The event-shaped sibling of [`assert_projection_noninterference`]: assert
/// that `viewer` cannot distinguish `(state_after_a, event_a)` from
/// `(state_after_b, event_b)` through `view_event`.
///
/// Encodes `Option<R::ViewEvent>` — not just the `Some` payload — so this
/// catches both shapes of derived event leak in one oracle:
///
/// - `Some(redacted_a) != Some(redacted_b)`: the redacted content itself
///   depends on hidden data (a checksum, a degraded-but-still-correlated
///   field).
/// - `None` on one side, `Some(..)` on the other: the mere **existence** of
///   the event, as this viewer would observe it, depends on hidden data —
///   arguably the more dangerous case, since nothing about "no event
///   happened" looks like a leak in isolation.
///
/// As with [`assert_projection_noninterference`], the caller proves *by
/// construction* that the two `(state, event)` pairs differ only in
/// information `viewer` is not authorized to see — normally by producing
/// both from the same reachable script via
/// `crate::determinism::run_typed_trace`, varying only a hidden input. This
/// function trusts that construction completely; see the module docs for why
/// no generic scanner can discover it another way.
///
/// No `R::ViewEvent: Debug` bound, for the same reason
/// [`assert_projection_noninterference`] has none: the oracle is the
/// canonical-byte comparison of the encoded `Option`, and the failure message
/// reports length and digest, never a structural dump or the raw event.
///
/// # Panics
/// If the two `Option<ViewEvent>` values encode to different canonical bytes.
pub fn assert_view_event_noninterference<R: GameRules>(
    case: &str,
    state_after_a: &R::State,
    event_a: &R::Event,
    state_after_b: &R::State,
    event_b: &R::Event,
    viewer: Viewer,
) {
    let bytes_a = canonical_encode(&R::view_event(state_after_a, event_a, viewer))
        .expect("an Option<ViewEvent> must be canonically encodable");
    let bytes_b = canonical_encode(&R::view_event(state_after_b, event_b, viewer))
        .expect("an Option<ViewEvent> must be canonically encodable");

    assert!(
        bytes_a == bytes_b,
        "event noninterference violated [{case}]: viewer {viewer:?} received two different \
         Option<ViewEvent> results (a redacted event differed, or one side saw an event exist \
         and the other did not) from a pair of (state, event) constructed to differ only in \
         information this viewer is not authorized to see. If that is wrong — the pair also \
         differs in something {viewer:?} legitimately may observe — fix the test's pair, not \
         this assertion.\n\
         \n  view_event of pair a: {}\n  view_event of pair b: {}",
        digest(&bytes_a),
        digest(&bytes_b)
    );
}

/// The positive control for [`assert_view_event_noninterference`]: assert
/// that `viewer` **can** distinguish the two `(state, event)` pairs.
///
/// Same rationale as [`assert_projection_differs`]: a noninterference oracle
/// that always passes is worthless, so every use of
/// [`assert_view_event_noninterference`] should be paired with at least one
/// case proving the harness can see a real, authorized difference — e.g. the
/// event's own owner learning that its authorized detail changed, or
/// `Viewer::Audit` observing a scrambled hidden field it is entitled to see
/// in full (doc 00 §9.4).
///
/// # Panics
/// If the two `Option<ViewEvent>` values encode to identical canonical bytes.
pub fn assert_view_event_differs<R: GameRules>(
    case: &str,
    state_after_a: &R::State,
    event_a: &R::Event,
    state_after_b: &R::State,
    event_b: &R::Event,
    viewer: Viewer,
) {
    let bytes_a = canonical_encode(&R::view_event(state_after_a, event_a, viewer))
        .expect("an Option<ViewEvent> must be canonically encodable");
    let bytes_b = canonical_encode(&R::view_event(state_after_b, event_b, viewer))
        .expect("an Option<ViewEvent> must be canonically encodable");

    assert!(
        bytes_a != bytes_b,
        "positive control failed [{case}]: viewer {viewer:?} was expected to observe a \
         difference between the two (state, event) pairs, but view_event produced identical \
         canonical bytes for both. Either the pair does not actually differ in anything \
         {viewer:?} can see, or `view_event` is ignoring information it should expose — both \
         are bugs the control is designed to catch.\n\
         \n  view_event of pair a: {}\n  view_event of pair b: {}",
        digest(&bytes_a),
        digest(&bytes_b)
    );
}
