//! `impl GameRules for TilesRules`. (doc 02 §12.4)
//!
//! # Shape
//!
//! The same **validate fully, then mutate** structure tic-tac-toe demonstrates
//! (doc 02 §10.2), scaled up: every command has a `validate_*` half returning a
//! witness of what it proved, and a `commit_*` half that cannot be reached
//! without one. Contract R2 is therefore structural rather than remembered.
//!
//! # Randomness is drawn once
//!
//! `create` shuffles the bag with `ctx.rng.stream(DOMAIN_SHUFFLE)` and **no
//! later input touches `ctx.rng` at all** — a draw is a `pop`. That is not an
//! optimisation: it means a rejected input cannot consume randomness in the
//! first place, so contract R8 holds by construction rather than by rollback,
//! and a replay reproduces every draw from the seed alone.
//!
//! @ai.role functional-core
//! @ai.domain tiles.rules
//! @ai.pure true
//! @ai.invariant rejected-input-preserves-canonical-state
//! @ai.invariant randomness-is-consumed-only-at-create
//! @ai.law validate-then-mutate
//! @ai.evidence tests::rejected_commands_leave_the_state_byte_identical
//! @ai.evidence crate::rules::tests::draws_consume_no_randomness_after_create

#![allow(clippy::doc_markdown)]

pub mod coord;
pub mod feature;
pub mod placement;
pub mod scoring;
/// The information model in code. Compiled only for tests and the `testkit`
/// feature — a server or client build carries no `SecretModel` and no
/// dependency on `tabula-testkit`.
#[cfg(any(test, feature = "testkit"))]
pub mod secret;
pub mod state;
pub mod tile;

pub use coord::{Coord, CoordError, Rotation, Side, MAX_COORD};
pub use feature::{
    recompute, Feature, FeatureGraph, FeatureGraphError, FeatureId, ReferenceComponent,
    ReferenceGraph, SegmentRef, SegmentRefError, MONASTERY_NEIGHBOURS,
};
pub use placement::{
    check_placement, first_legal_placement, frontier, has_any_legal_placement, is_legal_placement,
    legal_placements, PlacementError,
};
pub use scoring::{awards, completed_value, incomplete_value, majority, MEEPLES_PER_SEAT};
pub use state::{
    turn_deadline, Command, Config, Event, State, StateError, Status, TurnDeadlineError, TurnPhase,
    View, ViewEvent, DEFAULT_ASYNC_TURN_DEADLINE_MS, MAX_SEATS, MIN_SEATS, MIN_TURN_DEADLINE_MS,
};
pub use tile::{
    Board, FeatureKind, PlacedTile, SegmentDef, Terrain, TileDef, TileKind, TileKindError,
    BAG_SIZE, MAX_SEGMENTS, START_TILE, TILE_SET,
};

use std::collections::{BTreeMap, BTreeSet};

use smallvec::{smallvec, SmallVec};
use tabula_core::{
    rng::domain, AbortReason, MatchOutcome, Millis, OutcomeKind, RuleError, RuleErrorCode,
    RulesVersion, SeatId, SeatRoster, Standing, TimerId, Viewer,
};
use tabula_game_api::{
    A11yDescription, AdminInput, CommandHint, Ctx, Effect, GameRules, Init, InitError, Input,
    LegalCommands, Outcome,
};

/// Game-scoped timer id for the per-turn deadline.
const TIMER_TURN: TimerId = TimerId(1);

/// RNG domain for the opening shuffle. Game domains start at
/// [`domain::GAME_BASE`]; the platform owns everything below it.
const DOMAIN_SHUFFLE: u32 = domain::GAME_BASE + 1;

/// The hint kind [`GameRules::legal_commands`] emits during placement.
pub const HINT_PLACE_TILE: &str = "place-tile";

/// The payload of a `place-tile` hint: one board square and the rotations that
/// are legal on it.
///
/// Encoded with `tabula_core::canonical_encode`, so a presenter or bot decodes
/// it with `canonical_decode` and nothing in between has to agree on a layout.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlaceTileHint {
    pub at: Coord,
    pub rotations: Vec<Rotation>,
}

