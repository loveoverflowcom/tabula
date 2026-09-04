//! `State`, `Command`, `Event`, `View`, `ViewEvent`, `Config`, and the one
//! validator every one of them passes through. (doc 02 §10.2)
//!
//! # The validator is the load-bearing part
//!
//! `State` derives `Deserialize` through [`RawState`], so **there is no way to
//! obtain a `State` that has not been checked** — not from the wire, not from a
//! snapshot, not from a test fixture. Everything the rules assume without
//! re-checking is asserted once, here: the board is connected and internally
//! consistent, no tile has been conjured or lost, and the turn belongs to a
//! seat that exists.
//!
//! That single function is also the invariant oracle the property tests use
//! after every accepted input, which is why it is worth its length.
//!
//! @ai.role domain-types
//! @ai.domain tiles.rules.state
//! @ai.pure true
//! @ai.invariant board-is-connected-and-edge-consistent
//! @ai.invariant tiles-are-conserved-across-bag-board-drawn-and-discards
//! @ai.invariant turn-belongs-to-a-seated-player
//! @ai.evidence tests::a_state_that_lost_a_tile_is_rejected
//! @ai.evidence tests::a_disconnected_board_is_rejected
//! @ai.evidence tests::an_edge_inconsistent_board_is_rejected

#![allow(clippy::doc_markdown)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use tabula_core::{MatchOutcome, SeatId};

use super::coord::{Coord, Rotation};
use super::tile::{Board, TileKind, START_TILE, TILE_SET};

/// The fewest seats a match may have.
pub const MIN_SEATS: u8 = 2;
/// The most seats a match may have.
pub const MAX_SEATS: u8 = 5;

/// Floor on a configured per-turn deadline. `0` selects "no deadline"; every
/// other value below this is rejected at both creation boundaries.
pub const MIN_TURN_DEADLINE_MS: u64 = 5_000;

/// The deadline the *capability* advertises for async play. Nothing in `apply`
/// reads it: it is a lobby-facing default, and the rules take whatever
/// [`Config::turn_deadline_ms`] says.
pub const DEFAULT_ASYNC_TURN_DEADLINE_MS: u64 = 24 * 60 * 60 * 1_000;

/// Where a match is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Playing,
    /// The bag ran out and final scoring is done.
    Ended,
    /// An operator cancelled it.
    Aborted,
}

/// Lobby-chosen options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Milliseconds a seat has to complete its turn. `0` means no deadline,
    /// which is what local hot-seat play uses. Live (60 s) and async (24 h)
    /// differ only in this number — `apply` reads `ctx.now` either way.
    pub turn_deadline_ms: u64,
}

/// Why a configured deadline is not usable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("a nonzero turn deadline must be at least {MIN_TURN_DEADLINE_MS} ms")]
pub struct TurnDeadlineError;

/// Resolve a configured deadline, or reject it.
///
/// Both boundaries that can refuse a config — `GameModule::validate_config` at
/// match creation and `GameRules::create` — call this one function, so the
/// lobby and the rules cannot disagree about which values are acceptable.
///
/// # Errors
/// [`TurnDeadlineError`] for a nonzero value below [`MIN_TURN_DEADLINE_MS`].
pub fn turn_deadline(cfg: &Config) -> Result<Option<u64>, TurnDeadlineError> {
    match cfg.turn_deadline_ms {
        0 => Ok(None),
        value if value >= MIN_TURN_DEADLINE_MS => Ok(Some(value)),
        _ => Err(TurnDeadlineError),
    }
}

/// Player intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// Put the drawn tile on the board.
    PlaceTile { at: Coord, rotation: Rotation },
}

/// What happened, canonically.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// A tile left the bag and became public. The *order* it came from stays
    /// secret; this event reveals one tile, not the sequence behind it.
    TileDrawn {
        kind: TileKind,
    },
    /// A drawn tile had no legal square anywhere, so it was set aside and
    /// another drawn. Public — everyone at a real table sees this too.
    TileDiscarded {
        kind: TileKind,
    },
    TilePlaced {
        seat: SeatId,
        at: Coord,
        kind: TileKind,
        rotation: Rotation,
    },
    /// The turn deadline expired and the rules resolved the turn themselves.
    TurnAutoResolved {
        seat: SeatId,
    },
    Paused,
    Resumed,
    Ended {
        outcome: MatchOutcome,
    },
}

