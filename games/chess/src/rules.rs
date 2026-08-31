#![allow(clippy::doc_markdown)] // `@ai.*` schema values must remain bare machine-readable paths.

//! Chess's single pure state transition.
//!
//! @ai.role functional-core
//! @ai.domain chess.rules
//! @ai.pure true
//! @ai.invariant rejected-input-preserves-state
//! @ai.invariant legal-move-never-leaves-own-king-attacked
//! @ai.law deterministic-legal-command-order
//! @ai.evidence tests::rules::illegal_moves_are_byte_identical_noops
//! @ai.evidence tests::perft::published_positions_match

use smallvec::{smallvec, SmallVec};
use tabula_core::{
    AbortReason, MatchOutcome, OutcomeKind, RuleError, RuleErrorCode, RulesVersion, SeatId,
    SeatRoster, Standing, Viewer,
};
use tabula_game_api::{
    AdminInput, Ctx, Effect, GameRules, Init, InitError, Input, LegalCommands, Outcome,
};

use crate::{
    movegen::{apply_move, in_check, legal_moves},
    Color, Command, Config, Event, PieceKind, State, Status, View, ViewEvent,
};

/// The complete standard-chess rule implementation.
#[derive(Debug)]
pub struct ChessRules;

impl GameRules for ChessRules {
    type State = State;
    type Command = Command;
    type Event = Event;
    type View = View;
    type ViewEvent = ViewEvent;
    type Config = Config;

    // PositionKey is compact Zobrist state rather than a serialized board.
    // That changes canonical State encoding, so the match version advances.
    const RULES_VERSION: RulesVersion = RulesVersion(2);