#[derive(Debug)]
pub struct TilesRules;

impl GameRules for TilesRules {
    type State = State;
    type Command = Command;
    type Event = Event;
    type View = View;
    type ViewEvent = ViewEvent;
    type Config = Config;

    const RULES_VERSION: RulesVersion = RulesVersion(1);
    const RULES_HASH: [u8; 32] = *include_bytes!(concat!(env!("OUT_DIR"), "/rules_hash.bin"));

    fn create(
        cfg: &Config,
        roster: &SeatRoster,
        ctx: &mut Ctx<'_>,
    ) -> Result<Init<Self>, InitError> {
        let seats: Vec<SeatId> = roster.iter().map(|entry| entry.seat).collect();
        let seat_count = u8::try_from(seats.len()).unwrap_or(u8::MAX);
        if !(MIN_SEATS..=MAX_SEATS).contains(&seat_count) {
            return Err(InitError::SeatCount {
                got: seat_count,
                allowed: "2..=5".into(),
            });
        }
        let deadline =
            turn_deadline(cfg).map_err(|_| InitError::Config("turn_deadline_ms".into()))?;

        // The one and only use of randomness in this game.
        let mut bag = fresh_bag();
        ctx.rng.stream(DOMAIN_SHUFFLE).shuffle(&mut bag);

        let mut board = Board::new();
        board.insert(Coord::ORIGIN, PlacedTile::new(START_TILE, Rotation::R0));
        let mut features = FeatureGraph::new();
        features.place_tile(&board, Coord::ORIGIN);

        // A playing match always holds a drawn tile, so the opening draw
        // happens before the state exists rather than leaving it briefly
        // invalid. Every tile in the set is placeable next to the start tile
        // (`placement::tests`), so this loop discards nothing in practice — it
        // runs anyway so there is one draw rule and not two.
        let mut events: SmallVec<[Event; 4]> = smallvec![];
        let mut discarded = Vec::new();
        let drawn = draw_placeable(&board, &mut bag, &mut discarded, &mut events);
        let status = if drawn.is_some() {
            Status::Playing
        } else {
            Status::Ended
        };

        let meeples = seats.iter().map(|seat| (*seat, MEEPLES_PER_SEAT)).collect();
        let scores = seats.iter().map(|seat| (*seat, 0)).collect();

        let mut state = State::from_parts(state::StateParts {
            board,
            bag,
            drawn,
            discarded,
            features,
            meeples,
            scores,
            seats,
            turn_index: 0,
            phase: TurnPhase::PlaceTile,
            status,
            paused: false,
            turn_deadline_ms: deadline.unwrap_or(0),
            last_placed: None,
        })
        .map_err(|_| InitError::Config("roster".into()))?;

        let mut effects: SmallVec<[Effect; 4]> = smallvec![];
        if drawn.is_some() {
            arm_turn_timer(&state, &mut effects);
        } else {
            // Only reachable with a doctored tile set; ending immediately is
            // the total answer rather than a panic.
            state.status = Status::Playing;
            finish_match(&mut state, &mut events, &mut effects);
        }

        Ok(Init {
            state,
            events,
            effects,
        })
    }