/// Nothing in [`Event`] is secret, so a viewer's event is the canonical one.
pub type ViewEvent = Event;

/// Canonical, full-information state. Server-only, never on the wire (I-5).
///
/// `bag` is the one secret field: its *order* determines every future draw. Its
/// length is public and travels in [`View::bag_remaining`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawState", into = "RawState")]
pub struct State {
    pub(crate) board: Board,
    /// SECRET ORDER. Public length. Draws take from the back, which is why
    /// `create` shuffles once and no later input consumes randomness at all.
    pub(crate) bag: Vec<TileKind>,
    /// Public from the moment it is drawn.
    pub(crate) drawn: Option<TileKind>,
    /// Tiles that had no legal square. Public.
    pub(crate) discarded: Vec<TileKind>,
    pub(crate) seats: Vec<SeatId>,
    pub(crate) turn_index: u8,
    pub(crate) status: Status,
    pub(crate) paused: bool,
    pub(crate) turn_deadline_ms: u64,
    pub(crate) last_placed: Option<Coord>,
}

/// The unvalidated shape. Every route into `State` goes through
/// [`State::from_parts`], including deserialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawState {
    board: Board,
    bag: Vec<TileKind>,
    drawn: Option<TileKind>,
    discarded: Vec<TileKind>,
    seats: Vec<SeatId>,
    turn_index: u8,
    status: Status,
    paused: bool,
    turn_deadline_ms: u64,
    last_placed: Option<Coord>,
}

/// A rejected snapshot is corrupt, not a legal alternative position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    #[error("a tiles match needs between {MIN_SEATS} and {MAX_SEATS} distinct seats")]
    SeatCount,
    #[error("tiles seats must be distinct")]
    DuplicateSeats,
    #[error("the seat on turn is not in the roster")]
    TurnOutsideRoster,
    #[error("the board must hold the start tile at the origin")]
    MissingStartTile,
    #[error("a placed tile does not match a neighbour's shared edge")]
    EdgeInconsistent,
    #[error("the board has tiles that do not touch the start tile")]
    Disconnected,
    #[error("tiles were created or destroyed between the bag, the board, and the discards")]
    TileConservation,
    #[error("a match in progress always has a drawn tile, and a finished one never does")]
    DrawnTileDisagreesWithStatus,
    #[error("only a match in progress can be paused")]
    PausedWhenNotPlaying,
    #[error("the last placed square is not on the board")]
    LastPlacedNotOnBoard,
    #[error("a nonzero turn deadline must be at least five seconds")]
    InvalidTurnDeadline,
}

/// The complete set of fields a [`State`] is built from.
///
/// A struct rather than ten positional arguments: the two call sites (`create`
/// and the deserializer) pass values whose types repeat — three `Vec`s, two
/// `Option`s, two integers — and a transposed pair would type-check.
#[derive(Clone, Debug)]
pub(crate) struct StateParts {
    pub board: Board,
    pub bag: Vec<TileKind>,
    pub drawn: Option<TileKind>,
    pub discarded: Vec<TileKind>,
    pub seats: Vec<SeatId>,
    pub turn_index: u8,
    pub status: Status,
    pub paused: bool,
    pub turn_deadline_ms: u64,
    pub last_placed: Option<Coord>,
}

