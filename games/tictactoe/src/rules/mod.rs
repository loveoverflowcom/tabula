#![allow(clippy::doc_markdown)] // `@ai.*` schema values must remain bare machine-readable paths.

//! `impl GameRules for TicTacToeRules`. (doc 02 §10.2)
//!
//! This is the reference implementation of the contract. Read it before writing
//! any game; the structure — **validate fully, then mutate** — is the part to
//! copy, not the tic-tac-toe logic.
//!
//! @ai.role functional-core
//! @ai.domain tictactoe.rules
//! @ai.pure true
//! @ai.invariant rules-hash-excludes-noncanonical-feature-source
//! @ai.invariant canonical-rules-depend-only-on-rules-tree
//! @ai.invariant rejected-input-preserves-state
//! @ai.law canonical-rules-source-change-changes-rules-hash
//! @ai.evidence tests::rules_hash_matches_independent_rules_subtree_oracle
//! @ai.evidence tests::canonical_source_mutation_changes_oracle_hash
//! @ai.evidence tests::canonical_tree_rejects_noncanonical_feature_sources
//! @ai.evidence tests::canonical_rules_do_not_depend_on_crate_root_sources
//! @ai.evidence verification::rejected_initial_place_preserves_state
//! @ai.evidence verification::rejected_second_place_preserves_state

pub mod state;

pub use state::{Command, Config, Event, Mark, State, Status, View, ViewEvent};

use smallvec::smallvec;
use tabula_core::{
    AbortReason, MatchOutcome, Millis, OutcomeKind, RuleError, RuleErrorCode, RulesVersion, SeatId,
    SeatRoster, Standing, TimerId, Viewer,
};
use tabula_game_api::{
    A11yDescription, AdminInput, Ctx, Effect, GameRules, Init, InitError, Input, LegalCommands,
    Outcome,
};

#[derive(Debug)]
pub struct TicTacToeRules;

/// Timer ids are game-scoped; two games' ids never collide because they never
/// share a match.
const TIMER_MOVE: TimerId = TimerId(1);

/// Floor on the configurable move timeout. A zero value selects this documented
/// default; every other value below it is rejected at both creation boundaries.
pub(crate) const MIN_MOVE_TIMEOUT: u64 = 5_000;

const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8], // rows
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8], // columns
    [0, 4, 8],
    [2, 4, 6], // diagonals
];

impl GameRules for TicTacToeRules {
    type State = State;
    type Command = Command;
    type Event = Event;
    type View = View;
    type ViewEvent = Event;
    type Config = Config;

    const RULES_VERSION: RulesVersion = RulesVersion(2);
    const RULES_HASH: [u8; 32] = *include_bytes!(concat!(env!("OUT_DIR"), "/rules_hash.bin"));

    fn create(
        cfg: &Config,
        roster: &SeatRoster,
        _ctx: &mut Ctx<'_>,
    ) -> Result<Init<Self>, InitError> {
        let [first, second] = roster.as_slice() else {
            return Err(InitError::SeatCount {
                got: u8::try_from(roster.len()).unwrap_or(u8::MAX),
                allowed: "2".into(),
            });
        };
        let timeout =
            move_timeout(cfg).map_err(|()| InitError::Config("move_timeout_ms".into()))?;

        Ok(Init {
            state: State::new([first.seat, second.seat], timeout)
                .map_err(|_| InitError::Config("roster".into()))?,
            events: smallvec![],
            effects: smallvec![Effect::SetTimer {
                id: TIMER_MOVE,
                delay: Millis(timeout),
            }],
        })
    }

