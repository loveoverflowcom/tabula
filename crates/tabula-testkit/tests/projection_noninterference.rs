//! Proves [`tabula_testkit::projection::assert_projection_noninterference`]
//! (and its positive-control sibling `assert_projection_differs`) against a
//! tiny, test-only reference model with genuine hidden information.
//!
//! # Why a reference model instead of a real game
//!
//! At the time of this PR, `games/cards` and `games/werewolf` are doc-comment
//! sketches with zero `impl GameRules` (Phase 3 has not started; see
//! `docs/architecture/07-phases-and-implementation-roadmap.md`). Chess and
//! tic-tac-toe are perfect-information games and cannot exercise a secrecy
//! property honestly — see `games/*/tests/projection_control.rs` for what
//! *can* be said about them (projection determinism and a public-difference
//! positive control, not secrecy).
//!
//! So this file defines the smallest game that has real hidden information:
//! a round counter (public) plus one hand of opaque "cards" per seat
//! (secret, visible only to the owning seat — and, in full, to
//! `Viewer::Audit`, matching doc 00 §9.4's stated semantics for that
//! viewer). It exists ONLY to exercise the harness; it is not a step toward
//! implementing `games/cards`.
//!
//! `tabula-testkit` itself stays generic: this model lives in a test binary,
//! not in `src/`, and nothing in `src/projection.rs` knows this type exists.
//!
//! # Verification ledger for this file
//!
//! ```text
//! P1  projection is deterministic for fixed state/viewer
//! P2  hidden-state noninterference (the primary proposition)
//! P3  authorized/public differences remain observable (the sanity control)
//! ```

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use tabula_core::{Millis, RulesVersion, SeatId, SeatRoster, SpectatorTier, Viewer};
use tabula_game_api::{Ctx, GameRules, Init, InitError, Input, Outcome, RuleError};
use tabula_testkit::projection::{assert_projection_differs, assert_projection_noninterference};

// ---------------------------------------------------------------------------
// The reference model: a public round counter plus one hidden hand per seat.
// ---------------------------------------------------------------------------

/// Canonical state. `hands` is the only secret: each seat's own hand is
/// authorized to that seat and to `Viewer::Audit`; nobody else may see it.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct State {
    public_round: u32,
    hands: BTreeMap<SeatId, Vec<u8>>,
}

impl State {
    /// Proof-by-construction: every state built through this constructor has
    /// at least one seat and a hand entry for each. There is no path to a
    /// `State` with a dangling or missing hand, which is what makes the
    /// noninterference pairs below trustworthy — they are not filling in a
    /// blanked field, they are two different, equally valid worlds.
    fn new(public_round: u32, hands: BTreeMap<SeatId, Vec<u8>>) -> Self {
        assert!(
            !hands.is_empty(),
            "reference model: a match needs at least one seat"
        );
        Self {
            public_round,
            hands,
        }
    }
}

fn hands(entries: &[(u8, &[u8])]) -> BTreeMap<SeatId, Vec<u8>> {
    entries
        .iter()
        .map(|(seat, cards)| (SeatId(*seat), cards.to_vec()))
        .collect()
}

fn initial_state(roster: &SeatRoster) -> State {
    let hands = roster.iter().map(|e| (e.seat, Vec::new())).collect();
    State::new(0, hands)
}

/// Per-viewer redacted state, honestly projected.
///
/// * A seated viewer sees only its own hand — never `Option<Vec<u8>>` set to
///   `None` for *other* hands and filled in for its own by coincidence; the
///   field literally cannot exist for the wrong seat because it is looked up
///   by `viewer`'s own seat, not stored per-seat in the view.
/// * A spectator sees hand *counts* only (public: how many cards, not which).
/// * `Viewer::Audit` sees every hand, matching doc 00 §9.4's "sees canonical
///   information" — this is a deliberate, documented exception, not a leak,
///   and the tests below treat scrambling as *expected* to change Audit's
///   projection rather than asserting noninterference for it.
#[derive(Clone, Debug, Serialize)]
struct View {
    public_round: u32,
    your_hand: Option<Vec<u8>>,
    hand_counts: BTreeMap<SeatId, usize>,
    all_hands_for_audit: Option<BTreeMap<SeatId, Vec<u8>>>,
}

struct Rules;

impl GameRules for Rules {
    type State = State;
    type Command = ();
    type Event = ();
    type View = View;
    type ViewEvent = ();
    type Config = ();

    const RULES_VERSION: RulesVersion = RulesVersion(1);

    fn create((): &(), roster: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        Ok(Init {
            state: initial_state(roster),
            events: SmallVec::new(),
            effects: SmallVec::new(),
        })
    }