impl State {
    /// The only constructor. `create` builds the opening position through it,
    /// and so does every decode.
    pub(crate) fn from_parts(parts: StateParts) -> Result<Self, StateError> {
        let StateParts {
            board,
            bag,
            drawn,
            discarded,
            seats,
            turn_index,
            status,
            paused,
            turn_deadline_ms,
            last_placed,
        } = parts;
        let seat_count = u8::try_from(seats.len()).map_err(|_| StateError::SeatCount)?;
        if !(MIN_SEATS..=MAX_SEATS).contains(&seat_count) {
            return Err(StateError::SeatCount);
        }
        if seats.iter().collect::<BTreeSet<_>>().len() != seats.len() {
            return Err(StateError::DuplicateSeats);
        }
        if usize::from(turn_index) >= seats.len() {
            return Err(StateError::TurnOutsideRoster);
        }
        if turn_deadline_ms != 0 && turn_deadline_ms < MIN_TURN_DEADLINE_MS {
            return Err(StateError::InvalidTurnDeadline);
        }
        if board.get(Coord::ORIGIN).map(|tile| tile.kind) != Some(START_TILE) {
            return Err(StateError::MissingStartTile);
        }
        if paused && status != Status::Playing {
            return Err(StateError::PausedWhenNotPlaying);
        }
        if drawn.is_some() != (status == Status::Playing) {
            return Err(StateError::DrawnTileDisagreesWithStatus);
        }
        if let Some(coord) = last_placed {
            if !board.contains(coord) {
                return Err(StateError::LastPlacedNotOnBoard);
            }
        }

        check_board_consistency(&board)?;
        check_tile_conservation(&board, &bag, drawn, &discarded)?;

        Ok(Self {
            board,
            bag,
            drawn,
            discarded,
            seats,
            turn_index,
            status,
            paused,
            turn_deadline_ms,
            last_placed,
        })
    }

    /// Re-runs the full structural validation on the current value.
    ///
    /// `apply` maintains these invariants incrementally, so this is not on the
    /// production path. It exists so property tests can assert them after every
    /// accepted input against the same oracle a decode uses — one definition of
    /// "well formed", not two.
    ///
    /// # Errors
    /// The first violated invariant.
    pub fn check_invariants(&self) -> Result<(), StateError> {
        check_board_consistency(&self.board)?;
        check_tile_conservation(&self.board, &self.bag, self.drawn, &self.discarded)?;
        if self.drawn.is_some() != (self.status == Status::Playing) {
            return Err(StateError::DrawnTileDisagreesWithStatus);
        }
        Ok(())
    }

    #[must_use]
    pub const fn board(&self) -> &Board {
        &self.board
    }

    #[must_use]
    pub const fn drawn(&self) -> Option<TileKind> {
        self.drawn
    }

    #[must_use]
    pub fn bag_remaining(&self) -> usize {
        self.bag.len()
    }

    #[must_use]
    pub fn discarded(&self) -> &[TileKind] {
        &self.discarded
    }

    #[must_use]
    pub fn seats(&self) -> &[SeatId] {
        &self.seats
    }

    #[must_use]
    pub fn turn(&self) -> SeatId {
        self.seats[usize::from(self.turn_index)]
    }

    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }

    #[must_use]
    pub const fn paused(&self) -> bool {
        self.paused
    }

    #[must_use]
    pub const fn last_placed(&self) -> Option<Coord> {
        self.last_placed
    }

    #[must_use]
    pub fn is_seated(&self, seat: SeatId) -> bool {
        self.seats.contains(&seat)
    }

    pub(crate) fn advance_turn(&mut self) {
        // The validator guarantees `2 <= seats.len() <= MAX_SEATS`.
        let count = u8::try_from(self.seats.len()).unwrap_or(MAX_SEATS);
        self.turn_index = (self.turn_index + 1) % count;
    }
}

/// Every shared edge agrees, and every tile is reachable from the start tile.
fn check_board_consistency(board: &Board) -> Result<(), StateError> {
    for (coord, tile) in board.iter() {
        for (side, neighbour_coord) in coord.orthogonal() {
            if let Some(neighbour) = board.get(neighbour_coord) {
                if tile.terrain(side) != neighbour.terrain(side.opposite()) {
                    return Err(StateError::EdgeInconsistent);
                }
            }
        }
    }

    let mut seen: BTreeSet<Coord> = BTreeSet::new();
    let mut queue: VecDeque<Coord> = VecDeque::new();
    seen.insert(Coord::ORIGIN);
    queue.push_back(Coord::ORIGIN);
    while let Some(coord) = queue.pop_front() {
        for (_, neighbour) in coord.orthogonal() {
            if board.contains(neighbour) && seen.insert(neighbour) {
                queue.push_back(neighbour);
            }
        }
    }
    if seen.len() == board.len() {
        Ok(())
    } else {
        Err(StateError::Disconnected)
    }
}