    fn apply(
        state: &mut State,
        input: Input<Command>,
        _ctx: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        if state.status() != Status::Playing {
            return Err(RuleError::code(RuleErrorCode::MatchOver));
        }

        // `Input::Timer{..}` (stale) and `Input::Seat{..}` both return
        // `Outcome::empty()` for DIFFERENT reasons — see each arm. Merging them
        // would hide that the seat arm is a deliberate rules decision rather
        // than a fall-through.
        #[allow(clippy::match_same_arms)]
        match input {
            Input::Player { seat, .. } if !state.is_seated(seat) => {
                Err(RuleError::code(RuleErrorCode::NoSuchSeat))
            }

            Input::Player {
                seat,
                command: Command::PlaceTile { at, rotation },
            } => place_tile(state, seat, at, rotation),

            Input::Player {
                seat,
                command: Command::PlaceMeeple { segment },
            } => place_meeple(state, seat, segment),

            Input::Player {
                seat,
                command: Command::SkipMeeple,
            } => skip_meeple(state, seat),

            // The deadline expired. The rules resolve the turn themselves, in
            // canonical order, so live and async play differ only in how long
            // the platform waited before delivering this.
            Input::Timer { timer } if timer == TIMER_TURN => Ok(resolve_turn_on_deadline(state)),

            // A timer this version never set. Ignoring it is correct and total:
            // the runtime may deliver a stale one after a rules change.
            Input::Timer { .. } => Ok(Outcome::empty()),

            // `notify_rules = false`, so these should not arrive. Handling them
            // costs one line and satisfies R3.
            Input::Seat { .. } => Ok(Outcome::empty()),

            Input::Admin(AdminInput::Cancel { reason }) => Ok(abort(state, reason)),

            // `pausable = true`: the platform decides *whether* pausing is
            // allowed, this decides what it means. Tiles has no clock to
            // protect, so pausing simply stops accepting play and disarms the
            // deadline; resuming grants a full fresh deadline rather than the
            // remainder, which is the generous reading and the one an async
            // match wants.
            Input::Admin(AdminInput::Pause) => Ok(set_paused(state, true)),
            Input::Admin(AdminInput::Resume) => Ok(set_paused(state, false)),

            // Forcing a result would have to invent standings this game can
            // compute honestly from its own scores. Rejecting is the truthful
            // answer until there is a caller that needs it.
            Input::Admin(AdminInput::ForceEnd { .. }) => {
                Err(RuleError::code(RuleErrorCode::Unsupported))
            }
        }
    }

    /// The security boundary. Everything here is public except the one thing
    /// that is not present: the bag's order. `bag_remaining` is a count, and
    /// there is no field a refactor could widen back into a sequence.
    fn project(state: &State, viewer: Viewer) -> View {
        View {
            board: state.board().clone(),
            drawn: state.drawn(),
            discarded: state.discarded().to_vec(),
            bag_remaining: u16::try_from(state.bag_remaining()).unwrap_or(u16::MAX),
            seats: state.seats().to_vec(),
            turn: state.turn(),
            phase: state.phase(),
            status: state.status(),
            paused: state.paused(),
            scores: state.scores().clone(),
            meeples_in_hand: state.meeples_in_hand().clone(),
            followers: state.features().followers(),
            last_placed: state.last_placed(),
            meeple_slots: state.claimable_segments(),
            you: viewer.seat(),
        }
    }

    /// Every event is public. A drawn tile is revealed *because* it was drawn;
    /// the order it came from never appears in an event at all, so there is
    /// nothing to degrade and nothing whose existence must be hidden.
    fn view_event(_after: &State, event: &Event, _viewer: Viewer) -> Option<ViewEvent> {
        Some(event.clone())
    }

    fn legal_commands(state: &State, seat: SeatId) -> LegalCommands<Command> {
        if state.status() != Status::Playing || state.paused() || seat != state.turn() {
            return LegalCommands::None;
        }

        // The variant is chosen per *state*, not per game. In the claim step
        // there are at most five commands, so enumerating them is both cheap
        // and strictly more useful than a hint.
        if state.phase() == TurnPhase::PlaceMeeple {
            let mut commands: Vec<Command> = state
                .claimable_segments()
                .into_iter()
                .map(|segment| Command::PlaceMeeple { segment })
                .collect();
            commands.push(Command::SkipMeeple);
            return LegalCommands::Enumerated(commands);
        }

        let Some(kind) = state.drawn() else {
            return LegalCommands::None;
        };

        // `Hints`, not `Enumerated`: a mid-game board offers a few hundred
        // (square, rotation) pairs and a client only needs the squares to
        // highlight. One hint per square, carrying that square's rotations.
        LegalCommands::Hints(
            legal_placements(state.board(), kind)
                .into_iter()
                .filter_map(|(at, rotations)| {
                    let payload = PlaceTileHint { at, rotations };
                    tabula_core::canonical_encode(&payload)
                        .ok()
                        .map(|data| CommandHint::new(HINT_PLACE_TILE, data))
                })
                .collect(),
        )
    }