    fn apply(
        state: &mut State,
        input: Input<Command>,
        _ctx: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        if !matches!(state.status, Status::Playing) {
            return Err(RuleError::code(RuleErrorCode::MatchOver));
        }

        // `Input::Timer{..}` and `Input::Seat{..}` both return `Outcome::empty()`,
        // but for DIFFERENT reasons — see the comments below. Merging them would
        // hide the fact that the seat arm is a deliberate rules decision ("the
        // clock keeps running") rather than a fall-through.
        #[allow(clippy::match_same_arms)]
        match input {
            Input::Player { seat, .. } if !state.is_seated(seat) => {
                Err(RuleError::code(RuleErrorCode::NoSuchSeat))
            }

            Input::Player {
                seat,
                command: Command::Place { cell },
            } => place(state, seat, cell),

            Input::Player {
                seat,
                command: Command::Resign,
            } => Ok(end_by_resign(state, seat)),

            // The platform fired our timer: whoever is on turn loses on time.
            // Note we never asked what time it is — the runtime did the waiting.
            Input::Timer { timer } if timer == TIMER_MOVE => Ok(end_by_resign(state, state.turn)),

            // A timer we did not set. Not an error: the runtime may deliver a
            // stale timer after a rules change. Ignoring it is correct and total.
            Input::Timer { .. } => Ok(Outcome::empty()),

            // `notify_rules = false` in game.toml, so we should not see these.
            // Handling them anyway costs one line and satisfies R3.
            Input::Seat { .. } => Ok(Outcome::empty()),

            Input::Admin(AdminInput::Cancel { reason }) => Ok(end_aborted(state, reason)),

            // `pausable = false`, so Pause/Resume/ForceEnd are not ours to honour.
            // Rejecting is honest; silently accepting would desync the operator UI.
            Input::Admin(_) => Err(RuleError::code(RuleErrorCode::Unsupported)),
        }
    }

    fn project(state: &State, viewer: Viewer) -> View {
        View {
            board: *state.board(),
            turn: state.turn(),
            status: state.status(),
            you: viewer.seat(),
        }
    }

    fn view_event(_after: &State, event: &Event, _viewer: Viewer) -> Option<Event> {
        // Nothing is secret. Every viewer sees every event unchanged.
        Some(event.clone())
    }

    fn legal_commands(state: &State, seat: SeatId) -> LegalCommands<Command> {
        if seat != state.turn || !matches!(state.status, Status::Playing) {
            return LegalCommands::None;
        }
        // Full enumeration unlocks move highlighting, drag-drop legality, a free
        // `Trivial` bot, and self-play fuzzing — all from these five lines.
        LegalCommands::Enumerated(
            (0..9u8)
                .filter(|c| state.board[*c as usize].is_none())
                .map(|cell| Command::Place { cell })
                .collect(),
        )
    }

    fn describe(_state: &State, _viewer: Viewer) -> A11yDescription {
        // TODO(phase 5): "Your turn. X to move. Cells A1, B2 and C3 are free."
        // Keyboard play is mandatory (doc 04 §10.3), and a game without
        // `describe()` cannot be marked accessible in the catalog.
        A11yDescription::unsupported()
    }
}

/// The canonical validate-then-mutate shape. **Copy this structure.**
///
/// Everything that can reject happens above the first assignment to `state`. That
/// is contract R2 made structural rather than remembered.
///
/// @ai.role domain-transition
/// @ai.domain tictactoe.rules.place
/// @ai.pure true
/// @ai.invariant rejected-input-preserves-state
/// @ai.law validate-then-mutate
/// @ai.evidence verification::rejected_initial_place_preserves_state
/// @ai.evidence verification::rejected_second_place_preserves_state
fn place(state: &mut State, seat: SeatId, cell: u8) -> Result<Outcome<TicTacToeRules>, RuleError> {
    let idx = validate_place(state, seat, cell)?;

    Ok(commit_place(state, seat, cell, idx))
}

/// The pure validation half of [`place`]. Keeping this separate makes the R2
/// boundary visible to both readers and the proof harness.
fn validate_place(state: &State, seat: SeatId, cell: u8) -> Result<usize, RuleError> {
    if seat != state.turn {
        return Err(RuleError::code(RuleErrorCode::NotYourTurn));
    }
    let idx = cell as usize;
    if idx >= 9 || state.board[idx].is_some() {
        return Err(RuleError::code(RuleErrorCode::IllegalMove));
    }
    Ok(idx)
}

