#![allow(clippy::doc_markdown)] // `@ai.*` schema values must remain bare machine-readable paths.

//! Chess's single pure state transition.
//!
//! @ai.role functional-core
//! @ai.domain chess.rules
//! @ai.pure true
//! @ai.invariant rejected-input-preserves-state
//! @ai.invariant legal-move-never-leaves-own-king-attacked
//! @ai.invariant timeout-requires-mating-capability
//! @ai.invariant valid-player-action-cannot-bypass-deadline
//! @ai.law deterministic-legal-command-order
//! @ai.evidence tests::rules::illegal_moves_are_byte_identical_noops
//! @ai.evidence tests::perft::published_positions_match
//! @ai.evidence tests::clocks::timeout_ignores_flagged_side_material_when_survivor_is_bare_king
//! @ai.evidence tests::clocks::expired_clock_preempts_valid_non_move_commands

use smallvec::{smallvec, SmallVec};
use tabula_core::{
    AbortReason, MatchOutcome, OutcomeKind, RuleError, RuleErrorCode, RulesVersion, SeatId,
    SeatRoster, Standing, Viewer,
};
use tabula_game_api::{
    AdminInput, Ctx, Effect, GameRules, Init, InitError, Input, LegalCommands, Outcome,
};

use crate::{
    clock::{self, MoveCharge, TimerCheck},
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

    // PositionKey is compact Zobrist state rather than a serialized board, and
    // clock state/events now participate in the canonical contract.
    const RULES_VERSION: RulesVersion = RulesVersion(3);

    fn create(
        config: &Config,
        roster: &SeatRoster,
        ctx: &mut Ctx<'_>,
    ) -> Result<Init<Self>, InitError> {
        if !has_standard_roster(roster) {
            return Err(InitError::SeatCount {
                got: roster.len().try_into().unwrap_or(u8::MAX),
                allowed: "seats 0 and 1".into(),
            });
        }
        if let Some(clock) = config.clock {
            if !clock::config_is_valid(&clock) {
                return Err(InitError::Config("clock".into()));
            }
        }
        let mut state = State::initial();
        let mut effects = SmallVec::new();
        if let Some(clock_config) = config.clock {
            let clock = clock::initial_state(clock_config, ctx.now);
            effects.push(clock::arm_effect(&clock, Color::White, ctx.now));
            state.clock = Some(clock);
        }
        Ok(Init {
            state,
            events: smallvec![],
            effects,
        })
    }

    fn apply(
        state: &mut State,
        input: Input<Command>,
        ctx: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        if !matches!(state.status, Status::Playing) {
            return Err(RuleError::code(RuleErrorCode::MatchOver));
        }
        match input {
            Input::Player { seat, command } => apply_player(state, seat, command, ctx.now),
            Input::Timer { timer } => Ok(apply_timer(state, timer, ctx.now)),
            // Disconnects do not pause Chess clocks; the already requested timer
            // continues to burn (doc 02 §12.1).
            Input::Seat { .. } => Ok(Outcome::empty()),
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
            Event::ClockUpdated { seat, remaining } => ViewEvent::ClockUpdated {
                seat: *seat,
                remaining: *remaining,
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
    now: tabula_core::LogicalTime,
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

            // Evaluate time only after legality is established and before the
            // first state mutation, preserving R2 for rejected commands.
            let charge = clock::charge_completed_move(state.clock.as_ref(), color, now);
            if matches!(charge, MoveCharge::Flagged) {
                return Ok(timeout(state, color, now));
            }
            let captured = apply_move(state, candidate, true);
            let mut events = smallvec![Event::Moved {
                seat,
                from: candidate.from,
                to: candidate.to,
                promotion: candidate.promotion,
                captured
            }];
            let mut effects = SmallVec::new();
            if let MoveCharge::Ready(remaining) = charge {
                if let Some(clock) = state.clock.as_mut() {
                    clock.remaining[color_index(color)] = remaining;
                    clock.last_move_at = now;
                    events.push(Event::ClockUpdated { seat, remaining });
                }
            }
            if let Some(outcome) = terminal_outcome(state) {
                let ended = end(state, outcome);
                events.extend(ended.events);
                effects.extend(ended.effects);
            } else if let Some(clock) = state.clock.as_ref() {
                effects.push(clock::arm_effect(clock, state.turn, now));
            }
            Ok(Outcome { events, effects })
        }
        command => apply_non_move(state, seat, color, command, now),
    }
}

fn apply_non_move(
    state: &mut State,
    seat: SeatId,
    color: Color,
    command: Command,
    now: tabula_core::LogicalTime,
) -> Result<Outcome<ChessRules>, RuleError> {
    // Resolve command validity before the deadline. Once a non-move command
    // is valid, the current turn's expiry preempts it even if its timer effect
    // has not yet been delivered by the platform.
    match command {
        Command::Resign => {
            if turn_is_expired(state, now) {
                return Ok(timeout(state, state.turn, now));
            }
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
            if turn_is_expired(state, now) {
                return Ok(timeout(state, state.turn, now));
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
            if turn_is_expired(state, now) {
                return Ok(timeout(state, state.turn, now));
            }
            Ok(end(state, draw("draw agreed")))
        }
        Command::DeclineDraw => {
            if state.draw_offer != Some(color.other()) {
                return Err(RuleError::code(RuleErrorCode::WrongPhase));
            }
            if turn_is_expired(state, now) {
                return Ok(timeout(state, state.turn, now));
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
            if turn_is_expired(state, now) {
                return Ok(timeout(state, state.turn, now));
            }
            Ok(end(state, draw("draw claimed")))
        }
        Command::Move { .. } => Err(RuleError::code(RuleErrorCode::IllegalMove)),
    }
}

fn apply_timer(
    state: &mut State,
    timer: tabula_core::TimerId,
    now: tabula_core::LogicalTime,
) -> Outcome<ChessRules> {
    if timer != clock::TIMER_CLOCK {
        return Outcome::empty();
    }

    let Some(clock_state) = state.clock.as_ref() else {
        return Outcome::empty();
    };
    match clock::check_timer(clock_state, state.turn, now) {
        TimerCheck::Active(_) => Outcome {
            events: smallvec![],
            effects: smallvec![clock::arm_effect(clock_state, state.turn, now)],
        },
        TimerCheck::Flagged => timeout(state, state.turn, now),
    }
}

fn timeout(
    state: &mut State,
    flagged: Color,
    now: tabula_core::LogicalTime,
) -> Outcome<ChessRules> {
    let clock_updated = state.clock.as_mut().map(|clock| {
        clock::flag(clock, flagged, now);
        Event::ClockUpdated {
            seat: flagged.seat(),
            remaining: tabula_core::Millis::ZERO,
        }
    });
    let mut outcome = end(state, timeout_outcome(state, flagged));
    if let Some(event) = clock_updated {
        outcome.events.insert(0, event);
    }
    outcome
}

/// Returns the timeout result after checking whether the surviving side has a
/// possible mating construction. A bare king cannot mate, even when the side
/// that flagged still owns a queen or other substantial material; that
/// material cannot be assumed to cooperate with the winner.
fn timeout_outcome(state: &State, flagged: Color) -> MatchOutcome {
    let winner = flagged.other();
    if can_checkmate(state, winner, flagged) {
        decisive(winner, "timeout")
    } else {
        draw("timeout")
    }
}

/// Checks the finite material cases that can mate a bare king, plus helpmate
/// material on the flagged side. The latter matters because timeout is about
/// whether *any* future legal series can contain mate, not whether the winner
/// can force mate against a cooperating bare king.
fn can_checkmate(state: &State, winner: Color, flagged: Color) -> bool {
    let mut non_king = 0_u8;
    let mut has_pawn_or_major = false;
    let mut has_bishop = false;
    let mut has_knight = false;
    let mut bishop_colors = [false; 2];
    let mut flagged_has_material = false;

    for (square, piece) in state
        .board
        .iter()
        .enumerate()
        .filter_map(|(square, piece)| piece.map(|piece| (square, piece)))
    {
        if piece.kind == PieceKind::King {
            continue;
        }
        if piece.color == flagged {
            flagged_has_material = true;
            continue;
        }
        if piece.color != winner {
            continue;
        }

        non_king = non_king.saturating_add(1);
        match piece.kind {
            PieceKind::Pawn | PieceKind::Rook | PieceKind::Queen => {
                has_pawn_or_major = true;
            }
            PieceKind::Bishop => {
                has_bishop = true;
                let square_color = (square % 8 + square / 8) % 2;
                bishop_colors[square_color] = true;
            }
            PieceKind::Knight => has_knight = true,
            PieceKind::King => unreachable!(),
        }
    }

    if has_pawn_or_major || (has_bishop && has_knight) || bishop_colors == [true, true] {
        return true;
    }

    // A non-king piece belonging to the flagged side can provide the blocker
    // needed by a minor-only helpmate. With no such piece, K+B/K+N and the
    // other known bare-king dead positions cannot ever reach checkmate.
    flagged_has_material && non_king > 0
}

fn turn_is_expired(state: &State, now: tabula_core::LogicalTime) -> bool {
    state.clock.as_ref().is_some_and(|clock| {
        matches!(
            clock::check_timer(clock, state.turn, now),
            TimerCheck::Flagged
        )
    })
}

fn color_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
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
    let mut effects = SmallVec::new();
    if state.clock.is_some() {
        effects.push(Effect::CancelTimer {
            id: clock::TIMER_CLOCK,
        });
    }
    let events = smallvec![Event::Ended {
        outcome: outcome.clone(),
    }];
    effects.push(Effect::EndMatch { outcome });
    Outcome { events, effects }
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