/// The bag, the drawn tile, the discards, and the board (less the start tile)
/// must together be exactly the bag the match began with.
///
/// This is the invariant that makes a lost or duplicated tile impossible to
/// miss: any mutation that forgets to remove a tile from one place while adding
/// it to another fails here.
fn check_tile_conservation(
    board: &Board,
    bag: &[TileKind],
    drawn: Option<TileKind>,
    discarded: &[TileKind],
) -> Result<(), StateError> {
    let mut counts: BTreeMap<TileKind, usize> = BTreeMap::new();
    for kind in bag.iter().chain(discarded.iter()).copied().chain(drawn) {
        *counts.entry(kind).or_default() += 1;
    }
    let mut start_seen = false;
    for (coord, tile) in board.iter() {
        if coord == Coord::ORIGIN && !start_seen {
            start_seen = true;
            continue;
        }
        *counts.entry(tile.kind).or_default() += 1;
    }

    for (kind, def) in TileKind::all().zip(TILE_SET.iter()) {
        if counts.remove(&kind).unwrap_or(0) != usize::from(def.count) {
            return Err(StateError::TileConservation);
        }
    }
    if counts.is_empty() {
        Ok(())
    } else {
        Err(StateError::TileConservation)
    }
}

impl TryFrom<RawState> for State {
    type Error = StateError;

    fn try_from(raw: RawState) -> Result<Self, Self::Error> {
        Self::from_parts(StateParts {
            board: raw.board,
            bag: raw.bag,
            drawn: raw.drawn,
            discarded: raw.discarded,
            seats: raw.seats,
            turn_index: raw.turn_index,
            status: raw.status,
            paused: raw.paused,
            turn_deadline_ms: raw.turn_deadline_ms,
            last_placed: raw.last_placed,
        })
    }
}

impl From<State> for RawState {
    fn from(state: State) -> Self {
        Self {
            board: state.board,
            bag: state.bag,
            drawn: state.drawn,
            discarded: state.discarded,
            seats: state.seats,
            turn_index: state.turn_index,
            status: state.status,
            paused: state.paused,
            turn_deadline_ms: state.turn_deadline_ms,
            last_placed: state.last_placed,
        }
    }
}