    fn describe(state: &State, viewer: Viewer) -> A11yDescription {
        // Phase 9 owns the full Board Reader with coordinate-relative
        // navigation (doc 04 §10.4). A status line is what Phase 3 can say
        // honestly, and it is better than `unsupported()`.
        let mut status = A11yDescription::unsupported();
        status.status = match state.status() {
            Status::Playing if state.paused() => "The match is paused.".to_owned(),
            Status::Playing if state.phase() == TurnPhase::PlaceMeeple => format!(
                "Seat {} may place a follower or pass. {} tiles remain in the bag.",
                state.turn().0,
                state.bag_remaining()
            ),
            Status::Playing => format!(
                "Seat {} to place. {} tiles remain in the bag.",
                state.turn().0,
                state.bag_remaining()
            ),
            Status::Ended => "The match is over.".to_owned(),
            Status::Aborted => "The match was cancelled.".to_owned(),
        };
        if viewer.seat() == Some(state.turn()) && state.status() == Status::Playing {
            status.status.push_str(" It is your turn.");
        }
        status
    }
}

// ---------------------------------------------------------------------------
// Bag and draw
// ---------------------------------------------------------------------------

/// The unshuffled bag: `count` copies of each kind, in tile-set order.
fn fresh_bag() -> Vec<TileKind> {
    let mut bag = Vec::with_capacity(BAG_SIZE);
    for (kind, def) in TileKind::all().zip(TILE_SET.iter()) {
        for _ in 0..def.count {
            bag.push(kind);
        }
    }
    bag
}

/// Draw until a placeable tile is found, discarding the rest.
///
/// This is the rule that keeps a match from deadlocking: a tile nobody can play
/// is set aside publicly, exactly as at a physical table.
///
/// It takes the three fields it moves tiles between rather than a `&mut State`
/// so that `create` — which must have a drawn tile in hand *before* it can
/// build a valid `State` at all — runs the same code as `end_turn` instead of a
/// second copy of it.
fn draw_placeable(
    board: &Board,
    bag: &mut Vec<TileKind>,
    discarded: &mut Vec<TileKind>,
    events: &mut SmallVec<[Event; 4]>,
) -> Option<TileKind> {
    while let Some(kind) = bag.pop() {
        if has_any_legal_placement(board, kind) {
            events.push(Event::TileDrawn { kind });
            return Some(kind);
        }
        discarded.push(kind);
        events.push(Event::TileDiscarded { kind });
    }
    None
}

/// [`draw_placeable`] applied to a live state.
fn draw_until_placeable(state: &mut State, events: &mut SmallVec<[Event; 4]>) -> bool {
    let drawn = draw_placeable(&state.board, &mut state.bag, &mut state.discarded, events);
    state.drawn = drawn;
    drawn.is_some()
}

fn arm_turn_timer(state: &State, effects: &mut SmallVec<[Effect; 4]>) {
    if state.turn_deadline_ms > 0 && !state.paused() {
        effects.push(Effect::SetTimer {
            id: TIMER_TURN,
            delay: Millis(state.turn_deadline_ms),
        });
    }
}

// ---------------------------------------------------------------------------
// Placing a tile
// ---------------------------------------------------------------------------

/// Proof that a placement was checked. Only [`validate_place`] builds one, and
/// [`commit_place`] is the only thing that consumes one — so no code path can
/// mutate the board without the check having run (contract R2).
#[derive(Clone, Copy)]
struct PlacementProof {
    at: Coord,
    tile: PlacedTile,
}