/// The mutation half of [`place`], called only after [`validate_place`] succeeds.
#[cfg_attr(kani, inline(never))]
fn commit_place(state: &mut State, seat: SeatId, cell: u8, idx: usize) -> Outcome<TicTacToeRules> {
    let mark = state.mark_for(seat);
    state.board[idx] = Some(mark);

    let mut events = smallvec![Event::Placed { seat, cell, mark }];
    let mut effects = smallvec![];

    if let Some(outcome) = check_end(state, seat) {
        state.status = match outcome.kind() {
            OutcomeKind::Decisive => Status::Won(seat),
            _ => Status::Drawn,
        };
        events.push(Event::Ended {
            outcome: outcome.clone(),
        });
        // Timers set and cancelled symmetrically — doc 02 §14 checklist.
        effects.push(Effect::CancelTimer { id: TIMER_MOVE });
        effects.push(Effect::EndMatch { outcome });
    } else {
        state.turn = state.other(state.turn);
        effects.push(Effect::SetTimer {
            id: TIMER_MOVE,
            delay: Millis(state.move_timeout_ms),
        });
    }

    Outcome { events, effects }
}

fn check_end(state: &State, mover: SeatId) -> Option<MatchOutcome> {
    let won = LINES.iter().any(|line| {
        let [a, b, c] = *line;
        state.board[a].is_some()
            && state.board[a] == state.board[b]
            && state.board[b] == state.board[c]
    });

    if won {
        return Some(decisive(state, mover));
    }
    if state.moves() >= 9 {
        return Some(drawn(state));
    }
    None
}

/// Standings must cover every seat exactly once, with contiguous ranks from 0.
/// The testkit asserts it (`outcome_wellformed`) because malformed standings
/// corrupt ratings silently.
fn decisive(state: &State, winner: SeatId) -> MatchOutcome {
    MatchOutcome::new_for_seats(
        OutcomeKind::Decisive,
        smallvec![
            Standing {
                seat: winner,
                rank: 0,
                score: 1
            },
            Standing {
                seat: state.other(winner),
                rank: 1,
                score: 0
            },
        ],
        "three in a row".into(),
        &state.seats,
    )
    .expect("two distinct state seats always form a valid decisive outcome")
}

fn drawn(state: &State) -> MatchOutcome {
    MatchOutcome::new_for_seats(
        OutcomeKind::Draw,
        // Ties share a rank.
        smallvec![
            Standing {
                seat: state.seats[0],
                rank: 0,
                score: 0
            },
            Standing {
                seat: state.seats[1],
                rank: 0,
                score: 0
            },
        ],
        "board full".into(),
        &state.seats,
    )
    .expect("two distinct state seats always form a valid drawn outcome")
}

fn end_by_resign(state: &mut State, resigning: SeatId) -> Outcome<TicTacToeRules> {
    let winner = state.other(resigning);
    state.status = Status::Forfeited(winner);
    let outcome = decisive(state, winner);
    Outcome {
        events: smallvec![Event::Ended {
            outcome: outcome.clone()
        }],
        effects: smallvec![
            Effect::CancelTimer { id: TIMER_MOVE },
            Effect::EndMatch { outcome }
        ],
    }
}

fn end_aborted(state: &mut State, reason: AbortReason) -> Outcome<TicTacToeRules> {
    state.status = Status::Aborted;
    let outcome = MatchOutcome::new_for_seats(
        OutcomeKind::Aborted { reason },
        smallvec![],
        "cancelled".into(),
        &state.seats,
    )
    .expect("empty aborted outcome is structurally valid");
    Outcome {
        events: smallvec![Event::Ended {
            outcome: outcome.clone()
        }],
        effects: smallvec![
            Effect::CancelTimer { id: TIMER_MOVE },
            Effect::EndMatch { outcome }
        ],
    }
}

pub(crate) fn move_timeout(cfg: &Config) -> Result<u64, ()> {
    match cfg.move_timeout_ms {
        0 => Ok(MIN_MOVE_TIMEOUT),
        value if value >= MIN_MOVE_TIMEOUT => Ok(value),
        _ => Err(()),
    }
}

#[cfg(kani)]
mod verification {
    use super::{place, Mark, State, TicTacToeRules};
    use tabula_core::SeatId;
    use tabula_game_api::Outcome;

    /// Compare every field that participates in the canonical TicTacToe state.
    /// This proof intentionally establishes field equality, not serialized-byte
    /// equality. The serde representation is the fixed declaration-order
    /// encoding of these five fields; there are no ignored or derived canonical
    /// fields, so equal fields imply equal current canonical bytes. The proof
    /// deliberately avoids invoking the codec; an explicit encoding proof
    /// remains future work.
    fn canonical_state_fields_equal(before: &State, after: &State) -> bool {
        before.board == after.board
            && before.seats == after.seats
            && before.turn == after.turn
            && before.status == after.status
            && before.move_timeout_ms == after.move_timeout_ms
    }

