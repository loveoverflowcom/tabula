//! Proves [`tabula_testkit::projection::assert_projection_noninterference`]
//! (and its positive-control sibling `assert_projection_differs`), the two
//! secret-containment scanners (`assert_no_leaks`,
//! `assert_no_event_bypasses_redaction`), their event-shaped noninterference
//! counterparts (`assert_view_event_noninterference`,
//! `assert_view_event_differs`), and the opt-in
//! `HiddenInformationFixture`/`projection_security!` conformance layer —
//! against a tiny, test-only reference model with genuine hidden
//! information.
//!
//! # Why a reference model instead of a real game
//!
//! At the time of this PR, `games/werewolf` and `games/tiles` are doc-comment
//! sketches with zero `impl GameRules` (Phase 3 has not started; see
//! `docs/architecture/07-phases-and-implementation-roadmap.md`) — they are the
//! reference games in the current portfolio (doc 09 §3, doc 08) with
//! `hidden_information = true`. Chess and Caro are
//! perfect-information games and cannot exercise a secrecy property honestly
//! — see `games/*/tests/projection_control.rs` for what *can* be said about
//! them (projection determinism and a public-difference positive control,
//! not secrecy).
//!
//! So this file defines the smallest game that has real hidden information:
//! a public round counter plus one hand of opaque "cards" per seat, dealt by
//! a legal `Command::Deal`. It exists ONLY to exercise the harness; it is not
//! a step toward implementing any specific game — "cards" here names the
//! shape of the fixture (an opaque per-seat hand), not a game in the
//! reference portfolio.
//!
//! `tabula-testkit` itself stays generic: this model lives in a test binary,
//! not in `src/`, and nothing in `src/projection.rs` knows this type exists.
//!
//! # Every state here is reachable, not merely representable
//!
//! `rust-property-testing`'s reachable-vs-arbitrary split is explicit that a
//! semantic law like noninterference needs states produced by the game's own
//! legal transitions, not a struct literal: "generate reachable states...
//! reachable — produced only by the system's own legal transitions." A
//! secret becomes real in `State` only via a legal `Input::Player` carrying
//! `Command::Deal`, applied through `GameRules::create`/`apply` exactly as
//! `tabula_testkit::determinism::run_typed` (and, in production, the match
//! runtime) would apply it. Nothing in this file constructs a `State` value
//! directly — `dealt_state` is the only constructor, and it is a thin wrapper
//! around `run_typed`.
//!
//! # Verification ledger for this file
//!
//! ```text
//! P1  projection is deterministic for fixed state/viewer
//! P2  hidden-state noninterference (the primary proposition)
//! P3  authorized/public differences remain observable (the sanity control)
//! P4  direct secret containment: View        (assert_no_leaks)
//! P5  direct secret containment: ViewEvent   (assert_no_event_bypasses_redaction)
//! P6  derived-leak noninterference: ViewEvent (assert_view_event_noninterference)
//! P7  the roster/capability-derived client viewer universe cannot be
//!     bypassed by a leak to a seat no Secret ever named
//! ```
//!
//! P1–P3 are exercised directly against `project`/`view_event` with
//! hand-picked viewers, in the style PR #37 established. P4–P7 additionally
//! exercise the fixture-driven pipeline
//! (`tabula_testkit::conformance::security`) that a real hidden-information
//! game crate would use, via the `HiddenInformationFixture` impls below —
//! proving the wiring, not just the primitives, works end to end.

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::LazyLock;

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use tabula_core::{
    GameId, GameVersion, MatchSeed, Millis, Occupant, RuleError, RuleErrorCode, RulesVersion,
    SeatEntry, SeatId, SeatRoster, SpectatorTier, UserId, Viewer,
};
use tabula_game_api::{
    AssetRef, AsyncTurnPolicy, Budget, Category, ChatPolicy, Complexity, ConfigError,
    ContentRating, Ctx, Durability, DurationRange, GameCapabilities, GameCapabilitiesSpec,
    GameMetadata, GameMetadataSpec, GameModule, GameRules, I18nKey, Init, InitError, Input,
    Outcome, RankedSupport, ReconnectPolicy, SeatCounts, SeatSpec, SpectatorPolicy, StateSizeClass,
    SubstitutionPolicy, TurnModel, VoiceRequirement,
};
use tabula_testkit::conformance::security;
use tabula_testkit::determinism::{run_typed, Scenario};
use tabula_testkit::projection::{
    assert_no_event_bypasses_redaction, assert_no_leaks, assert_projection_differs,
    assert_projection_noninterference, assert_view_event_differs,
    assert_view_event_noninterference, Secret, SecretModel,
};
use tabula_testkit::{GameTestFixture, HiddenInformationFixture};

// ---------------------------------------------------------------------------
// The reference model: a public round counter plus one hidden hand per seat,
// each hand set exactly once by a legal `Deal` command.
// ---------------------------------------------------------------------------

/// Canonical state. `hands` is the game's *persistent* secret: each seat's
/// own hand is authorized to that seat and to `Viewer::Audit`; nobody else
/// may see it. `bid_counts` is deliberately public (how many bids a seat has
/// submitted) — the bid *amount* is never stored here at all; see
/// `Command::SubmitSecretBid` and `Event::BidSubmitted` for why that matters.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct State {
    public_round: u32,
    hands: BTreeMap<SeatId, Vec<u8>>,
    bid_counts: BTreeMap<SeatId, u32>,
}

/// The only things that can happen in this reference game — deliberately
/// tiny, but real transitions, so that every `State` this file exercises is
/// produced by `GameRules::apply`, never by a struct literal.
#[derive(Clone, Debug, Serialize, Deserialize)]
enum Command {
    /// Deal the acting seat's own hand. Legal exactly once per seat, from an
    /// empty hand — this is the ONLY way `hands` ever becomes non-empty.
    Deal { cards: Vec<u8> },
    /// Advance the public round counter. Legal from any seat, any time — a
    /// minimal public-state transition, used only to give the P3 positive
    /// control a real reachable "public fact changed" pair.
    AdvanceRound,
    /// Submit a sealed bid. Legal from any seat, any time. `state_after`
    /// only ever records that a bid happened (`bid_counts` +1) — `amount`
    /// itself is never written into `State` anywhere. This is deliberate:
    /// it is the smallest reachable case of a secret that lives ONLY in a
    /// canonical `Event`, to exercise `SecretModel::event_secrets` (see
    /// `event_secret_never_touches_state` below).
    SubmitSecretBid { amount: u8 },
}

fn initial_state(roster: &SeatRoster) -> State {
    State {
        public_round: 0,
        hands: roster.iter().map(|e| (e.seat, Vec::new())).collect(),
        bid_counts: roster.iter().map(|e| (e.seat, 0u32)).collect(),
    }
}