fn place_tile(
    state: &mut State,
    seat: SeatId,
    at: Coord,
    rotation: Rotation,
) -> Result<Outcome<TilesRules>, RuleError> {
    let proof = validate_place(state, seat, at, rotation)?;
    Ok(commit_place(state, seat, proof))
}

/// The pure half. Nothing above the first mutation may be skipped, and nothing
/// here mutates.
fn validate_place(
    state: &State,
    seat: SeatId,
    at: Coord,
    rotation: Rotation,
) -> Result<PlacementProof, RuleError> {
    if state.paused() {
        return Err(RuleError::code(RuleErrorCode::WrongPhase));
    }
    if seat != state.turn() {
        return Err(RuleError::code(RuleErrorCode::NotYourTurn));
    }
    let Some(kind) = state.drawn() else {
        return Err(RuleError::code(RuleErrorCode::WrongPhase));
    };
    let tile = PlacedTile::new(kind, rotation);
    check_placement(state.board(), at, tile)
        .map_err(|_| RuleError::code(RuleErrorCode::IllegalMove))?;
    Ok(PlacementProof { at, tile })
}

fn commit_place(state: &mut State, seat: SeatId, proof: PlacementProof) -> Outcome<TilesRules> {
    let PlacementProof { at, tile } = proof;

    state.board.insert(at, tile);
    state.features.place_tile(&state.board, at);
    state.drawn = None;
    state.last_placed = Some(at);
    state.phase = TurnPhase::PlaceMeeple;

    let mut events: SmallVec<[Event; 4]> = smallvec![Event::TilePlaced {
        seat,
        at,
        kind: tile.kind,
        rotation: tile.rotation,
    }];
    let mut effects: SmallVec<[Effect; 2]> = smallvec![];

    // Scoring waits for the claim: a follower placed on a feature that *this*
    // tile completes still scores it. Offering the claim only when there is
    // something to claim keeps the client from having to render a decision
    // with no options.
    if state.claimable_segments().is_empty() {
        finish_turn(state, &mut events, &mut effects);
    }

    Outcome { events, effects }
}

// ---------------------------------------------------------------------------
// Claiming a feature
// ---------------------------------------------------------------------------

/// Proof that a claim was checked, carrying the resolved segment so
/// [`commit_meeple`] cannot re-derive it differently.
#[derive(Clone, Copy)]
struct ClaimProof {
    at: SegmentRef,
}

fn place_meeple(
    state: &mut State,
    seat: SeatId,
    segment: u8,
) -> Result<Outcome<TilesRules>, RuleError> {
    let proof = validate_meeple(state, seat, segment)?;
    Ok(commit_meeple(state, seat, proof))
}

fn validate_meeple(state: &State, seat: SeatId, segment: u8) -> Result<ClaimProof, RuleError> {
    if state.paused() || state.phase() != TurnPhase::PlaceMeeple {
        return Err(RuleError::code(RuleErrorCode::WrongPhase));
    }
    if seat != state.turn() {
        return Err(RuleError::code(RuleErrorCode::NotYourTurn));
    }
    // `claimable_segments` is the single authority: it already accounts for
    // the phase, the seat's remaining followers, whether the tile has such a
    // segment, and whether the feature is unclaimed and unscored. Asking it
    // rather than re-deriving those four conditions is what keeps this
    // agreeing with `legal_commands` and with the client's own hints.
    if !state.claimable_segments().contains(&segment) {
        return Err(RuleError::code(RuleErrorCode::IllegalMove));
    }
    let coord = state
        .last_placed()
        .ok_or_else(|| RuleError::code(RuleErrorCode::WrongPhase))?;
    let at =
        SegmentRef::new(coord, segment).map_err(|_| RuleError::code(RuleErrorCode::IllegalMove))?;
    Ok(ClaimProof { at })
}

