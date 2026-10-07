//! The information model, in code: what Tiles keeps secret and from whom.
//! (doc 02 §7.3, `docs/games/tiles.md`)
//!
//! # Why this lives in the library rather than in `tests/`
//!
//! [`SecretModel`] is a foreign trait and [`TilesRules`] would be a foreign
//! type to an integration-test crate, so the impl cannot live there. It is
//! compiled for `cfg(test)` and behind the `testkit` feature — never in a
//! server or client build, where nothing consumes it.
//!
//! # Token granularity, and its honest limit
//!
//! `Secret::tokens` carried a `TODO(phase 3)` asking the first real
//! hidden-information game to decide what a token *is*. For an **ordered**
//! secret the answer is not "one token per hidden value": a single remaining
//! tile encodes to about two bytes, and two bytes appear in a `View` full of
//! tile kinds and coordinates by coincidence, constantly. A containment scan
//! built on tokens that short reports leaks that are not leaks, and a scan
//! nobody believes is worse than no scan.
//!
//! So Tiles declares two tokens, both **sequences**, and only while the bag is
//! long enough for a sequence to be specific:
//!
//! ```text
//! token 0   the entire remaining bag order
//! token 1   the next few draws, in draw order
//! ```
//!
//! and states the resulting gap plainly: **containment proves nothing about the
//! last few tiles.** That gap is closed by the noninterference property in this
//! module's tests, which permutes the bag and requires every unauthorized
//! projection to be byte-identical — a property that holds for a bag of one as
//! readily as for a bag of seventy, and that also catches a *derived* leak
//! (a checksum, an ordering, a count that moved) which no containment scan can
//! see.
//!
//! @ai.role security-boundary
//! @ai.domain tiles.rules.secret
//! @ai.invariant remaining-bag-order-is-authorised-to-nobody
//! @ai.law unauthorized-projection-does-not-depend-on-bag-order
//! @ai.evidence tests::permuting_the_bag_changes_no_projection_for_any_viewer
//! @ai.evidence tests::permuting_the_bag_changes_no_view_event_for_any_viewer
//! @ai.evidence tests::tabula_hidden_information_security_suite

#![allow(clippy::doc_markdown)]

use tabula_core::canonical_encode;
use tabula_testkit::projection::{Secret, SecretModel};

use super::{State, TileKind, TilesRules};

/// Below this many remaining tiles a sequence token stops being specific
/// enough to be a leak detector. See the module docs: the noninterference
/// property, not a shorter token, is what covers the tail of the bag.
pub(crate) const MIN_TOKENIZABLE_BAG: usize = 4;

/// How many upcoming draws the second token covers. Long enough to be a
/// specific byte string, short enough to catch a "peek at the next few" leak
/// that the whole-order token would miss.
pub(crate) const NEXT_DRAW_WINDOW: usize = 4;

/// The tiles that would be drawn next, in draw order.
///
/// Draws `pop` from the back of the bag, so the next draw is the *last*
/// element. Getting this backwards would produce a token that never matches
/// anything and a scan that always passes.
fn next_draws(state: &State, window: usize) -> Vec<TileKind> {
    state.bag.iter().rev().take(window).copied().collect()
}

impl SecretModel for TilesRules {
    fn secrets(state: &State) -> Vec<Secret> {
        if state.bag.len() < MIN_TOKENIZABLE_BAG {
            return Vec::new();
        }
        let mut tokens = Vec::new();
        if let Ok(whole) = canonical_encode(&state.bag) {
            tokens.push(whole);
        }
        if let Ok(window) = canonical_encode(&next_draws(state, NEXT_DRAW_WINDOW)) {
            tokens.push(window);
        }
        if tokens.is_empty() {
            return Vec::new();
        }
        // `nobody`: not the seat on turn, not a spectator, not a future seat.
        // Only the count and the tile already drawn are public.
        vec![Secret::nobody("remaining tile-bag order", tokens)]
    }

    // `event_secrets` stays at its default. No canonical event carries bag
    // order: `TileDrawn` and `TileDiscarded` each name one tile that has
    // *left* the bag and become public by that very act, and no event names a
    // tile still in it.
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{
        canonical_decode, DetRng, InputIndex, LogicalTime, MatchSeed, Occupant, SeatEntry, SeatId,
        SeatRoster, SpectatorTier, UserId, Viewer,
    };
    use tabula_game_api::{Budget, Ctx, GameModule, GameRules, Input};
    use tabula_testkit::conformance::security::HiddenInformationFixture;
    use tabula_testkit::projection::{
        assert_projection_differs, assert_projection_noninterference, assert_view_event_differs,
        assert_view_event_noninterference,
    };
    use tabula_testkit::GameTestFixture;