    fn create(
        config: &Config,
        roster: &SeatRoster,
        _ctx: &mut Ctx<'_>,
    ) -> Result<Init<Self>, InitError> {
        if !has_standard_roster(roster) {
            return Err(InitError::SeatCount {
                got: roster.len().try_into().unwrap_or(u8::MAX),
                allowed: "seats 0 and 1".into(),
            });
        }
        if config.clock.is_some() {
            return Err(InitError::Config("clock".into()));
        }
        Ok(Init {
            state: State::initial(),
            events: smallvec![],
            effects: smallvec![],
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
        match input {
            Input::Player { seat, command } => apply_player(state, seat, command),
            // Clock handling is intentionally deferred, but a stale/no-clock timer
            // is a deterministic harmless input, not a reason to panic.
            Input::Timer { .. } | Input::Seat { .. } => Ok(Outcome::empty()),
            Input::Admin(AdminInput::Cancel { reason }) => Ok(end(state, aborted(reason))),
            Input::Admin(AdminInput::ForceEnd { outcome }) => Ok(end(state, outcome)),
            Input::Admin(_) => Err(RuleError::code(RuleErrorCode::Unsupported)),
        }
    }

    fn project(state: &State, viewer: Viewer) -> View {
        let you = match viewer {
            Viewer::Seat(seat) => Color::from_seat(seat),
            Viewer::Spectator(_) | Viewer::Audit => None,
        };
        let legal_moves = if you == Some(state.turn) && matches!(state.status, Status::Playing) {
            commands_for(state)
        } else {
            Vec::new()
        };
        View {
            board: state.board,
            turn: state.turn,
            castling: state.castling,
            en_passant: state.en_passant,
            halfmove_clock: state.halfmove_clock,
            fullmove_number: state.fullmove_number,
            status: state.status.clone(),
            draw_offer: state.draw_offer,
            clock: state.clock,
            you,
            legal_moves,
        }
    }

    fn view_event(_state_after: &State, event: &Event, _viewer: Viewer) -> Option<ViewEvent> {
        Some(match event {
            Event::Moved {
                seat,
                from,
                to,
                promotion,
                captured,
            } => ViewEvent::Moved {
                seat: *seat,
                from: *from,
                to: *to,
                promotion: *promotion,
                captured: *captured,
            },
            Event::DrawOffered { seat } => ViewEvent::DrawOffered { seat: *seat },
            Event::DrawDeclined { seat } => ViewEvent::DrawDeclined { seat: *seat },
            Event::Ended { outcome } => ViewEvent::Ended {
                outcome: outcome.clone(),
            },
        })
    }

    fn legal_commands(state: &State, seat: SeatId) -> LegalCommands<Command> {
        if Color::from_seat(seat) != Some(state.turn) || !matches!(state.status, Status::Playing) {
            return LegalCommands::None;
        }
        LegalCommands::Enumerated(commands_for(state))
    }
}

fn apply_player(
    state: &mut State,
    seat: SeatId,
    command: Command,
) -> Result<Outcome<ChessRules>, RuleError> {
    let color = Color::from_seat(seat).ok_or_else(|| RuleError::code(RuleErrorCode::NoSuchSeat))?;
    let turn_bound = matches!(command, Command::Move { .. } | Command::ClaimDraw);
    if turn_bound && color != state.turn {
        return Err(RuleError::code(RuleErrorCode::NotYourTurn));
    }
    match command {
        Command::Move {
            from,
            to,
            promotion,
        } => {
            // This is the validation barrier: only a candidate emitted by our
            // deterministic legal generator can reach the mutation below.
            let candidate = legal_moves(state)
                .into_iter()
                .find(|candidate| {
                    candidate.from.0 == from
                        && candidate.to.0 == to
                        && candidate.promotion == promotion
                })
                .ok_or_else(|| RuleError::code(RuleErrorCode::IllegalMove))?;
            let captured = apply_move(state, candidate, true);
            let mut events = smallvec![Event::Moved {
                seat,
                from: candidate.from,
                to: candidate.to,
                promotion: candidate.promotion,
                captured
            }];
            let mut effects = SmallVec::new();
            if let Some(outcome) = terminal_outcome(state) {
                state.status = Status::Ended {
                    outcome: outcome.clone(),
                };
                events.push(Event::Ended {
                    outcome: outcome.clone(),
                });
                effects.push(Effect::EndMatch { outcome });
            }
            Ok(Outcome { events, effects })
        }
        Command::Resign => {
            let outcome = if insufficient_material(state) {
                draw("dead position")
            } else {
                decisive(color.other(), "resignation")
            };
            Ok(end(state, outcome))
        }
        Command::OfferDraw => {
            // Offers are made after the offerer's move, while the opponent is
            // on turn. This keeps a pending offer alive until that opponent
            // responds or makes the next move.
            if color == state.turn || state.fullmove_number == 1 || state.draw_offer.is_some() {
                return Err(RuleError::code(RuleErrorCode::WrongPhase));
            }
            state.draw_offer = Some(color);
            Ok(Outcome {
                events: smallvec![Event::DrawOffered { seat }],
                effects: smallvec![],
            })
        }
        Command::AcceptDraw => {
            if state.draw_offer != Some(color.other()) {
                return Err(RuleError::code(RuleErrorCode::WrongPhase));
            }
            Ok(end(state, draw("draw agreed")))
        }
        Command::DeclineDraw => {
            if state.draw_offer != Some(color.other()) {
                return Err(RuleError::code(RuleErrorCode::WrongPhase));
            }
            state.draw_offer = None;
            Ok(Outcome {
                events: smallvec![Event::DrawDeclined { seat }],
                effects: smallvec![],
            })
        }
        Command::ClaimDraw => {
            if !can_claim_draw(state) {
                return Err(RuleError::code(RuleErrorCode::WrongPhase));
            }
            Ok(end(state, draw("draw claimed")))
        }
    }
}

fn commands_for(state: &State) -> Vec<Command> {
    legal_moves(state)
        .into_iter()
        .map(|candidate| Command::Move {
            from: candidate.from.0,
            to: candidate.to.0,
            promotion: candidate.promotion,
        })
        .collect()
}

fn terminal_outcome(state: &State) -> Option<MatchOutcome> {
    let available = legal_moves(state);
    if available.is_empty() {
        return Some(if in_check(state, state.turn) {
            decisive(state.turn.other(), "checkmate")
        } else {
            draw("stalemate")
        });
    }
    if state.halfmove_clock >= 150 {
        return Some(draw("seventy-five-move rule"));
    }
    if repetition_count(state) >= 5 {
        return Some(draw("fivefold repetition"));
    }
    if insufficient_material(state) {
        return Some(draw("insufficient material"));
    }
    None
}

fn can_claim_draw(state: &State) -> bool {
    state.halfmove_clock >= 100 || repetition_count(state) >= 3
}

fn has_standard_roster(roster: &SeatRoster) -> bool {
    roster.len() == 2 && roster.get(SeatId(0)).is_some() && roster.get(SeatId(1)).is_some()
}

fn repetition_count(state: &State) -> usize {
    state.repetition.last().map_or(0, |current| {
        state
            .repetition
            .iter()
            .filter(|entry| *entry == current)
            .count()
    })
}

fn insufficient_material(state: &State) -> bool {
    let minor: Vec<_> = state
        .board
        .iter()
        .enumerate()
        .filter_map(|(square, piece)| {
            let square = u8::try_from(square).ok()?;
            piece.map(|piece| (square, piece))
        })
        .filter(|(_, piece)| piece.kind != PieceKind::King)
        .collect();
    match minor.as_slice() {
        [] => true,
        [(_, piece)] => matches!(piece.kind, PieceKind::Bishop | PieceKind::Knight),
        [(first_square, first), (second_square, second)] => {
            first.kind == PieceKind::Bishop
                && second.kind == PieceKind::Bishop
                && ((first_square % 8 + first_square / 8) % 2
                    == (second_square % 8 + second_square / 8) % 2)
        }
        _ => false,
    }
}

fn end(state: &mut State, outcome: MatchOutcome) -> Outcome<ChessRules> {
    state.status = Status::Ended {
        outcome: outcome.clone(),
    };
    state.draw_offer = None;
    Outcome {
        events: smallvec![Event::Ended {
            outcome: outcome.clone()
        }],
        effects: smallvec![Effect::EndMatch { outcome }],
    }
}

fn decisive(winner: Color, summary: &str) -> MatchOutcome {
    MatchOutcome::new_for_seats(
        OutcomeKind::Decisive,
        smallvec![
            Standing {
                seat: winner.seat(),
                rank: 0,
                score: 1
            },
            Standing {
                seat: winner.other().seat(),
                rank: 1,
                score: 0
            }
        ],
        summary.into(),
        &[SeatId(0), SeatId(1)],
    )
    .expect("standard chess seats form a valid decisive outcome")
}

fn draw(summary: &str) -> MatchOutcome {
    MatchOutcome::new_for_seats(
        OutcomeKind::Draw,
        smallvec![
            Standing {
                seat: SeatId(0),
                rank: 0,
                score: 0
            },
            Standing {
                seat: SeatId(1),
                rank: 0,
                score: 0
            }
        ],
        summary.into(),
        &[SeatId(0), SeatId(1)],
    )
    .expect("standard chess seats form a valid drawn outcome")
}

fn aborted(reason: AbortReason) -> MatchOutcome {
    MatchOutcome::new_for_seats(
        OutcomeKind::Aborted { reason },
        SmallVec::new(),
        "cancelled".into(),
        &[SeatId(0), SeatId(1)],
    )
    .expect("empty aborted outcome is structurally valid")
}