fn commit_meeple(state: &mut State, seat: SeatId, proof: ClaimProof) -> Outcome<TilesRules> {
    let placed = state.features.place_follower(proof.at, seat);
    debug_assert!(placed, "validate_meeple proved this claim is legal");
    if placed {
        if let Some(in_hand) = state.meeples.get_mut(&seat) {
            *in_hand = in_hand.saturating_sub(1);
        }
    }

    let mut events: SmallVec<[Event; 4]> = smallvec![Event::MeeplePlaced { seat, at: proof.at }];
    let mut effects: SmallVec<[Effect; 2]> = smallvec![];
    finish_turn(state, &mut events, &mut effects);
    Outcome { events, effects }
}

fn skip_meeple(state: &mut State, seat: SeatId) -> Result<Outcome<TilesRules>, RuleError> {
    if state.paused() || state.phase() != TurnPhase::PlaceMeeple {
        return Err(RuleError::code(RuleErrorCode::WrongPhase));
    }
    if seat != state.turn() {
        return Err(RuleError::code(RuleErrorCode::NotYourTurn));
    }
    let mut events: SmallVec<[Event; 4]> = smallvec![Event::MeepleSkipped { seat }];
    let mut effects: SmallVec<[Effect; 2]> = smallvec![];
    finish_turn(state, &mut events, &mut effects);
    Ok(Outcome { events, effects })
}

// ---------------------------------------------------------------------------
// Scoring and finishing a turn
// ---------------------------------------------------------------------------

/// Every component the last placement could have completed.
///
/// Recomputed from `last_placed` rather than carried in `State` from the
/// placement: the answer is a function of the board and the graph, so storing
/// it would be a second authority that could disagree with them. It is also
/// exactly the incremental win — the components of the placed tile's own
/// segments, plus any monastery whose ninth square this tile just filled, and
/// never a board sweep.
fn features_touched_by_last_placement(state: &State) -> BTreeSet<FeatureId> {
    let mut touched = BTreeSet::new();
    let Some(coord) = state.last_placed() else {
        return touched;
    };
    let Some(tile) = state.board().get(coord) else {
        return touched;
    };
    for index in 0..tile.segment_count() {
        if let Ok(segment) = SegmentRef::new(coord, index) {
            touched.extend(state.features().of(segment));
        }
    }
    for neighbour_coord in coord.surrounding() {
        let Some(neighbour) = state.board().get(neighbour_coord) else {
            continue;
        };
        for index in 0..neighbour.segment_count() {
            if neighbour.segment(index).map(|def| def.kind) != Some(FeatureKind::Monastery) {
                continue;
            }
            if let Ok(segment) = SegmentRef::new(neighbour_coord, index) {
                touched.extend(state.features().of(segment));
            }
        }
    }
    touched
}

/// Score whatever the last placement completed, then hand the turn on.
fn finish_turn(
    state: &mut State,
    events: &mut SmallVec<[Event; 4]>,
    effects: &mut SmallVec<[Effect; 2]>,
) {
    score_completed_features(state, events);
    state.phase = TurnPhase::PlaceTile;
    end_turn(state, events, effects);
}

/// Retire and pay out every completed component the last placement touched.
///
/// Ascending by feature id, so the award order is canonical (I-2). `retire`
/// is what makes "exactly once" true: a component that has already been
/// retired pays nothing and returns nothing however often it is revisited.
fn score_completed_features(state: &mut State, events: &mut SmallVec<[Event; 4]>) {
    for id in features_touched_by_last_placement(state) {
        let Some(feature) = state.features().feature(id) else {
            continue;
        };
        if feature.scored() || !feature.is_complete() {
            continue;
        }
        let kind = feature.kind();
        let tiles = u16::try_from(feature.tiles().len()).unwrap_or(u16::MAX);
        let awarded = scoring::awards(feature, state.board(), true);
        let returned = state.features.retire(id);

        for (seat, points) in &awarded {
            if let Some(score) = state.scores.get_mut(seat) {
                *score = score.saturating_add(*points);
            }
        }
        let returned: Vec<(SeatId, u8)> = returned
            .into_iter()
            .map(|(seat, count)| {
                let count = u8::try_from(count).unwrap_or(u8::MAX);
                if let Some(in_hand) = state.meeples.get_mut(&seat) {
                    *in_hand = in_hand.saturating_add(count);
                }
                (seat, count)
            })
            .collect();

        events.push(Event::FeatureScored {
            feature: id,
            kind,
            tiles,
            awarded,
            returned,
        });
    }
}