    fn initial_state() -> State {
        State::new([SeatId(7), SeatId(42)], 5_000)
            .expect("the proof fixture is a valid initial state")
    }

    /// The transactional proofs only exercise `place`'s rejection path. This
    /// verifier-only replacement makes reaching the outcome-building mutation
    /// path an immediate counterexample instead of asking CBMC to model its
    /// unrelated `SmallVec` drops. The second harness uses the same production
    /// `place` call for its known-valid prefix; this tiny replacement models
    /// only that fixed prefix and rejects every other commit attempt.
    #[allow(dead_code)]
    fn commit_place_verification_stub(
        state: &mut State,
        seat: SeatId,
        cell: u8,
        idx: usize,
    ) -> Outcome<TicTacToeRules> {
        if state.board == [None; 9]
            && state.seats == [SeatId(7), SeatId(42)]
            && state.turn == SeatId(7)
            && state.status == super::Status::Playing
            && state.move_timeout_ms == 5_000
            && seat == SeatId(7)
            && cell == 0
            && idx == 0
        {
            state.board[0] = Some(Mark::X);
            state.turn = SeatId(42);
            return Outcome::empty();
        }
        panic!("a rejected placement must not reach commit_place")
    }

    #[kani::proof]
    fn concrete_opening_place_is_accepted() {
        let mut state = initial_state();
        let opening = place(&mut state, SeatId(7), 0);
        assert!(opening.is_ok());
        assert!(state.board[0] == Some(Mark::X));
        assert!(state.turn == SeatId(42));
        core::mem::forget(opening);
    }

    /// For every raw seat and cell, a rejected placement against the canonical
    /// initial state leaves all canonical fields unchanged. Symbolic execution
    /// naturally covers wrong-player, unknown-seat, and out-of-range-cell
    /// rejections without assuming any failure class away.
    #[kani::proof]
    #[kani::stub(super::commit_place, commit_place_verification_stub)]
    fn rejected_initial_place_preserves_state() {
        let mut state = initial_state();
        let before = state.clone();
        let seat = SeatId(kani::any());
        let cell: u8 = kani::any();
        let should_reject = seat != state.turn || cell >= 9;

        if should_reject {
            let result = place(&mut state, seat, cell);
            let rejected = result.is_err();
            let unchanged = canonical_state_fields_equal(&before, &state);
            // The proof concerns the state mutation, not `Outcome` destruction.
            // Avoid making CBMC unwind SmallVec's unrelated drop implementation.
            core::mem::forget(result);

            assert!(rejected);
            assert!(unchanged);
        }
    }

    /// After the known-valid opening move at cell zero, every raw second
    /// placement that is rejected leaves all canonical fields unchanged. The
    /// model covers the occupied cell, wrong turn, unknown seat, and out-of-range
    /// cell classes reachable after this one-move prefix; it does not claim R2
    /// for every arbitrary reachable TicTacToe position.
    #[kani::proof]
    #[kani::stub(super::commit_place, commit_place_verification_stub)]
    fn rejected_second_place_preserves_state() {
        let mut state = initial_state();
        let opening = place(&mut state, SeatId(7), 0);
        let opening_was_accepted = opening.is_ok();
        core::mem::forget(opening);
        assert!(opening_was_accepted);
        assert!(state.board[0] == Some(Mark::X));
        assert!(state.turn == SeatId(42));
        let before = state.clone();
        let seat = SeatId(kani::any());
        let cell: u8 = kani::any();
        let should_reject = seat != state.turn || cell == 0 || cell >= 9;

        if should_reject {
            let result = place(&mut state, seat, cell);
            let rejected = result.is_err();
            let unchanged = canonical_state_fields_equal(&before, &state);
            // The proof concerns the state mutation, not `Outcome` destruction.
            // Avoid making CBMC unwind SmallVec's unrelated drop implementation.
            core::mem::forget(result);

            assert!(rejected);
            assert!(unchanged);
        }
    }
}