    use crate::rules::{first_legal_placement, Command, Config, Event, Status, TurnPhase};
    use crate::TilesModule;

    const SEED: [u8; 32] = [11u8; 32];

    fn roster(count: u8) -> SeatRoster {
        SeatRoster::new(
            (0..count)
                .map(|index| SeatEntry {
                    seat: SeatId(index),
                    occupant: Occupant::Human(UserId(u128::from(index) + 1)),
                    team: None,
                })
                .collect(),
        )
        .expect("fixture seats are unique")
    }

    fn config() -> Config {
        Config {
            turn_deadline_ms: 0,
        }
    }

    /// Drive a real match by always taking the first legal placement, and
    /// return both the reached state and the script that reached it.
    ///
    /// The script is what a `GameTestFixture` needs; the state is what the
    /// noninterference properties need. Building both from one driver keeps
    /// them talking about the same trace.
    fn drive(seed: &MatchSeed, seats: u8, turns: usize) -> (State, Vec<Input<Command>>) {
        let mut rng = DetRng::for_input(seed, InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let init = TilesRules::create(&config(), &roster(seats), &mut ctx)
            .expect("the fixture roster and config are valid");
        let mut state = init.state;
        let mut script = Vec::new();

        for step in 0..turns {
            if state.status() != Status::Playing {
                break;
            }
            // The same "first legal placement, then claim greedily" driver the
            // integration tests use. Claiming matters here: a trace with no
            // followers on the board would still exercise the bag secret, but
            // it would not exercise a `View` that carries follower positions.
            let seat = state.turn();
            let command = match state.phase() {
                TurnPhase::PlaceTile => {
                    let Some(kind) = state.drawn() else { break };
                    let Some((at, rotation)) = first_legal_placement(state.board(), kind) else {
                        break;
                    };
                    Command::PlaceTile { at, rotation }
                }
                TurnPhase::PlaceMeeple => match state.claimable_segments().first() {
                    Some(segment) => Command::PlaceMeeple { segment: *segment },
                    None => Command::SkipMeeple,
                },
            };
            let input = Input::Player { seat, command };
            let index = InputIndex(step as u64 + 1);
            let mut rng = DetRng::for_input(seed, index);
            let mut ctx = Ctx {
                now: LogicalTime::ZERO,
                index,
                rng: &mut rng,
                budget: Budget::default(),
            };
            TilesRules::apply(&mut state, input.clone(), &mut ctx)
                .expect("a driven input is legal");
            script.push(input);
        }
        (state, script)
    }

    struct TilesSecurityFixture;

    impl GameTestFixture for TilesSecurityFixture {
        type Module = TilesModule;

        fn config() -> Config {
            config()
        }

        fn roster() -> SeatRoster {
            roster(3)
        }

        fn seed() -> MatchSeed {
            MatchSeed::from_bytes(SEED)
        }

        fn deterministic_script() -> Vec<Input<Command>> {
            drive(&MatchSeed::from_bytes(SEED), 3, 12).1
        }
    }

    impl HiddenInformationFixture for TilesSecurityFixture {}

    // The mandatory hidden-information suite: every reachable step of a real
    // trace, both containment oracles, every seat and the live spectator.
    tabula_testkit::projection_security!(TilesSecurityFixture);

    fn viewers() -> Vec<Viewer> {
        vec![
            Viewer::Seat(SeatId(0)),
            Viewer::Seat(SeatId(1)),
            Viewer::Seat(SeatId(2)),
            Viewer::Spectator(SpectatorTier::Live),
        ]
    }

    /// Reverse the remaining bag. A permutation preserves the multiset, so the
    /// result is still a well-formed state — which the assertion below checks,
    /// so the property is over a *reachable-shaped* state and not a nonsense
    /// one.
    fn with_reversed_bag(state: &State) -> State {
        let mut scrambled = state.clone();
        scrambled.bag.reverse();
        assert_eq!(
            scrambled.check_invariants(),
            Ok(()),
            "a permutation of the bag must still be a legal state, or this \
             property is testing nonsense"
        );
        scrambled
    }

    /// Rotate the remaining bag by one. A second, differently-shaped
    /// permutation: a `View` that leaked only the *next* tile would survive a
    /// reversal when the bag happens to be near-palindromic, and would not
    /// survive this.
    fn with_rotated_bag(state: &State) -> State {
        let mut scrambled = state.clone();
        if let Some(top) = scrambled.bag.pop() {
            scrambled.bag.insert(0, top);
        }
        assert_eq!(scrambled.check_invariants(), Ok(()));
        scrambled
    }

    /// **The oracle the containment scan cannot be.** Changing only data no
    /// viewer is authorized to see must change no viewer's projection, byte
    /// for byte — including a length, a count, or anything derived.
    #[test]
    fn permuting_the_bag_changes_no_projection_for_any_viewer() {
        let (state, _) = drive(&MatchSeed::from_bytes(SEED), 3, 12);
        assert!(
            state.bag_remaining() > 2,
            "the trace must leave a bag worth permuting"
        );

        for scrambled in [with_reversed_bag(&state), with_rotated_bag(&state)] {
            assert_ne!(
                state.bag, scrambled.bag,
                "the scramble must actually change the secret, or the property is vacuous"
            );
            for viewer in viewers() {
                assert_projection_noninterference::<TilesRules>(
                    "tiles bag-order noninterference",
                    &state,
                    &scrambled,
                    viewer,
                );
            }
        }
    }

    /// The event-shaped sibling: the same canonical event, produced from two
    /// states differing only in bag order, must reach every viewer identically.
    #[test]
    fn permuting_the_bag_changes_no_view_event_for_any_viewer() {
        let (state, _) = drive(&MatchSeed::from_bytes(SEED), 3, 12);
        let scrambled = with_reversed_bag(&state);
        let event = Event::TileDrawn {
            kind: state
                .drawn()
                .or_else(|| state.board().get(state.last_placed()?).map(|t| t.kind))
                .expect("the trace has drawn or placed at least one tile"),
        };

        for viewer in viewers() {
            assert_view_event_noninterference::<TilesRules>(
                "tiles bag-order event noninterference",
                &state,
                &event,
                &scrambled,
                &event,
                viewer,
            );
        }
    }

    /// The positive control. Without it, a `project` that returned a constant
    /// would satisfy every assertion above. One more real turn is a *public*
    /// change — a tile appears, the count drops, possibly a follower lands — so
    /// every viewer must be able to tell the two states apart.
    ///
    /// The precondition is stated as "the canonical states actually differ"
    /// rather than "the bag count dropped": whether input *n* is a placement or
    /// a claim depends on the shuffle, and a control whose precondition is
    /// accidentally false is not a control.
    #[test]
    fn a_public_change_is_visible_to_every_viewer() {
        let (before, _) = drive(&MatchSeed::from_bytes(SEED), 3, 4);
        let (after, _) = drive(&MatchSeed::from_bytes(SEED), 3, 5);
        assert_ne!(
            canonical_encode(&before).unwrap(),
            canonical_encode(&after).unwrap(),
            "the two states must really differ, or this control proves nothing"
        );

        for viewer in viewers() {
            assert_projection_differs::<TilesRules>(
                "one more turn is public",
                &before,
                &after,
                viewer,
            );
        }
    }

    /// The event-shaped positive control: two *different* public events must
    /// not collapse to the same `ViewEvent` for anyone.
    #[test]
    fn two_different_public_events_remain_distinguishable_to_every_viewer() {
        let (state, _) = drive(&MatchSeed::from_bytes(SEED), 3, 6);
        let drawn = state
            .drawn()
            .or_else(|| state.board().get(state.last_placed()?).map(|t| t.kind))
            .expect("the trace has drawn or placed at least one tile");
        let other = crate::rules::TileKind::all()
            .find(|kind| *kind != drawn)
            .expect("the tile set has more than one kind");

        for viewer in viewers() {
            assert_view_event_differs::<TilesRules>(
                "distinct public draws stay distinct",
                &state,
                &Event::TileDrawn { kind: drawn },
                &state,
                &Event::TileDrawn { kind: other },
                viewer,
            );
        }
    }

    /// The declared secret must actually be the bag order, and its tokens must
    /// be the sequences this module's docs promise — not, say, an accidental
    /// empty list that would make the containment scan vacuous.
    #[test]
    fn the_declared_secret_is_the_bag_order_and_is_authorised_to_nobody() {
        let (state, _) = drive(&MatchSeed::from_bytes(SEED), 3, 6);
        let secrets = TilesRules::secrets(&state);
        assert_eq!(secrets.len(), 1);
        let secret = &secrets[0];
        assert!(secret.authorized.is_empty(), "the bag order is nobody's");
        assert_eq!(secret.tokens.len(), 2);
        assert!(secret.tokens.iter().all(|token| token.len() >= 5));

        // Token 0 really is the whole order, and token 1 really is the next
        // draws in draw order — a token that decoded to something else would
        // scan for a byte string that never appears.
        assert_eq!(
            canonical_decode::<Vec<TileKind>>(&secret.tokens[0]).unwrap(),
            state.bag
        );
        let window = canonical_decode::<Vec<TileKind>>(&secret.tokens[1]).unwrap();
        assert_eq!(window.len(), NEXT_DRAW_WINDOW);
        assert_eq!(
            window[0],
            *state.bag.last().unwrap(),
            "the first element of the window must be the very next draw"
        );
    }

    /// The stated gap, asserted rather than merely written down: below the
    /// threshold Tiles declares no token at all. A reader who later "fixes"
    /// this by emitting two-byte tokens will fail here and find the reason.
    #[test]
    fn a_bag_too_short_to_tokenise_declares_no_containment_secret() {
        let (mut state, _) = drive(&MatchSeed::from_bytes(SEED), 3, 6);
        state.bag.truncate(MIN_TOKENIZABLE_BAG - 1);
        assert!(TilesRules::secrets(&state).is_empty());

        // And the noninterference oracle still covers it, which is the whole
        // reason the gap is acceptable.
        let scrambled = {
            let mut scrambled = state.clone();
            scrambled.bag.reverse();
            scrambled
        };
        for viewer in viewers() {
            assert_projection_noninterference::<TilesRules>(
                "short-bag noninterference",
                &state,
                &scrambled,
                viewer,
            );
        }
    }

    /// A negative control for the whole apparatus: a projection that really
    /// did carry the secret must be *caught* by the declared tokens, and the
    /// real one must not be. Written as a direct byte comparison rather than by
    /// installing a broken `project`, because the real `project` is the thing
    /// under test everywhere else.
    ///
    /// Each token is checked against the leak it exists to detect: token 0
    /// against a projection carrying the whole order, token 1 against one
    /// carrying only a peek at the next few draws — the leak the whole-order
    /// token would sail past.
    #[test]
    fn a_projection_that_carried_the_bag_would_be_caught_by_the_declared_tokens() {
        let (state, _) = drive(&MatchSeed::from_bytes(SEED), 3, 6);
        let secrets = TilesRules::secrets(&state);
        let honest = canonical_encode(&TilesRules::project(&state, Viewer::Seat(SeatId(0))))
            .expect("a View encodes");
        let view = TilesRules::project(&state, Viewer::Seat(SeatId(0)));
        let leaks_whole_order =
            canonical_encode(&(&view, &state.bag)).expect("the leaky shape encodes");
        let leaks_next_draws = canonical_encode(&(&view, next_draws(&state, NEXT_DRAW_WINDOW)))
            .expect("the peeking shape encodes");

        let contains = |haystack: &[u8], needle: &[u8]| {
            haystack
                .windows(needle.len())
                .any(|window| window == needle)
        };

        assert!(
            contains(&leaks_whole_order, &secrets[0].tokens[0]),
            "the whole-order token must match a projection carrying the whole order"
        );
        assert!(
            contains(&leaks_next_draws, &secrets[0].tokens[1]),
            "the next-draws token must match a projection peeking at the next draws — \
             this is the leak the whole-order token alone would miss"
        );
        for token in &secrets[0].tokens {
            assert!(
                !contains(&honest, token),
                "and neither token may match the real projection"
            );
        }
    }

    /// `secrets` is called by the scanner on every reachable step; it must not
    /// panic or allocate an empty token at any bag length, including zero.
    #[test]
    fn secrets_is_total_across_every_bag_length() {
        let (mut state, _) = drive(&MatchSeed::from_bytes(SEED), 3, 6);
        while !state.bag.is_empty() {
            for secret in TilesRules::secrets(&state) {
                assert!(secret.tokens.iter().all(|token| !token.is_empty()));
            }
            state.bag.pop();
        }
        assert!(TilesRules::secrets(&state).is_empty());
    }

    #[test]
    fn the_module_declares_the_hidden_information_it_actually_has() {
        assert!(
            TilesModule::capabilities().hidden_information(),
            "a SecretModel on a module that disclaims hidden information proves nothing"
        );
    }
}