    fn apply(_: &mut State, _: Input<()>, _: &mut Ctx<'_>) -> Result<Outcome<Self>, RuleError> {
        // Unused by these tests: every state below is built directly via
        // `State::new`, not by driving `apply`. Still required by the trait.
        Ok(Outcome::empty())
    }

    fn project(state: &State, viewer: Viewer) -> View {
        let hand_counts = state.hands.iter().map(|(s, h)| (*s, h.len())).collect();
        match viewer {
            Viewer::Seat(seat) => View {
                public_round: state.public_round,
                your_hand: state.hands.get(&seat).cloned(),
                hand_counts,
                all_hands_for_audit: None,
            },
            Viewer::Spectator(_) => View {
                public_round: state.public_round,
                your_hand: None,
                hand_counts,
                all_hands_for_audit: None,
            },
            Viewer::Audit => View {
                public_round: state.public_round,
                your_hand: None,
                hand_counts,
                all_hands_for_audit: Some(state.hands.clone()),
            },
        }
    }

    fn view_event(_: &State, (): &(), _: Viewer) -> Option<()> {
        None
    }
}

// ---------------------------------------------------------------------------
// P1 — projection determinism (necessary but weak on its own)
// ---------------------------------------------------------------------------

#[test]
fn same_state_and_viewer_yield_the_same_projection_every_time() {
    let state = State::new(3, hands(&[(0, &[1, 2]), (1, &[9, 9, 9])]));

    // Passing the same state as both "a" and "b" reduces the noninterference
    // check to plain determinism: project() called twice on one state must
    // agree, for every viewer kind.
    for viewer in [
        Viewer::Seat(SeatId(0)),
        Viewer::Seat(SeatId(1)),
        Viewer::Spectator(SpectatorTier::Live),
        Viewer::Spectator(SpectatorTier::Delayed { by: Millis(30_000) }),
        Viewer::Audit,
    ] {
        assert_projection_noninterference::<Rules>("determinism", &state, &state, viewer);
    }
}

// ---------------------------------------------------------------------------
// P2 — hidden-state noninterference (the primary proposition)
// ---------------------------------------------------------------------------

#[test]
fn opponents_hidden_hand_does_not_affect_a_seated_viewers_projection() {
    // Same public round, same seat 0 hand; only seat 1's hidden hand
    // differs — in *content*, not in count. Hand count is public in this
    // model (`View::hand_counts`), so a pair that also changes the length
    // would be varying something seat 0 legitimately observes, not only
    // hidden information — see the property test below, which generates
    // exactly this partition (fixed length, varied content) systematically.
    let state_a = State::new(5, hands(&[(0, &[1, 2]), (1, &[9, 9])]));
    let state_b = State::new(5, hands(&[(0, &[1, 2]), (1, &[7, 3])]));

    assert_projection_noninterference::<Rules>(
        "seat 1's hand is not seat 0's business",
        &state_a,
        &state_b,
        Viewer::Seat(SeatId(0)),
    );
}

#[test]
fn spectators_see_no_hand_information_regardless_of_hidden_hands() {
    // Same hand *counts* (public) for both states; only card content differs.
    let state_a = State::new(5, hands(&[(0, &[1, 2]), (1, &[9, 9])]));
    let state_b = State::new(5, hands(&[(0, &[4, 4]), (1, &[7, 3])]));

    // Both hands' content differs between the two states, so a spectator
    // authorized for neither must be unable to tell them apart at all —
    // live or delayed.
    for viewer in [
        Viewer::Spectator(SpectatorTier::Live),
        Viewer::Spectator(SpectatorTier::Delayed { by: Millis(30_000) }),
    ] {
        assert_projection_noninterference::<Rules>(
            "spectators are not player 0",
            &state_a,
            &state_b,
            viewer,
        );
    }
}

#[test]
fn each_seat_is_indifferent_to_every_other_seats_hand_in_a_three_seat_match() {
    // Multiple viewers (item 16): three seats, each checked against a pair
    // that varies every *other* seat's hand while holding its own fixed.
    let state_a = State::new(1, hands(&[(0, &[1]), (1, &[2, 2]), (2, &[3, 3, 3])]));
    let state_b = State::new(1, hands(&[(0, &[1]), (1, &[8, 8]), (2, &[5, 5, 5])]));

    assert_projection_noninterference::<Rules>(
        "seat 0 indifferent to seats 1 and 2",
        &state_a,
        &state_b,
        Viewer::Seat(SeatId(0)),
    );

    let state_c = State::new(1, hands(&[(0, &[9]), (1, &[2, 2]), (2, &[3, 3, 3])]));
    let state_d = State::new(1, hands(&[(0, &[6]), (1, &[2, 2]), (2, &[1, 1, 1])]));

    assert_projection_noninterference::<Rules>(
        "seat 1 indifferent to seats 0 and 2",
        &state_c,
        &state_d,
        Viewer::Seat(SeatId(1)),
    );
}

