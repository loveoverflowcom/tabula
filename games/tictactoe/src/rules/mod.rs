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
//! @ai.law canonical-rules-source-change-changes-rules-hash
//! @ai.evidence tests::rules_hash_matches_independent_rules_subtree_oracle
//! @ai.evidence tests::canonical_source_mutation_changes_oracle_hash
//! @ai.evidence tests::synthetic_non_rules_source_does_not_participate_in_compiled_hash

use smallvec::smallvec;
use tabula_core::{
    AbortReason, MatchOutcome, Millis, OutcomeKind, RuleError, RuleErrorCode, RulesVersion, SeatId,
    SeatRoster, Standing, TimerId, Viewer,
};
use tabula_game_api::{
    A11yDescription, AdminInput, Ctx, Effect, GameRules, Init, InitError, Input, LegalCommands,
    Outcome,
};

use crate::state::{Command, Config, Event, State, Status, View};

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
fn place(state: &mut State, seat: SeatId, cell: u8) -> Result<Outcome<TicTacToeRules>, RuleError> {
    // ---- validate fully BEFORE mutating (contract R2) ----
    if seat != state.turn {
        return Err(RuleError::code(RuleErrorCode::NotYourTurn));
    }
    let idx = cell as usize;
    if idx >= 9 || state.board[idx].is_some() {
        return Err(RuleError::code(RuleErrorCode::IllegalMove));
    }

    // ---- mutate ----
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

    Ok(Outcome { events, effects })
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