/// The single mutation rule shared by [`Rules`] and [`LeakyRules`] below —
/// they differ only in `project`, so this is factored out rather than
/// duplicated.
fn apply_command(state: &mut State, input: &Input<Command>) -> Result<(), RuleError> {
    match input {
        Input::Player {
            seat,
            command: Command::Deal { cards },
        } => {
            let hand = state
                .hands
                .get_mut(seat)
                .ok_or_else(|| RuleError::code(RuleErrorCode::NoSuchSeat))?;
            if !hand.is_empty() {
                // Re-dealing is not part of this model; every state stays
                // dealt-once, which is all the noninterference property below
                // needs.
                return Err(RuleError::code(RuleErrorCode::IllegalMove));
            }
            hand.clone_from(cards);
            Ok(())
        }
        Input::Player {
            command: Command::AdvanceRound,
            ..
        } => {
            state.public_round += 1;
            Ok(())
        }
        Input::Player {
            seat,
            command: Command::SubmitSecretBid { .. },
        } => {
            let count = state
                .bid_counts
                .get_mut(seat)
                .ok_or_else(|| RuleError::code(RuleErrorCode::NoSuchSeat))?;
            // The amount itself is intentionally dropped here — it never
            // becomes part of `State`. Only the count is public.
            *count += 1;
            Ok(())
        }
        _ => Err(RuleError::code(RuleErrorCode::UnknownCommand)),
    }
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

fn project_honestly(state: &State, viewer: Viewer) -> View {
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

/// Canonical, full-information record of what happened (doc 00 §5, I-6). One
/// variant per [`Command`] this model has, exactly mirroring it — this is the
/// "meaningful canonical event" this PR's item 11 asks for, in place of the
/// dummy `Event = ()` PR #37 used (that PR only needed `project`/state
/// noninterference; this one also needs to exercise `view_event`).
#[derive(Clone, Debug, Serialize, Deserialize)]
enum Event {
    /// The dealt seat's own cards, in the clear — this is canonical, never
    /// redacted at this layer (doc 02 §12.2's `Event::Dealt`).
    Dealt {
        seat: SeatId,
        cards: Vec<u8>,
    },
    RoundAdvanced {
        round: u32,
    },
    /// The one secret in this file that `state_after` never holds: `amount`
    /// is reported here and nowhere else (`apply_command` drops it, keeping
    /// only `bid_counts`). See `SecretModel::event_secrets`.
    BidSubmitted {
        seat: SeatId,
        amount: u8,
    },
}

/// Per-viewer redacted event. A genuinely different *type* from [`Event`]
/// (doc 02 §7.2: never `type ViewEvent = Event`), matching the
/// `Dealt`/`DealtToOther` degrade-not-hide shape doc 02 §12.2 uses for real
/// cards.
#[derive(Clone, Debug, Serialize)]
enum ViewEvent {
    /// The dealt seat learns its own cards.
    DealtToYou {
        cards: Vec<u8>,
    },
    /// Everyone else learns only that a deal happened, and how many cards —
    /// the "card back flies across the table" case (doc 02 §7.2) — never the
    /// content.
    DealtToOther {
        seat: SeatId,
        count: usize,
    },
    /// `Viewer::Audit` sees the full canonical event (doc 00 §9.4).
    DealtForAudit {
        seat: SeatId,
        cards: Vec<u8>,
    },
    RoundAdvanced {
        round: u32,
    },
    /// The bidder learns its own amount.
    BidSubmittedToYou {
        amount: u8,
    },
    /// Everyone else learns only that a bid happened, and by whom — never
    /// the amount.
    BidSubmittedByOther {
        seat: SeatId,
    },
    /// `Viewer::Audit` sees the full canonical event (doc 00 §9.4).
    BidSubmittedForAudit {
        seat: SeatId,
        amount: u8,
    },
}

/// Build the canonical event a just-applied `input` produced, from the
/// post-mutation `state`. Shared by every `GameRules` impl in this file that
/// uses real events ([`Rules`], [`BypassingRules`], [`DerivedEventLeakRules`]
/// below) so their event shape cannot drift from `apply_command`'s mutation.
fn command_event(input: &Input<Command>, state: &State) -> Event {
    match input {
        Input::Player {
            seat,
            command: Command::Deal { cards },
        } => Event::Dealt {
            seat: *seat,
            cards: cards.clone(),
        },
        Input::Player {
            command: Command::AdvanceRound,
            ..
        } => Event::RoundAdvanced {
            round: state.public_round,
        },
        Input::Player {
            seat,
            command: Command::SubmitSecretBid { amount },
        } => Event::BidSubmitted {
            seat: *seat,
            amount: *amount,
        },
        _ => unreachable!("apply_command already rejected any other input shape"),
    }
}

/// The honest redaction: an owner learns its own cards/bid, everyone else
/// (any other seat, and any spectator) learns only that something happened
/// and, where relevant, by whom — never the hidden content — and
/// `Viewer::Audit` sees the canonical event in full. The same authorization
/// shape [`project_honestly`] uses for state, applied to one event.
fn view_event_honestly(event: &Event, viewer: Viewer) -> ViewEvent {
    match event {
        Event::Dealt { seat, cards } => match viewer {
            Viewer::Seat(s) if s == *seat => ViewEvent::DealtToYou {
                cards: cards.clone(),
            },
            Viewer::Seat(_) | Viewer::Spectator(_) => ViewEvent::DealtToOther {
                seat: *seat,
                count: cards.len(),
            },
            Viewer::Audit => ViewEvent::DealtForAudit {
                seat: *seat,
                cards: cards.clone(),
            },
        },
        Event::RoundAdvanced { round } => ViewEvent::RoundAdvanced { round: *round },
        Event::BidSubmitted { seat, amount } => match viewer {
            Viewer::Seat(s) if s == *seat => ViewEvent::BidSubmittedToYou { amount: *amount },
            Viewer::Seat(_) | Viewer::Spectator(_) => {
                ViewEvent::BidSubmittedByOther { seat: *seat }
            }
            Viewer::Audit => ViewEvent::BidSubmittedForAudit {
                seat: *seat,
                amount: *amount,
            },
        },
    }
}

/// The [`SecretModel::event_secrets`] every real-event `GameRules` impl in
/// this file that models bids shares ([`Rules`], [`BypassingRules`]): the
/// amount from one `Event::BidSubmitted`, authorized only to the bidding
/// seat. Nothing here reads `state_after` — the whole point is that this
/// secret has no home there at all (see `Command::SubmitSecretBid`'s docs).
fn bid_event_secrets(event: &Event) -> Vec<Secret> {
    match event {
        Event::BidSubmitted { seat, amount } => vec![Secret::authorized(
            &format!("seat {}'s bid amount", seat.0),
            vec![vec![*amount]],
            vec![Viewer::Seat(*seat)],
        )],
        Event::Dealt { .. } | Event::RoundAdvanced { .. } => Vec::new(),
    }
}

/// The [`SecretModel`] every `GameRules` impl in this file shares: each
/// seat's own hand, authorized only to that seat — never to
/// `Viewer::Audit`, which this file treats as a documented, separately
/// controlled exception rather than folding it into "authorized" (this PR's
/// item 5/12). A seat that has not been dealt yet (an empty hand) has
/// nothing to declare: an empty token would be a malformed declaration (item
/// 13), not a secret, and there is genuinely no card to leak yet.
///
/// # Token granularity
///
/// One token per hand — the whole dealt `Vec<u8>`, not one token per card.
/// A per-card token here would be a single byte, and a single byte is not
/// collision-resistant: it would spuriously "match" `public_round`,
/// `hand_counts` entries, or any other small integer serialized nearby,
/// making the scan noisy rather than strict (item 12). The whole-hand token
/// is exactly the bytes `serde` emits for a `Vec<u8>` field, so it appears
/// verbatim inside a `View`/`ViewEvent` that embeds the hand honestly, and
/// the tests below choose hand contents ([`AAAA`]-style byte pairs distinct
/// from any nearby public integer) so an accidental byte-sequence collision
/// with an unrelated field is not a realistic risk for this fixture.
fn hand_secrets(state: &State) -> Vec<Secret> {
    state
        .hands
        .iter()
        .filter(|(_, hand)| !hand.is_empty())
        .map(|(seat, hand)| {
            Secret::authorized(
                &format!("seat {}'s hand", seat.0),
                vec![hand.clone()],
                vec![Viewer::Seat(*seat)],
            )
        })
        .collect()
}

struct Rules;

impl GameRules for Rules {
    type State = State;
    type Command = Command;
    type Event = Event;
    type View = View;
    type ViewEvent = ViewEvent;
    type Config = ();

    const RULES_VERSION: RulesVersion = RulesVersion(1);

    fn create((): &(), roster: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        Ok(Init {
            state: initial_state(roster),
            events: SmallVec::new(),
            effects: SmallVec::new(),
        })
    }

    fn apply(
        state: &mut State,
        input: Input<Command>,
        _: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        apply_command(state, &input)?;
        let event = command_event(&input, state);
        Ok(Outcome {
            events: smallvec![event],
            effects: SmallVec::new(),
        })
    }

    fn project(state: &State, viewer: Viewer) -> View {
        project_honestly(state, viewer)
    }

    fn view_event(_state_after: &State, event: &Event, viewer: Viewer) -> Option<ViewEvent> {
        Some(view_event_honestly(event, viewer))
    }
}

impl SecretModel for Rules {
    fn secrets(state: &State) -> Vec<Secret> {
        hand_secrets(state)
    }

    fn event_secrets(_state_after: &State, event: &Event) -> Vec<Secret> {
        bid_event_secrets(event)
    }
}

// ---------------------------------------------------------------------------
// The only constructor: build a state by replaying a legal `Deal`/
// `AdvanceRound` script through `create` + `apply`, exactly the path
// `run_typed` (and, in production, the match runtime) uses.
// ---------------------------------------------------------------------------

fn three_seat_roster() -> SeatRoster {
    SeatRoster::new(smallvec![
        SeatEntry {
            seat: SeatId(0),
            occupant: Occupant::Human(UserId(1)),
            team: None,
        },
        SeatEntry {
            seat: SeatId(1),
            occupant: Occupant::Human(UserId(2)),
            team: None,
        },
        SeatEntry {
            seat: SeatId(2),
            occupant: Occupant::Human(UserId(3)),
            team: None,
        },
    ])
    .expect("fixture seats are unique")
}

fn deal(seat: u8, cards: &[u8]) -> Input<Command> {
    Input::Player {
        seat: SeatId(seat),
        command: Command::Deal {
            cards: cards.to_vec(),
        },
    }
}

fn advance_round() -> Input<Command> {
    // Any seat may advance the round in this model; seat 0 is arbitrary.
    Input::Player {
        seat: SeatId(0),
        command: Command::AdvanceRound,
    }
}

fn submit_secret_bid(seat: u8, amount: u8) -> Input<Command> {
    Input::Player {
        seat: SeatId(seat),
        command: Command::SubmitSecretBid { amount },
    }
}

/// Deal `deals` in order and return the resulting **reachable** state, for
/// any `R` sharing this file's `State`/`Command`/`Config` (i.e. [`Rules`] and
/// [`LeakyRules`] below — they are two different projections of the exact
/// same reachable-state space, not two different games).
fn dealt_state<R>(script: Vec<Input<Command>>) -> State
where
    R: GameRules<State = State, Command = Command, Config = ()>,
{
    let scenario = Scenario {
        config: (),
        roster: three_seat_roster(),
        seed: MatchSeed::from_bytes([42u8; 32]),
        inputs: script,
    };
    run_typed::<R>(&scenario).expect("this file's scripts are legal by construction")
}

// ---------------------------------------------------------------------------
// P1 — projection determinism (necessary but weak on its own)
// ---------------------------------------------------------------------------

#[test]
fn same_state_and_viewer_yield_the_same_projection_every_time() {
    let state = dealt_state::<Rules>(vec![deal(0, &[1, 2]), deal(1, &[9, 9, 9])]);

    // Passing the same state as both "a" and "b" reduces the noninterference
    // check to plain determinism: project() called twice on one state must
    // agree, for every viewer kind.
    for viewer in [
        Viewer::Seat(SeatId(0)),
        Viewer::Seat(SeatId(1)),
        Viewer::Spectator(SpectatorTier::Live),
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
    // Two independently reachable states: same public round, same seat 0
    // deal; only seat 1's dealt hand differs — in *content*, not in count.
    // Hand count is public in this model (`View::hand_counts`), so a pair
    // that also changed the length would be varying something seat 0
    // legitimately observes, not only hidden information — see the property
    // test below, which generates exactly this partition systematically.
    let state_a = dealt_state::<Rules>(vec![deal(0, &[1, 2]), deal(1, &[9, 9])]);
    let state_b = dealt_state::<Rules>(vec![deal(0, &[1, 2]), deal(1, &[7, 3])]);

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
    let state_a = dealt_state::<Rules>(vec![deal(0, &[1, 2]), deal(1, &[9, 9])]);
    let state_b = dealt_state::<Rules>(vec![deal(0, &[4, 4]), deal(1, &[7, 3])]);

    // Both hands' content differs between the two states, so a spectator
    // authorized for neither must be unable to tell them apart at all.
    assert_projection_noninterference::<Rules>(
        "spectators are not player 0",
        &state_a,
        &state_b,
        Viewer::Spectator(SpectatorTier::Live),
    );
}

#[test]
fn each_seat_is_indifferent_to_every_other_seats_hand_in_a_three_seat_match() {
    // Multiple viewers (item 16): three seats, each checked against a pair
    // that varies every *other* seat's hand while holding its own fixed.
    let state_a = dealt_state::<Rules>(vec![deal(0, &[1]), deal(1, &[2, 2]), deal(2, &[3, 3, 3])]);
    let state_b = dealt_state::<Rules>(vec![deal(0, &[1]), deal(1, &[8, 8]), deal(2, &[5, 5, 5])]);

    assert_projection_noninterference::<Rules>(
        "seat 0 indifferent to seats 1 and 2",
        &state_a,
        &state_b,
        Viewer::Seat(SeatId(0)),
    );

    let state_c = dealt_state::<Rules>(vec![deal(0, &[9]), deal(1, &[2, 2]), deal(2, &[3, 3, 3])]);
    let state_d = dealt_state::<Rules>(vec![deal(0, &[6]), deal(1, &[2, 2]), deal(2, &[1, 1, 1])]);

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
    let state_a = dealt_state::<Rules>(vec![deal(0, &[1, 2]), deal(1, &[9, 9])]);
    let state_b = dealt_state::<Rules>(vec![deal(0, &[3, 4]), deal(1, &[9, 9])]);

    assert_projection_differs::<Rules>(
        "your own hand changing is visible to you",
        &state_a,
        &state_b,
        Viewer::Seat(SeatId(0)),
    );
}

#[test]
fn a_public_round_change_is_observable_to_every_viewer_kind() {
    // `state_b` is `state_a` plus one further legal transition
    // (`AdvanceRound`) — the reachable-state analogue of "the same state,
    // advanced by one public move" rather than two unrelated states that
    // happen to differ.
    let dealt = vec![deal(0, &[1, 2]), deal(1, &[9, 9])];
    let state_a = dealt_state::<Rules>(dealt.clone());
    let mut advanced = dealt;
    advanced.push(advance_round());
    let state_b = dealt_state::<Rules>(advanced);

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
    let state_a = dealt_state::<Rules>(vec![deal(0, &[1, 2]), deal(1, &[9, 9])]);
    let state_b = dealt_state::<Rules>(vec![deal(0, &[1, 2]), deal(1, &[7, 3])]);

    assert_projection_differs::<Rules>(
        "Audit sees hidden hands by design",
        &state_a,
        &state_b,
        Viewer::Audit,
    );
}

// ---------------------------------------------------------------------------
// Property test: scrambling hidden hands preserves an unauthorized viewer's
// projection, over many generated *reachable* pairs rather than a handful of
// hand-picked examples.
// ---------------------------------------------------------------------------

/// A hand is a short vector of opaque byte-valued "cards". Values do not need
/// to be unique or otherwise meaningful — the property is about their mere
/// presence and content, not any card-game rule.
///
/// **Hand *count* is public in this model** (`View::hand_counts`) — a
/// deliberate design choice mirroring a typical hidden-hand game, where the
/// number of cards/tokens a seat holds is legitimately public even though
/// their identity is not (doc 02 §7.1's `hand_counts: [u8; 4], // public`
/// pattern). So a "scramble only the secret" generator must hold each seat's
/// hand *length* fixed and vary only its *content* — this is what
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
    /// viewer seat's own dealt hand, then draw two *independent same-length*
    /// hands to deal every other seat. Both resulting states are reachable
    /// (each is `create` followed by three legal `Deal` inputs). Seat 0's
    /// projection must be identical across the pair regardless of what the
    /// other seats' hidden hands turned out to contain.
    ///
    /// This is strictly stronger than the handful of examples above: it does
    /// not just check that noninterference holds for one chosen pair, it
    /// checks that the pair's choice never mattered in the first place, over
    /// 128 generated pairs per run.
    #[test]
    fn changing_only_other_seats_secret_content_preserves_the_seated_viewers_projection(
        own_hand in arb_hand(),
        (other_hand_1_a, other_hand_1_b) in arb_hand_pair(),
        (other_hand_2_a, other_hand_2_b) in arb_hand_pair(),
    ) {
        let state_a = dealt_state::<Rules>(vec![
            deal(0, &own_hand),
            deal(1, &other_hand_1_a),
            deal(2, &other_hand_2_a),
        ]);
        let state_b = dealt_state::<Rules>(vec![
            deal(0, &own_hand),
            deal(1, &other_hand_1_b),
            deal(2, &other_hand_2_b),
        ]);

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
        (hand_0_a, hand_0_b) in arb_hand_pair(),
        (hand_1_a, hand_1_b) in arb_hand_pair(),
        (hand_2_a, hand_2_b) in arb_hand_pair(),
    ) {
        let state_a = dealt_state::<Rules>(vec![
            deal(0, &hand_0_a),
            deal(1, &hand_1_a),
            deal(2, &hand_2_a),
        ]);
        let state_b = dealt_state::<Rules>(vec![
            deal(0, &hand_0_b),
            deal(1, &hand_1_b),
            deal(2, &hand_2_b),
        ]);

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
//
// `LeakyRules` shares the exact same reachable-state space as `Rules` (same
// `State`, `Command`, and `apply_command`) — it differs only in `project`.
// That is deliberate: the states this test builds are reachable for
// `LeakyRules` for the same reason they are reachable for `Rules`.
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
        type Command = Command;
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

        fn apply(
            state: &mut State,
            input: Input<Command>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            apply_command(state, &input)?;
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
        // Same dealt hand *counts* as an honest `hand_counts` field would
        // report (2 and 2 either way) — the only thing distinguishing this
        // reachable pair is hidden card content, so `LeakyRules`'s
        // honest-looking fields agree and only the derived checksum gives
        // the leak away.
        let state_a = dealt_state::<LeakyRules>(vec![deal(0, &[1, 2]), deal(1, &[9, 9])]);
        let state_b = dealt_state::<LeakyRules>(vec![deal(0, &[1, 2]), deal(1, &[7, 3])]);

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

// ---------------------------------------------------------------------------
// Shared `GameModule` plumbing for the `HiddenInformationFixture` pipeline
// (P4-P7). Every fixture below shares this game's `GameMetadata` — nothing
// in these tests exercises catalog identity, so one is enough — and picks a
// `SpectatorPolicy` via `reference_capabilities` to exercise item 4's three
// derivable cases (`Forbidden`, `Live`, `GameControlled`).
// ---------------------------------------------------------------------------

fn reference_metadata() -> GameMetadata {
    GameMetadata::from(GameMetadataSpec {
        id: GameId::new("com.tabula.testkit.hidden-reference").unwrap(),
        version: GameVersion::new("0.0.0").unwrap(),
        rules_version: RulesVersion(1),
        name_key: I18nKey::new("test.hidden_reference.name").unwrap(),
        tagline_key: I18nKey::new("test.hidden_reference.tagline").unwrap(),
        description_key: I18nKey::new("test.hidden_reference.description").unwrap(),
        categories: vec![Category::Abstract],
        tags: Vec::new(),
        estimated_minutes: DurationRange::new(1, 1).unwrap(),
        complexity: Complexity::Light,
        content_rating: ContentRating::Everyone,
        icon: AssetRef::new("icon").unwrap(),
        hero: AssetRef::new("hero").unwrap(),
        rules_url_key: None,
    })
}

static METADATA: LazyLock<GameMetadata> = LazyLock::new(reference_metadata);

fn reference_capabilities(spectators: SpectatorPolicy) -> GameCapabilities {
    GameCapabilities::try_from(GameCapabilitiesSpec {
        seats: SeatSpec::new(SeatCounts::range(2, 8).unwrap(), None, false, true),
        turn_model: TurnModel::FreeForm,
        hidden_information: true,
        spectators,
        chat: ChatPolicy::new(Vec::new(), false).unwrap(),
        voice: VoiceRequirement::No,
        ranked: RankedSupport::No,
        async_turns: AsyncTurnPolicy::Disabled,
        reconnect: ReconnectPolicy {
            grace: Millis(0),
            notify_rules: false,
        },
        substitution: SubstitutionPolicy::Forbidden,
        pausable: false,
        durability: Durability::AckAfterApply,
        client_preview: true,
        state_size: StateSizeClass::Tiny,
        apply_budget: Budget::default(),
        max_match_duration: None,
    })
    .unwrap()
}

static CAPS_LIVE: LazyLock<GameCapabilities> =
    LazyLock::new(|| reference_capabilities(SpectatorPolicy::Live));
static CAPS_FORBIDDEN: LazyLock<GameCapabilities> =
    LazyLock::new(|| reference_capabilities(SpectatorPolicy::Forbidden));
static CAPS_GAME_CONTROLLED: LazyLock<GameCapabilities> =
    LazyLock::new(|| reference_capabilities(SpectatorPolicy::GameControlled));

// ---------------------------------------------------------------------------
// P4-P7 — the honest reference game through the real
// `HiddenInformationFixture` / `projection_security!` pipeline.
//
// This is the green path: it proves the wiring (roster -> viewer universe,
// reachable-trace replay, both containment scanners, the
// `hidden_information()` consistency check) works end to end on a game that
// does everything right, not just that the primitives can be called in
// isolation.
// ---------------------------------------------------------------------------

mod honest_pipeline {
    use super::*;

    struct Module;

    impl GameModule for Module {
        type Rules = Rules;
        fn metadata() -> &'static GameMetadata {
            &METADATA
        }
        fn capabilities() -> &'static GameCapabilities {
            &CAPS_LIVE
        }
        fn validate_config((): &(), _: &SeatRoster) -> Result<(), ConfigError> {
            Ok(())
        }
    }

    struct Fixture;

    impl GameTestFixture for Fixture {
        type Module = Module;
        fn config() {}
        fn roster() -> SeatRoster {
            three_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([42u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Command>> {
            vec![
                deal(0, &[0xAA, 0xBB]),
                deal(1, &[0x11, 0x22]),
                deal(2, &[0x33, 0x44]),
                advance_round(),
            ]
        }
    }

    impl HiddenInformationFixture for Fixture {}

    // Exercises the macro a real game crate would write, over every roster
    // seat (0, 1, 2) and the declared `Live` spectator, across `create`'s
    // step and every accepted `Deal`/`AdvanceRound` input.
    tabula_testkit::projection_security!(Fixture);
}

// ---------------------------------------------------------------------------
// The PR review's second blocker: a fixture that never actually reaches a
// secret state, or never exercises an unauthorized viewer, must not be
// allowed to report a passing security suite. `security::check` tracks
// coverage across the whole reachable trace and refuses a vacuous run.
// ---------------------------------------------------------------------------

mod vacuous_suite_is_rejected {
    use super::*;

    struct Module;
    impl GameModule for Module {
        type Rules = Rules;
        fn metadata() -> &'static GameMetadata {
            &METADATA
        }
        fn capabilities() -> &'static GameCapabilities {
            &CAPS_LIVE
        }
        fn validate_config((): &(), _: &SeatRoster) -> Result<(), ConfigError> {
            Ok(())
        }
    }

    /// The bug this fixture demonstrates: it never sends a single input, so
    /// no hand is ever dealt and no event is ever emitted. Every containment
    /// scan in `security::check` would trivially "pass" — zero secrets, zero
    /// comparisons — while checking nothing at all.
    struct Fixture;
    impl GameTestFixture for Fixture {
        type Module = Module;
        fn config() {}
        fn roster() -> SeatRoster {
            three_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([42u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Command>> {
            Vec::new()
        }
    }
    impl HiddenInformationFixture for Fixture {}

    #[test]
    fn an_empty_script_that_never_touches_a_secret_is_rejected_as_vacuous() {
        let result = catch_unwind(security::check::<Fixture>);

        let Err(payload) = result else {
            panic!(
                "security::check ACCEPTED a fixture whose deterministic_script() never deals a \
                 hand or exercises any secret — a hidden-information suite that passes over zero \
                 real comparisons proves nothing, exactly the \"green tick that means nothing\" \
                 failure mode tabula-testkit's own conformance docs warn about."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("never reached a state where SecretModel::secrets declared anything"),
            "the check rejected the vacuous fixture but not with the expected diagnostic; \
             got: {message}"
        );
    }
}

// ---------------------------------------------------------------------------
// P7 — the viewer universe must come from roster + capabilities, not from
// whichever seats a human test author happened to check.
//
// `LeakyToUnlistedSeatRules` leaks seat 0's hand verbatim to seat 2 — a seat
// that is part of the roster but is never named by any `Secret`'s
// `authorized` list, by any `Command`, or by this test file's other
// hand-picked viewer lists (which only ever exercise seats 0 and 1
// explicitly elsewhere). If `client_viewer_universe` did not derive "every
// roster seat" automatically, nothing here would ever construct
// `Viewer::Seat(2)` and this leak would pass silently (this PR's item 4/17).
// ---------------------------------------------------------------------------

mod viewer_universe_completeness {
    use super::*;

    #[derive(Clone, Debug, Serialize)]
    struct UnlistedSeatLeakView {
        public_round: u32,
        your_hand: Option<Vec<u8>>,
        hand_counts: BTreeMap<SeatId, usize>,
        /// THE BUG: seat 0's actual hand, handed unconditionally to seat 2.
        leaked_to_seat_2: Option<Vec<u8>>,
    }

    struct LeakyToUnlistedSeatRules;

    impl GameRules for LeakyToUnlistedSeatRules {
        type State = State;
        type Command = Command;
        type Event = ();
        type View = UnlistedSeatLeakView;
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

        fn apply(
            state: &mut State,
            input: Input<Command>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            apply_command(state, &input)?;
            Ok(Outcome::empty())
        }

        fn project(state: &State, viewer: Viewer) -> UnlistedSeatLeakView {
            let hand_counts = state.hands.iter().map(|(s, h)| (*s, h.len())).collect();
            UnlistedSeatLeakView {
                public_round: state.public_round,
                your_hand: match viewer {
                    Viewer::Seat(seat) => state.hands.get(&seat).cloned(),
                    Viewer::Spectator(_) | Viewer::Audit => None,
                },
                hand_counts,
                leaked_to_seat_2: if viewer == Viewer::Seat(SeatId(2)) {
                    state.hands.get(&SeatId(0)).cloned()
                } else {
                    None
                },
            }
        }

        fn view_event(_: &State, (): &(), _: Viewer) -> Option<()> {
            None
        }
    }

    impl SecretModel for LeakyToUnlistedSeatRules {
        fn secrets(state: &State) -> Vec<Secret> {
            hand_secrets(state)
        }
    }

    struct Module;
    impl GameModule for Module {
        type Rules = LeakyToUnlistedSeatRules;
        fn metadata() -> &'static GameMetadata {
            &METADATA
        }
        fn capabilities() -> &'static GameCapabilities {
            &CAPS_FORBIDDEN
        }
        fn validate_config((): &(), _: &SeatRoster) -> Result<(), ConfigError> {
            Ok(())
        }
    }

    struct Fixture;
    impl GameTestFixture for Fixture {
        type Module = Module;
        fn config() {}
        fn roster() -> SeatRoster {
            three_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([42u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Command>> {
            vec![
                deal(0, &[0xAA, 0xBB]),
                deal(1, &[0x11, 0x22]),
                deal(2, &[0x33, 0x44]),
            ]
        }
    }
    impl HiddenInformationFixture for Fixture {}

    #[test]
    fn viewer_universe_derivation_catches_a_leak_to_a_seat_no_secret_ever_named() {
        let result = catch_unwind(security::check::<Fixture>);

        let Err(payload) = result else {
            panic!(
                "the hidden-information security suite ACCEPTED a leak of seat 0's hand to \
                 seat 2, a roster seat no Secret's `authorized` list ever mentioned. If this \
                 passes, the viewer universe is not actually covering every roster seat."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("projection secrecy violated"),
            "the suite rejected the leaky fixture but not with the expected diagnostic; \
             got: {message}"
        );
        assert!(
            message.contains("SeatId(2)"),
            "the failure did not name the unlisted seat that actually received the leak; \
             got: {message}"
        );
    }
}

// ---------------------------------------------------------------------------
// Item 4 — `SpectatorPolicy::GameControlled` must not be silently guessed or
// skipped: the harness requires an explicit fixture hook naming the concrete
// spectator viewers, and fails loudly if that hook is not overridden.
// ---------------------------------------------------------------------------

mod game_controlled_spectators {
    use super::*;

    struct Module;
    impl GameModule for Module {
        type Rules = Rules;
        fn metadata() -> &'static GameMetadata {
            &METADATA
        }
        fn capabilities() -> &'static GameCapabilities {
            &CAPS_GAME_CONTROLLED
        }
        fn validate_config((): &(), _: &SeatRoster) -> Result<(), ConfigError> {
            Ok(())
        }
    }

    fn fixture_script() -> Vec<Input<Command>> {
        vec![deal(0, &[0xAA, 0xBB]), deal(1, &[0x11, 0x22])]
    }

    struct FixtureWithHook;
    impl GameTestFixture for FixtureWithHook {
        type Module = Module;
        fn config() {}
        fn roster() -> SeatRoster {
            three_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([42u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Command>> {
            fixture_script()
        }
    }
    impl HiddenInformationFixture for FixtureWithHook {
        fn game_controlled_spectators() -> Option<Vec<SpectatorTier>> {
            Some(vec![SpectatorTier::Live])
        }
    }

    struct FixtureMissingHook;
    impl GameTestFixture for FixtureMissingHook {
        type Module = Module;
        fn config() {}
        fn roster() -> SeatRoster {
            three_seat_roster()
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes([42u8; 32])
        }
        fn deterministic_script() -> Vec<Input<Command>> {
            fixture_script()
        }
    }
    impl HiddenInformationFixture for FixtureMissingHook {}

    #[test]
    fn an_explicit_hook_names_the_game_controlled_spectators_and_the_suite_passes() {
        security::check::<FixtureWithHook>();
    }

    #[test]
    fn a_missing_hook_fails_loudly_instead_of_silently_scanning_no_spectators() {
        let result = catch_unwind(security::client_viewer_universe::<FixtureMissingHook>);

        let Err(payload) = result else {
            panic!(
                "client_viewer_universe ACCEPTED a GameControlled spectator policy with no \
                 game_controlled_spectators() override, silently scanning zero spectators \
                 instead of failing loudly."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("GameControlled") && message.contains("game_controlled_spectators"),
            "the panic did not explain which hook needed to be overridden; got: {message}"
        );
    }
}

// ---------------------------------------------------------------------------
// Containment scanner: direct leaks (this PR's item 15, first two bullets).
// ---------------------------------------------------------------------------

mod direct_leaks_are_caught {
    use super::*;

    /// Bug: spectators receive every seat's actual hand, verbatim, alongside
    /// the honest fields. Oracle A (containment) must catch this — it is
    /// exactly "a whole card list leaking" from doc 02 §7.3.
    #[derive(Clone, Debug, Serialize)]
    struct SpectatorLeakView {
        public_round: u32,
        your_hand: Option<Vec<u8>>,
        hand_counts: BTreeMap<SeatId, usize>,
        all_hands_for_spectator: Option<BTreeMap<SeatId, Vec<u8>>>,
    }

    struct LeakyProjectionRules;

    impl GameRules for LeakyProjectionRules {
        type State = State;
        type Command = Command;
        type Event = ();
        type View = SpectatorLeakView;
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

        fn apply(
            state: &mut State,
            input: Input<Command>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            apply_command(state, &input)?;
            Ok(Outcome::empty())
        }

        fn project(state: &State, viewer: Viewer) -> SpectatorLeakView {
            let hand_counts = state.hands.iter().map(|(s, h)| (*s, h.len())).collect();
            SpectatorLeakView {
                public_round: state.public_round,
                your_hand: match viewer {
                    Viewer::Seat(seat) => state.hands.get(&seat).cloned(),
                    Viewer::Spectator(_) | Viewer::Audit => None,
                },
                hand_counts,
                all_hands_for_spectator: matches!(viewer, Viewer::Spectator(_))
                    .then(|| state.hands.clone()),
            }
        }

        fn view_event(_: &State, (): &(), _: Viewer) -> Option<()> {
            None
        }
    }

    impl SecretModel for LeakyProjectionRules {
        fn secrets(state: &State) -> Vec<Secret> {
            hand_secrets(state)
        }
    }

    #[test]
    fn containment_scanner_catches_a_direct_projection_leak_to_a_spectator() {
        let state = dealt_state::<LeakyProjectionRules>(vec![
            deal(0, &[0xAA, 0xBB]),
            deal(1, &[0x11, 0x22]),
        ]);

        let result = catch_unwind(AssertUnwindSafe(|| {
            assert_no_leaks::<LeakyProjectionRules>(
                "direct spectator leak",
                &state,
                &[
                    Viewer::Seat(SeatId(0)),
                    Viewer::Seat(SeatId(1)),
                    Viewer::Spectator(SpectatorTier::Live),
                ],
            );
        }));

        let Err(payload) = result else {
            panic!(
                "assert_no_leaks ACCEPTED a spectator View containing another seat's hand \
                 verbatim. The containment oracle is not enforcing anything."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("projection secrecy violated"),
            "the check panicked but not with the expected diagnostic; got: {message}"
        );
    }

    /// Bug: `view_event` is the "new Event variant added, match arm falls
    /// through to a catch-all `Some(..)`" failure I-6 exists to prevent — it
    /// forwards the canonical event, unredacted, to every viewer.
    struct BypassingRules;

    impl GameRules for BypassingRules {
        type State = State;
        type Command = Command;
        type Event = Event;
        type View = View;
        type ViewEvent = Event;
        type Config = ();

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        fn create((): &(), roster: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: initial_state(roster),
                events: SmallVec::new(),
                effects: SmallVec::new(),
            })
        }

        fn apply(
            state: &mut State,
            input: Input<Command>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            apply_command(state, &input)?;
            let event = command_event(&input, state);
            Ok(Outcome {
                events: smallvec![event],
                effects: SmallVec::new(),
            })
        }

        fn project(state: &State, viewer: Viewer) -> View {
            project_honestly(state, viewer)
        }

        fn view_event(_: &State, event: &Event, _viewer: Viewer) -> Option<Event> {
            // THE BUG.
            Some(event.clone())
        }
    }

    impl SecretModel for BypassingRules {
        fn secrets(state: &State) -> Vec<Secret> {
            hand_secrets(state)
        }

        fn event_secrets(_state_after: &State, event: &Event) -> Vec<Secret> {
            bid_event_secrets(event)
        }
    }

    #[test]
    fn event_containment_scanner_catches_a_direct_view_event_leak() {
        let state =
            dealt_state::<BypassingRules>(vec![deal(0, &[0xAA, 0xBB]), deal(1, &[0x11, 0x22])]);
        let event = Event::Dealt {
            seat: SeatId(1),
            cards: vec![0x11, 0x22],
        };

        let result = catch_unwind(AssertUnwindSafe(|| {
            assert_no_event_bypasses_redaction::<BypassingRules>(
                "view_event forwards the canonical event verbatim",
                &state,
                &[event],
                &[
                    Viewer::Seat(SeatId(0)),
                    Viewer::Spectator(SpectatorTier::Live),
                ],
            );
        }));

        let Err(payload) = result else {
            panic!(
                "assert_no_event_bypasses_redaction ACCEPTED a ViewEvent that forwards a \
                 canonical event verbatim to an unauthorized viewer. The event containment \
                 oracle is not enforcing anything."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("event secrecy violated"),
            "the check panicked but not with the expected diagnostic; got: {message}"
        );
    }

    /// The PR review's headline blocker: an event containment scan that only
    /// ever consults `SecretModel::secrets(state_after)` has literally
    /// nothing to compare against for a secret that never touches `State` —
    /// a bid amount, reported once in `Event::BidSubmitted` and then
    /// dropped. `SecretModel::event_secrets` exists precisely to close this
    /// gap; this test fails if `assert_no_event_bypasses_redaction` is ever
    /// changed back to consulting only `secrets(state_after)`.
    #[test]
    fn event_secret_never_touches_state_is_still_caught() {
        // No hands dealt at all — only a bid. `hand_secrets` (and therefore
        // `BypassingRules::secrets`) has nothing to say about this state.
        let state = dealt_state::<BypassingRules>(vec![submit_secret_bid(1, 17)]);
        let event = Event::BidSubmitted {
            seat: SeatId(1),
            amount: 17,
        };

        // Structural proof the gap is real, not merely asserted: state-based
        // secrets are provably empty here, so a scanner that consulted only
        // `secrets(state_after)` would have had zero tokens to check this
        // event against and would have silently accepted the leak below.
        assert!(
            BypassingRules::secrets(&state).is_empty(),
            "test setup: this script must not create any state-persistent secret, or it would \
             stop isolating the event-local-secret case this test exists to prove"
        );

        let result = catch_unwind(AssertUnwindSafe(|| {
            assert_no_event_bypasses_redaction::<BypassingRules>(
                "a secret that lives only in the event, never in state_after",
                &state,
                &[event],
                &[
                    Viewer::Seat(SeatId(0)),
                    Viewer::Spectator(SpectatorTier::Live),
                ],
            );
        }));

        let Err(payload) = result else {
            panic!(
                "assert_no_event_bypasses_redaction ACCEPTED a leak of a secret that exists ONLY \
                 in the canonical event, never in state_after. A scanner that only consults \
                 SecretModel::secrets(state_after) is blind to exactly this class of leak — see \
                 SecretModel::event_secrets."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("event secrecy violated") && message.contains("bid amount"),
            "the check panicked but not with the expected diagnostic; got: {message}"
        );
    }
}

// ---------------------------------------------------------------------------
// Item 15 (third bullet) / item 10 — a derived event leak: same public
// seat/count, different hidden cards, `ViewEvent` exposes a checksum. Proves
// containment cannot see it and event noninterference does.
// ---------------------------------------------------------------------------

mod derived_event_leak_needs_noninterference {
    use super::*;

    #[derive(Clone, Debug, Serialize)]
    enum DerivedLeakViewEvent {
        DealtToYou {
            cards: Vec<u8>,
        },
        DealtToOtherWithChecksum {
            seat: SeatId,
            count: usize,
            checksum: u32,
        },
        RoundAdvanced {
            round: u32,
        },
    }

    struct DerivedEventLeakRules;

    impl GameRules for DerivedEventLeakRules {
        type State = State;
        type Command = Command;
        type Event = Event;
        type View = View;
        type ViewEvent = DerivedLeakViewEvent;
        type Config = ();

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        fn create((): &(), roster: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: initial_state(roster),
                events: SmallVec::new(),
                effects: SmallVec::new(),
            })
        }

        fn apply(
            state: &mut State,
            input: Input<Command>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            apply_command(state, &input)?;
            let event = command_event(&input, state);
            Ok(Outcome {
                events: smallvec![event],
                effects: SmallVec::new(),
            })
        }

        fn project(state: &State, viewer: Viewer) -> View {
            project_honestly(state, viewer)
        }

        fn view_event(_: &State, event: &Event, viewer: Viewer) -> Option<DerivedLeakViewEvent> {
            Some(match event {
                Event::Dealt { seat, cards } => match viewer {
                    Viewer::Seat(s) if s == *seat => DerivedLeakViewEvent::DealtToYou {
                        cards: cards.clone(),
                    },
                    _ => DerivedLeakViewEvent::DealtToOtherWithChecksum {
                        seat: *seat,
                        count: cards.len(),
                        // THE BUG: derived from every hidden card, never
                        // copied verbatim, so a token-containment scan
                        // structurally cannot see it.
                        checksum: cards.iter().map(|&b| u32::from(b)).sum(),
                    },
                },
                Event::RoundAdvanced { round } => {
                    DerivedLeakViewEvent::RoundAdvanced { round: *round }
                }
                Event::BidSubmitted { .. } => unreachable!(
                    "this fixture's tests only ever submit Deal/AdvanceRound inputs — it exists \
                     to isolate the Dealt-checksum leak, not to model bids"
                ),
            })
        }
    }

    impl SecretModel for DerivedEventLeakRules {
        fn secrets(state: &State) -> Vec<Secret> {
            hand_secrets(state)
        }
    }

    #[test]
    fn event_containment_is_blind_to_a_derived_checksum_leak() {
        // Same public facts (seat 1, count 2) either way — only card CONTENT
        // differs, exactly the partition that defeats byte containment.
        let state = dealt_state::<DerivedEventLeakRules>(vec![
            deal(0, &[0xAA, 0xBB]),
            deal(1, &[0x11, 0x22]),
        ]);
        let event = Event::Dealt {
            seat: SeatId(1),
            cards: vec![0x11, 0x22],
        };

        // This is the documented residual gap (item 2/15), not a bug: the
        // containment scan passes here on purpose, demonstrating why
        // noninterference must exist as an independent oracle.
        assert_no_event_bypasses_redaction::<DerivedEventLeakRules>(
            "derived checksum leak is invisible to containment",
            &state,
            &[event],
            &[Viewer::Seat(SeatId(0))],
        );
    }

    #[test]
    fn event_noninterference_catches_the_derived_checksum_leak_containment_missed() {
        let state_a = dealt_state::<DerivedEventLeakRules>(vec![
            deal(0, &[0xAA, 0xBB]),
            deal(1, &[0x11, 0x22]),
        ]);
        let state_b = dealt_state::<DerivedEventLeakRules>(vec![
            deal(0, &[0xAA, 0xBB]),
            deal(1, &[0x33, 0x44]),
        ]);
        let event_a = Event::Dealt {
            seat: SeatId(1),
            cards: vec![0x11, 0x22],
        };
        let event_b = Event::Dealt {
            seat: SeatId(1),
            cards: vec![0x33, 0x44],
        };

        let result = catch_unwind(AssertUnwindSafe(|| {
            assert_view_event_noninterference::<DerivedEventLeakRules>(
                "derived checksum leak",
                &state_a,
                &event_a,
                &state_b,
                &event_b,
                Viewer::Seat(SeatId(0)),
            );
        }));

        let Err(payload) = result else {
            panic!(
                "assert_view_event_noninterference ACCEPTED a ViewEvent carrying a checksum \
                 derived from a hidden hand it never copied verbatim. Noninterference is the \
                 only oracle that can catch this class of leak, and it did not."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("event noninterference violated"),
            "the check panicked but not with the expected diagnostic; got: {message}"
        );
    }
}

// ---------------------------------------------------------------------------
// Event noninterference and its positive controls, on the HONEST reference
// model (item 16 / item 25's "Event noninterference" list).
// ---------------------------------------------------------------------------

#[test]
fn unauthorized_seat_and_spectator_receive_degraded_deal_detail_not_the_cards() {
    let state = dealt_state::<Rules>(vec![deal(0, &[0xAA, 0xBB]), deal(1, &[0x11, 0x22])]);
    let event = Event::Dealt {
        seat: SeatId(1),
        cards: vec![0x11, 0x22],
    };

    // Every canonical event actually passes through view_event...
    let to_other_seat = Rules::view_event(&state, &event, Viewer::Seat(SeatId(0)));
    assert!(matches!(
        to_other_seat,
        Some(ViewEvent::DealtToOther {
            seat: SeatId(1),
            count: 2
        })
    ));

    let to_spectator = Rules::view_event(&state, &event, Viewer::Spectator(SpectatorTier::Live));
    assert!(matches!(
        to_spectator,
        Some(ViewEvent::DealtToOther {
            seat: SeatId(1),
            count: 2
        })
    ));

    // ...and the authorized owner gets the real detail...
    let to_owner = Rules::view_event(&state, &event, Viewer::Seat(SeatId(1)));
    assert!(matches!(
        to_owner,
        Some(ViewEvent::DealtToYou { ref cards }) if cards == &[0x11, 0x22]
    ));

    // ...and Audit, a documented exception, sees the canonical detail too.
    let to_audit = Rules::view_event(&state, &event, Viewer::Audit);
    assert!(matches!(
        to_audit,
        Some(ViewEvent::DealtForAudit { seat: SeatId(1), ref cards }) if cards == &[0x11, 0x22]
    ));
}

#[test]
fn unauthorized_seat_and_spectator_are_indifferent_to_another_seats_deal_detail() {
    // Same public facts (seat 1 dealt, count 2) either way; only seat 1's
    // card CONTENT differs — the partition that isolates the property from
    // "count is secret too", exactly like the state-projection property test
    // above.
    let state_a = dealt_state::<Rules>(vec![deal(0, &[0xAA, 0xBB]), deal(1, &[0x11, 0x22])]);
    let state_b = dealt_state::<Rules>(vec![deal(0, &[0xAA, 0xBB]), deal(1, &[0x33, 0x44])]);
    let event_a = Event::Dealt {
        seat: SeatId(1),
        cards: vec![0x11, 0x22],
    };
    let event_b = Event::Dealt {
        seat: SeatId(1),
        cards: vec![0x33, 0x44],
    };

    for viewer in [
        Viewer::Seat(SeatId(0)),
        Viewer::Spectator(SpectatorTier::Live),
    ] {
        assert_view_event_noninterference::<Rules>(
            "seat 1's dealt cards are not seat 0's or a spectator's business",
            &state_a,
            &event_a,
            &state_b,
            &event_b,
            viewer,
        );
    }
}

#[test]
fn owner_receives_authorized_deal_detail_that_differs_with_its_own_cards() {
    let state_a = dealt_state::<Rules>(vec![deal(0, &[0xAA, 0xBB]), deal(1, &[0x11, 0x22])]);
    let state_b = dealt_state::<Rules>(vec![deal(0, &[0xCC, 0xDD]), deal(1, &[0x11, 0x22])]);
    let event_a = Event::Dealt {
        seat: SeatId(0),
        cards: vec![0xAA, 0xBB],
    };
    let event_b = Event::Dealt {
        seat: SeatId(0),
        cards: vec![0xCC, 0xDD],
    };

    assert_view_event_differs::<Rules>(
        "the owner sees its own dealt cards change",
        &state_a,
        &event_a,
        &state_b,
        &event_b,
        Viewer::Seat(SeatId(0)),
    );
}

#[test]
fn audit_sees_full_deal_detail_change_regardless_of_seat() {
    // Not a leak: doc 00 §9.4 states Audit "sees canonical information".
    let state_a = dealt_state::<Rules>(vec![deal(0, &[0xAA, 0xBB]), deal(1, &[0x11, 0x22])]);
    let state_b = dealt_state::<Rules>(vec![deal(0, &[0xAA, 0xBB]), deal(1, &[0x33, 0x44])]);
    let event_a = Event::Dealt {
        seat: SeatId(1),
        cards: vec![0x11, 0x22],
    };
    let event_b = Event::Dealt {
        seat: SeatId(1),
        cards: vec![0x33, 0x44],
    };

    assert_view_event_differs::<Rules>(
        "Audit sees hidden deal detail by design",
        &state_a,
        &event_a,
        &state_b,
        &event_b,
        Viewer::Audit,
    );
}

// ---------------------------------------------------------------------------
// Item 13 — a malformed (empty) secret token must fail loudly and
// distinguishably from an actual leak, never silently pass.
// ---------------------------------------------------------------------------

mod malformed_secret_token_fails_loudly {
    use super::*;

    struct EmptyTokenRules;

    impl GameRules for EmptyTokenRules {
        type State = State;
        type Command = Command;
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

        fn apply(
            state: &mut State,
            input: Input<Command>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            apply_command(state, &input)?;
            Ok(Outcome::empty())
        }

        fn project(state: &State, viewer: Viewer) -> View {
            project_honestly(state, viewer)
        }

        fn view_event(_: &State, (): &(), _: Viewer) -> Option<()> {
            None
        }
    }

    impl SecretModel for EmptyTokenRules {
        fn secrets(_state: &State) -> Vec<Secret> {
            // THE BUG: an empty token matches every byte string.
            vec![Secret::authorized(
                "malformed secret",
                vec![Vec::new()],
                vec![Viewer::Seat(SeatId(0))],
            )]
        }
    }

    #[test]
    fn containment_scanner_fails_loudly_on_an_empty_token_instead_of_silently_passing() {
        let state = dealt_state::<EmptyTokenRules>(Vec::new());

        let result = catch_unwind(AssertUnwindSafe(|| {
            assert_no_leaks::<EmptyTokenRules>(
                "malformed secret",
                &state,
                &[Viewer::Seat(SeatId(1))],
            );
        }));

        let Err(payload) = result else {
            panic!(
                "assert_no_leaks silently accepted a SecretModel that declared an empty token. \
                 An empty token matches every projection, so this must fail loudly rather than \
                 pass or (worse) report a leak on every unrelated viewer."
            );
        };
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("malformed SecretModel"),
            "the panic did not identify the declaration as malformed (as opposed to a real \
             leak); got: {message}"
        );
    }
}