// ---------------------------------------------------------------------------
// P3 — authorized/public differences remain observable (the sanity control)
// ---------------------------------------------------------------------------

#[test]
fn a_seats_own_hand_change_is_observable_to_that_seat() {
    let state_a = State::new(5, hands(&[(0, &[1, 2]), (1, &[9, 9])]));
    let state_b = State::new(5, hands(&[(0, &[3, 4]), (1, &[9, 9])]));

    assert_projection_differs::<Rules>(
        "your own hand changing is visible to you",
        &state_a,
        &state_b,
        Viewer::Seat(SeatId(0)),
    );
}

#[test]
fn a_public_round_change_is_observable_to_every_viewer_kind() {
    let state_a = State::new(1, hands(&[(0, &[1, 2]), (1, &[9, 9])]));
    let state_b = State::new(2, hands(&[(0, &[1, 2]), (1, &[9, 9])]));

    for viewer in [
        Viewer::Seat(SeatId(0)),
        Viewer::Seat(SeatId(1)),
        Viewer::Spectator(SpectatorTier::Live),
        Viewer::Audit,
    ] {
        assert_projection_differs::<Rules>("public round is public", &state_a, &state_b, viewer);
    }
}

#[test]
fn audit_legitimately_sees_every_hand_so_scrambling_is_expected_to_be_visible() {
    // Not a leak: doc 00 §9.4 states Audit "sees canonical information".
    // Scrambling a hidden hand SHOULD change Audit's projection — asserting
    // noninterference here would be asserting the wrong thing.
    let state_a = State::new(5, hands(&[(0, &[1, 2]), (1, &[9, 9])]));
    let state_b = State::new(5, hands(&[(0, &[1, 2]), (1, &[7, 3, 3])]));

    assert_projection_differs::<Rules>(
        "Audit sees hidden hands by design",
        &state_a,
        &state_b,
        Viewer::Audit,
    );
}

// ---------------------------------------------------------------------------
// Property test: scrambling hidden hands preserves an unauthorized viewer's
// projection, over many generated pairs rather than a handful of examples.
// ---------------------------------------------------------------------------

/// A hand is a short vector of opaque byte-valued "cards". Values do not need
/// to be unique or otherwise meaningful — the property is about their mere
/// presence and content, not any card-game rule.
///
/// **Hand *count* is public in this model** (`View::hand_counts`) — a
/// deliberate design choice mirroring the real Tiến Lên sketch in
/// `games/cards/src/lib.rs` (`hand_counts: [u8; 4], // public`). So a
/// "scramble only the secret" generator must hold each seat's hand *length*
/// fixed and vary only its *content* — this is what
/// `rust-property-testing`'s "reachable vs arbitrary" split (and this PR's
/// own §16) means by "invalid states are not generated merely to increase
/// coverage": a pair whose count also happened to differ would be testing a
/// different, false proposition (that count is secret too), not the one this
/// property states.
fn arb_hand_pair() -> impl Strategy<Value = (Vec<u8>, Vec<u8>)> {
    (0..6usize).prop_flat_map(|len| {
        (
            prop::collection::vec(any::<u8>(), len..=len),
            prop::collection::vec(any::<u8>(), len..=len),
        )
    })
}

fn arb_hand() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..6)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// The noninterference law, generated: fix the public round and the
    /// viewer seat's own hand, then draw two *independent same-length*
    /// hands for every other seat. Seat 0's projection must be identical
    /// across the pair regardless of what the other seats' hidden hands
    /// turned out to contain.
    ///
    /// This is strictly stronger than the handful of examples above: it does
    /// not just check that noninterference holds for one chosen pair, it
    /// checks that the pair's choice never mattered in the first place, over
    /// 128 generated pairs per run.
    #[test]
    fn changing_only_other_seats_secret_content_preserves_the_seated_viewers_projection(
        public_round in any::<u32>(),
        own_hand in arb_hand(),
        (other_hand_1_a, other_hand_1_b) in arb_hand_pair(),
        (other_hand_2_a, other_hand_2_b) in arb_hand_pair(),
    ) {
        let state_a = State::new(
            public_round,
            hands(&[
                (0, &own_hand),
                (1, &other_hand_1_a),
                (2, &other_hand_2_a),
            ]),
        );
        let state_b = State::new(
            public_round,
            hands(&[
                (0, &own_hand),
                (1, &other_hand_1_b),
                (2, &other_hand_2_b),
            ]),
        );

        assert_projection_noninterference::<Rules>(
            "property: scrambled other-seat hand content, count held fixed",
            &state_a,
            &state_b,
            Viewer::Seat(SeatId(0)),
        );
    }

    /// The same law from the spectator's position: a spectator is
    /// unauthorized for **every** hand, including seat 0's own — so this
    /// property scrambles all three seats' content at once (each pair
    /// length-matched, per [`arb_hand_pair`]) and checks the spectator's
    /// projection is unaffected.
    #[test]
    fn changing_any_seats_secret_content_preserves_the_spectators_projection(
        public_round in any::<u32>(),
        (hand_0_a, hand_0_b) in arb_hand_pair(),
        (hand_1_a, hand_1_b) in arb_hand_pair(),
        (hand_2_a, hand_2_b) in arb_hand_pair(),
    ) {
        let state_a = State::new(
            public_round,
            hands(&[(0, &hand_0_a), (1, &hand_1_a), (2, &hand_2_a)]),
        );
        let state_b = State::new(
            public_round,
            hands(&[(0, &hand_0_b), (1, &hand_1_b), (2, &hand_2_b)]),
        );

        assert_projection_noninterference::<Rules>(
            "property: spectator is indifferent to every hand's content",
            &state_a,
            &state_b,
            Viewer::Spectator(SpectatorTier::Live),
        );
    }
}