/// Hand the turn on, draw for the next seat, and end the match if the bag is
/// exhausted. Every path that finishes a turn goes through here.
fn end_turn(
    state: &mut State,
    events: &mut SmallVec<[Event; 4]>,
    effects: &mut SmallVec<[Effect; 2]>,
) {
    state.advance_turn();
    let mut drawn_events: SmallVec<[Event; 4]> = smallvec![];
    let drew = draw_until_placeable(state, &mut drawn_events);
    events.extend(drawn_events);

    if drew {
        effects.push(Effect::CancelTimer { id: TIMER_TURN });
        let mut armed: SmallVec<[Effect; 4]> = smallvec![];
        arm_turn_timer(state, &mut armed);
        effects.extend(armed);
    } else {
        let mut end_effects: SmallVec<[Effect; 4]> = smallvec![];
        finish_match(state, events, &mut end_effects);
        effects.extend(end_effects);
    }
}

// ---------------------------------------------------------------------------
// Deadlines, pausing, and terminal states
// ---------------------------------------------------------------------------

/// The turn deadline fired. Resolve the turn the rules' own way.
///
/// From the placement step: put the drawn tile on the **first legal square in
/// canonical order**, at that square's lowest legal rotation, and decline the
/// claim. From the claim step: decline the claim. Both are rules any observer
/// can reproduce and neither can be steered by the seat that timed out —
/// skipping the turn instead would reward running the clock down.
fn resolve_turn_on_deadline(state: &mut State) -> Outcome<TilesRules> {
    if state.paused() {
        return Outcome::empty();
    }
    let seat = state.turn();
    let mut events: SmallVec<[Event; 4]> = smallvec![Event::TurnAutoResolved { seat }];
    let mut effects: SmallVec<[Effect; 2]> = smallvec![];

    if let Some(kind) = state.drawn() {
        if let Some((at, rotation)) = first_legal_placement(state.board(), kind) {
            let tile = PlacedTile::new(kind, rotation);
            state.board.insert(at, tile);
            state.features.place_tile(&state.board, at);
            state.drawn = None;
            state.last_placed = Some(at);
            state.phase = TurnPhase::PlaceMeeple;
            events.push(Event::TilePlaced {
                seat,
                at,
                kind,
                rotation,
            });
        }
    }

    finish_turn(state, &mut events, &mut effects);
    Outcome { events, effects }
}

fn set_paused(state: &mut State, paused: bool) -> Outcome<TilesRules> {
    if state.paused == paused {
        return Outcome::empty();
    }
    state.paused = paused;
    let mut effects: SmallVec<[Effect; 2]> = smallvec![];
    if paused {
        effects.push(Effect::CancelTimer { id: TIMER_TURN });
    } else {
        let mut armed: SmallVec<[Effect; 4]> = smallvec![];
        arm_turn_timer(state, &mut armed);
        effects.extend(armed);
    }
    Outcome {
        events: smallvec![if paused {
            Event::Paused
        } else {
            Event::Resumed
        }],
        effects,
    }
}

/// The bag is empty: score what is unfinished and end the match.
fn finish_match(
    state: &mut State,
    events: &mut SmallVec<[Event; 4]>,
    effects: &mut SmallVec<[Effect; 4]>,
) {
    score_unfinished_features(state, events);
    state.status = Status::Ended;
    state.drawn = None;
    state.phase = TurnPhase::PlaceTile;
    state.paused = false;
    let outcome = outcome_from_scores(state);
    events.push(Event::Ended {
        outcome: outcome.clone(),
    });
    effects.push(Effect::CancelTimer { id: TIMER_TURN });
    effects.push(Effect::EndMatch { outcome });
}