/// The per-viewer projection.
///
/// A separate type from [`State`], not `State` with fields blanked (doc 02
/// §7.1). The bag is represented by its **count**: there is no field here that
/// a careless refactor could fill with the order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// Public: every tile on the board, with its rotation.
    pub board: Board,
    /// Public once drawn.
    pub drawn: Option<TileKind>,
    /// Public: tiles set aside as unplaceable.
    pub discarded: Vec<TileKind>,
    /// Public: how many tiles are left. **Not** which, and not in what order.
    pub bag_remaining: u16,
    pub seats: Vec<SeatId>,
    pub turn: SeatId,
    pub status: Status,
    pub paused: bool,
    pub last_placed: Option<Coord>,
    /// Which seat is looking, if any. `None` for a spectator.
    pub you: Option<SeatId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::coord::Side;
    use crate::rules::placement::check_placement;
    use crate::rules::tile::{PlacedTile, BAG_SIZE};

    fn full_bag() -> Vec<TileKind> {
        let mut bag = Vec::new();
        for (kind, def) in TileKind::all().zip(TILE_SET.iter()) {
            for _ in 0..def.count {
                bag.push(kind);
            }
        }
        bag
    }

    fn opening_board() -> Board {
        let mut board = Board::new();
        board.insert(Coord::ORIGIN, PlacedTile::new(START_TILE, Rotation::R0));
        board
    }

    /// A valid opening position, as parts, so each test perturbs exactly the
    /// one field it is about and the reader sees the perturbation rather than
    /// ten positional arguments.
    fn opening_parts() -> StateParts {
        let mut bag = full_bag();
        let drawn = bag.pop();
        StateParts {
            board: opening_board(),
            bag,
            drawn,
            discarded: Vec::new(),
            seats: vec![SeatId(0), SeatId(1)],
            turn_index: 0,
            status: Status::Playing,
            paused: false,
            turn_deadline_ms: 0,
            last_placed: None,
        }
    }

    fn opening_state() -> State {
        State::from_parts(opening_parts()).expect("the opening position is valid")
    }

    #[test]
    fn the_opening_position_is_valid_and_self_consistent() {
        let state = opening_state();
        assert_eq!(state.bag_remaining(), BAG_SIZE - 1);
        assert!(state.drawn().is_some());
        assert_eq!(state.turn(), SeatId(0));
        assert_eq!(state.check_invariants(), Ok(()));
    }

    #[test]
    fn a_state_that_lost_a_tile_is_rejected() {
        let mut parts = opening_parts();
        parts.bag.pop(); // vanished
        assert_eq!(State::from_parts(parts), Err(StateError::TileConservation));
    }

    #[test]
    fn a_state_that_duplicated_a_tile_is_rejected() {
        let mut parts = opening_parts();
        parts.bag.push(parts.bag[0]); // conjured
        assert_eq!(State::from_parts(parts), Err(StateError::TileConservation));
    }

    #[test]
    fn a_disconnected_board_is_rejected() {
        let mut parts = opening_parts();
        let stray = parts.bag.pop().expect("the bag is not empty");
        parts.board.insert(
            Coord::new(5, 5).unwrap(),
            PlacedTile::new(stray, Rotation::R0),
        );
        assert_eq!(State::from_parts(parts), Err(StateError::Disconnected));
    }

    #[test]
    fn an_edge_inconsistent_board_is_rejected() {
        // `city-cap` unrotated shows Field to its south, but the start tile
        // shows City to its north, so this pair cannot be adjacent.
        let city_cap = TileKind::new(7).unwrap();
        let mut parts = opening_parts();
        parts.board.insert(
            Coord::ORIGIN.neighbour(Side::North).unwrap(),
            PlacedTile::new(city_cap, Rotation::R0),
        );
        let position = parts
            .bag
            .iter()
            .position(|kind| *kind == city_cap)
            .expect("the bag holds a city-cap");
        parts.bag.remove(position);
        assert_eq!(State::from_parts(parts), Err(StateError::EdgeInconsistent));
    }

    #[test]
    fn a_board_without_the_start_tile_at_the_origin_is_rejected() {
        let mut parts = opening_parts();
        parts.board = Board::new();
        assert_eq!(State::from_parts(parts), Err(StateError::MissingStartTile));
    }

    #[test]
    fn seat_and_turn_shapes_are_checked() {
        let build = |seats: Vec<SeatId>, turn_index: u8| {
            let mut parts = opening_parts();
            parts.seats = seats;
            parts.turn_index = turn_index;
            State::from_parts(parts)
        };
        assert_eq!(build(vec![SeatId(0)], 0), Err(StateError::SeatCount));
        assert_eq!(
            build((0..6).map(SeatId).collect(), 0),
            Err(StateError::SeatCount)
        );
        assert_eq!(
            build(vec![SeatId(3), SeatId(3)], 0),
            Err(StateError::DuplicateSeats)
        );
        assert_eq!(
            build(vec![SeatId(0), SeatId(1)], 2),
            Err(StateError::TurnOutsideRoster)
        );
        assert!(build((0..5).map(SeatId).collect(), 4).is_ok());
    }

    #[test]
    fn a_match_in_progress_always_holds_a_drawn_tile() {
        let mut without = opening_parts();
        without
            .bag
            .push(without.drawn.take().expect("a drawn tile"));
        assert_eq!(
            State::from_parts(without),
            Err(StateError::DrawnTileDisagreesWithStatus)
        );

        let mut ended_holding = opening_parts();
        ended_holding.status = Status::Ended;
        assert_eq!(
            State::from_parts(ended_holding),
            Err(StateError::DrawnTileDisagreesWithStatus)
        );
    }

    #[test]
    fn a_finished_match_cannot_be_paused_and_a_stray_last_square_is_rejected() {
        let mut paused_and_over = opening_parts();
        paused_and_over.status = Status::Aborted;
        paused_and_over.drawn = None;
        paused_and_over.paused = true;
        assert_eq!(
            State::from_parts(paused_and_over),
            Err(StateError::PausedWhenNotPlaying)
        );

        let mut stray = opening_parts();
        stray.last_placed = Some(Coord::new(9, 9).unwrap());
        assert_eq!(
            State::from_parts(stray),
            Err(StateError::LastPlacedNotOnBoard)
        );
    }

    #[test]
    fn deserialization_cannot_bypass_the_validator() {
        let state = opening_state();
        let bytes = tabula_core::canonical_encode(&state).unwrap();
        assert_eq!(
            tabula_core::canonical_decode::<State>(&bytes).unwrap(),
            state
        );

        // A raw shape that skipped every rule must not decode.
        let raw = RawState {
            board: Board::new(),
            bag: Vec::new(),
            drawn: None,
            discarded: Vec::new(),
            seats: vec![SeatId(0)],
            turn_index: 9,
            status: Status::Playing,
            paused: true,
            turn_deadline_ms: 3,
            last_placed: Some(Coord::new(9, 9).unwrap()),
        };
        let hostile = tabula_core::canonical_encode(&raw).unwrap();
        assert!(tabula_core::canonical_decode::<State>(&hostile).is_err());
    }

    #[test]
    fn turn_deadline_config_partitions() {
        assert_eq!(
            turn_deadline(&Config {
                turn_deadline_ms: 0
            }),
            Ok(None)
        );
        assert_eq!(
            turn_deadline(&Config {
                turn_deadline_ms: MIN_TURN_DEADLINE_MS
            }),
            Ok(Some(MIN_TURN_DEADLINE_MS))
        );
        assert_eq!(
            turn_deadline(&Config {
                turn_deadline_ms: MIN_TURN_DEADLINE_MS - 1
            }),
            Err(TurnDeadlineError)
        );

        let mut parts = opening_parts();
        parts.turn_deadline_ms = MIN_TURN_DEADLINE_MS - 1;
        assert_eq!(
            State::from_parts(parts),
            Err(StateError::InvalidTurnDeadline),
            "the state validator refuses what the config validator refuses"
        );
    }

    #[test]
    fn turn_order_wraps_through_every_seat_in_roster_order() {
        let mut parts = opening_parts();
        parts.seats = vec![SeatId(4), SeatId(0), SeatId(2)];
        let mut state = State::from_parts(parts).unwrap();
        let seen: Vec<SeatId> = (0..6)
            .map(|_| {
                let seat = state.turn();
                state.advance_turn();
                seat
            })
            .collect();
        assert_eq!(
            seen,
            vec![
                SeatId(4),
                SeatId(0),
                SeatId(2),
                SeatId(4),
                SeatId(0),
                SeatId(2)
            ]
        );
    }

    /// Silently accepting an unplaceable square would be the dangerous
    /// failure: `check_placement` is the only legality authority and the
    /// validator must agree with it on the boards it accepts.
    #[test]
    fn the_validator_and_the_placement_rule_agree_about_shared_edges() {
        let state = opening_state();
        for (coord, tile) in state.board().iter() {
            let mut without = Board::new();
            for (other, other_tile) in state.board().iter() {
                if other != coord {
                    without.insert(other, other_tile);
                }
            }
            if !without.is_empty() {
                assert!(check_placement(&without, coord, tile).is_ok());
            }
        }
    }
}