// ---------------------------------------------------------------------------
// Oracle sanity: the noninterference check must be able to FAIL.
//
// A property with no failing mutant is decoration (rust-property-testing
// skill). This deliberately leaky variant proves the assertion actually
// catches a violation, in the same style as
// `conformance_catches_violations.rs`: build a broken game, assert the check
// panics and names what broke.
// ---------------------------------------------------------------------------

mod leaky_projection_is_caught {
    use super::*;

    /// The bug: `spectator_leak_checksum` is a value derived from every
    /// hidden hand's contents and exposed to viewers unconditionally,
    /// including spectators. No hand's bytes are copied verbatim into this
    /// view, so a token-containment scan (`assert_no_leaks`) would not catch
    /// it — this is exactly the "Class 3 — derived leak" the noninterference
    /// property exists to catch and a containment scan structurally cannot.
    #[derive(Clone, Debug, Serialize)]
    struct LeakyView {
        public_round: u32,
        your_hand: Option<Vec<u8>>,
        hand_counts: BTreeMap<SeatId, usize>,
        spectator_leak_checksum: u32,
    }

    struct LeakyRules;

    impl GameRules for LeakyRules {
        type State = State;
        type Command = ();
        type Event = ();
        type View = LeakyView;
        type ViewEvent = ();
        type Config = ();

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        fn create((): &(), roster: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: initial_state(roster),
                events: SmallVec::new(),
                effects: SmallVec::new(),
            })
        }

        fn apply(_: &mut State, _: Input<()>, _: &mut Ctx<'_>) -> Result<Outcome<Self>, RuleError> {
            Ok(Outcome::empty())
        }

        fn project(state: &State, viewer: Viewer) -> LeakyView {
            let hand_counts = state.hands.iter().map(|(s, h)| (*s, h.len())).collect();
            let checksum = state.hands.values().flatten().map(|&b| u32::from(b)).sum();
            LeakyView {
                public_round: state.public_round,
                your_hand: match viewer {
                    Viewer::Seat(seat) => state.hands.get(&seat).cloned(),
                    Viewer::Spectator(_) | Viewer::Audit => None,
                },
                hand_counts,
                // THE BUG: computed from every hand, handed out to every
                // viewer, including ones authorized for none of them.
                spectator_leak_checksum: checksum,
            }
        }

        fn view_event(_: &State, (): &(), _: Viewer) -> Option<()> {
            None
        }
    }

    #[test]
    fn noninterference_check_catches_a_derived_leak_a_containment_scan_would_miss() {
        // Same hand *counts* as the honest `hand_counts` field would report
        // (2 and 2 either way) — the only thing distinguishing this pair is
        // hidden card content, so `LeakyRules`'s honest-looking fields agree
        // and only the derived checksum gives the leak away.
        let state_a = State::new(5, hands(&[(0, &[1, 2]), (1, &[9, 9])]));
        let state_b = State::new(5, hands(&[(0, &[1, 2]), (1, &[7, 3])]));

        let result = catch_unwind(AssertUnwindSafe(|| {
            assert_projection_noninterference::<LeakyRules>(
                "leaky fixture",
                &state_a,
                &state_b,
                Viewer::Spectator(SpectatorTier::Live),
            );
        }));

        let Err(payload) = result else {
            panic!(
                "assert_projection_noninterference ACCEPTED a projection that leaks a \
                 derived value correlated with a hidden hand. The oracle is not enforcing \
                 anything."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("projection noninterference violated"),
            "the check panicked but not with the expected diagnostic; got: {message}"
        );
    }
}