/// Standings from the final scores: seats sorted by score descending, ties
/// sharing a rank. Every seat appears exactly once, which
/// `MatchOutcome::new_for_seats` checks and the testkit asserts.
fn outcome_from_scores(state: &State) -> MatchOutcome {
    let mut ranked: Vec<(SeatId, i64)> = state
        .seats()
        .iter()
        .map(|seat| (*seat, score_of(state, *seat)))
        .collect();
    // Descending score, then ascending seat: total and deterministic.
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0 .0.cmp(&right.0 .0)));

    // Dense ranking, not competition ranking: `MatchOutcome` requires ranks to
    // start at 0 with no gaps (`NonContiguousRanks`), so a tie for first is
    // followed by rank 1, never by rank 2.
    let mut standings: SmallVec<[Standing; 8]> = SmallVec::new();
    let mut rank = 0u8;
    for (index, (seat, score)) in ranked.iter().enumerate() {
        if index > 0 && ranked[index - 1].1 != *score {
            rank = rank.saturating_add(1);
        }
        standings.push(Standing {
            seat: *seat,
            rank,
            score: *score,
        });
    }

    let distinct_top = ranked
        .iter()
        .filter(|(_, score)| *score == ranked[0].1)
        .count();
    let kind = if distinct_top == 1 {
        OutcomeKind::Decisive
    } else {
        OutcomeKind::Draw
    };

    MatchOutcome::new_for_seats(kind, standings, "bag exhausted".into(), state.seats())
        .expect("standings cover every distinct seat exactly once")
}

/// End-of-game scoring: every component still unfinished pays its partial
/// value to whoever holds the majority on it.
///
/// One aggregate `FinalScored` event rather than one per component: at the end
/// of a match there can be dozens, and the per-input event budget is a real
/// constraint. The per-component detail is still in `State` for anyone who
/// wants to walk it.
fn score_unfinished_features(state: &mut State, events: &mut SmallVec<[Event; 4]>) {
    let mut totals: BTreeMap<SeatId, i64> = BTreeMap::new();
    let pending: Vec<FeatureId> = state
        .features()
        .iter()
        .filter(|(_, feature)| !feature.scored())
        .map(|(id, _)| id)
        .collect();

    for id in pending {
        let Some(feature) = state.features().feature(id) else {
            continue;
        };
        for (seat, points) in scoring::awards(feature, state.board(), false) {
            *totals.entry(seat).or_default() += points;
        }
        // Retire it even when it pays nothing, so nothing can be counted a
        // second time and no follower is left attached to a scored feature.
        for (seat, count) in state.features.retire(id) {
            if let Some(in_hand) = state.meeples.get_mut(&seat) {
                *in_hand = in_hand.saturating_add(u8::try_from(count).unwrap_or(u8::MAX));
            }
        }
    }

    for (seat, points) in &totals {
        if let Some(score) = state.scores.get_mut(seat) {
            *score = score.saturating_add(*points);
        }
    }
    if !totals.is_empty() {
        events.push(Event::FinalScored {
            awarded: totals.into_iter().collect(),
        });
    }
}

fn score_of(state: &State, seat: SeatId) -> i64 {
    state.scores().get(&seat).copied().unwrap_or(0)
}

fn abort(state: &mut State, reason: AbortReason) -> Outcome<TilesRules> {
    state.status = Status::Aborted;
    state.drawn = None;
    state.phase = TurnPhase::PlaceTile;
    state.paused = false;
    let outcome = MatchOutcome::new_for_seats(
        OutcomeKind::Aborted { reason },
        smallvec![],
        "cancelled".into(),
        state.seats(),
    )
    .expect("an empty aborted outcome is structurally valid");
    Outcome {
        events: smallvec![Event::Ended {
            outcome: outcome.clone()
        }],
        effects: smallvec![
            Effect::CancelTimer { id: TIMER_TURN },
            Effect::EndMatch { outcome }
        ],
    }
}
