//! Projection control tests for a **perfect-information** game.
//!
//! Chess has no hidden information (`capabilities.hidden_information ==
//! false`), so it cannot
//! exercise the noninterference property. See
//! `crates/tabula-testkit/tests/projection_noninterference.rs` for the real
//! hidden-information exercise. What this file checks:
//!
//! ```text
//! P1  projection determinism: same state + same viewer -> same projection
//! P3  a real, public state change (a move) remains observable to every
//!     viewer kind, including a spectator
//! P4  seated non-move affordances agree with canonical validation, including
//!     repetition claims; readonly/terminal viewers cannot act
//! P5  movement hints stay move-only and check is a projected public fact
//! ```

use smallvec::smallvec;
use tabula_core::{
    canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, Millis, Occupant, SeatEntry,
    SeatId, SeatRoster, SpectatorTier, UserId, Viewer,
};
use tabula_game_api::{Budget, Ctx, GameRules, Input, LegalCommands};
use tabula_game_chess::{ChessRules, Color, Command, Config, State, Status};
use tabula_testkit::determinism::{run_typed, Scenario};
use tabula_testkit::{assert_projection_differs, assert_projection_noninterference};

fn roster() -> SeatRoster {
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
    ])
    .expect("fixture seats are unique")
}

fn move_to(seat: u8, from: u8, to: u8) -> Input<Command> {
    Input::Player {
        seat: SeatId(seat),
        command: Command::Move {
            from,
            to,
            promotion: None,
        },
    }
}

fn scenario(inputs: Vec<Input<Command>>) -> Scenario<ChessRules> {
    Scenario {
        config: Config::default(),
        roster: roster(),
        seed: MatchSeed::from_bytes([11u8; 32]),
        inputs,
    }
}

const VIEWERS: [Viewer; 3] = [
    Viewer::Seat(SeatId(0)),
    Viewer::Seat(SeatId(1)),
    Viewer::Spectator(SpectatorTier::Live),
];

#[test]
fn projection_is_deterministic_for_a_fixed_reachable_state() {
    let state = run_typed::<ChessRules>(&scenario(vec![move_to(0, 13, 21), move_to(1, 52, 36)]))
        .expect("fixture script is legal");

    for viewer in VIEWERS {
        assert_projection_noninterference::<ChessRules>(
            "chess determinism",
            &state,
            &state,
            viewer,
        );
    }
}

#[test]
fn a_move_changes_the_board_which_is_public_to_every_viewer() {
    let before = run_typed::<ChessRules>(&scenario(Vec::new())).expect("create succeeds");
    let after =
        run_typed::<ChessRules>(&scenario(vec![move_to(0, 13, 21)])).expect("e2-e4 is legal");

    for viewer in VIEWERS {
        assert_projection_differs::<ChessRules>("a played move is public", &before, &after, viewer);
    }
}

const ACTIONS: [Command; 5] = [
    Command::Resign,
    Command::OfferDraw,
    Command::AcceptDraw,
    Command::DeclineDraw,
    Command::ClaimDraw,
];

fn apply_action(
    state: &mut State,
    color: Color,
    command: Command,
) -> Result<tabula_game_api::Outcome<ChessRules>, tabula_core::RuleError> {
    let seed = MatchSeed::from_bytes([23; 32]);
    let mut rng = DetRng::for_input(&seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget::default(),
    };
    ChessRules::apply(
        state,
        Input::Player {
            seat: color.seat(),
            command,
        },
        &mut ctx,
    )
}

fn actions(state: &State, color: Color) -> Vec<Command> {
    ChessRules::project(state, Viewer::Seat(color.seat())).actions
}

/// A snapshot's non-move affordances must be complete and valid for each seat.
/// Rejections must still preserve the canonical bytes (R2).
fn assert_actions_match_reducer(state: &State) -> [usize; 5] {
    let mut accepted = [0; 5];
    for color in [Color::White, Color::Black] {
        let projected = actions(state, color);
        assert_eq!(projected, actions(state, color), "action order is stable");
        for (index, command) in ACTIONS.into_iter().enumerate() {
            let mut candidate = state.clone();
            let before = canonical_encode(&candidate).unwrap();
            let result = apply_action(&mut candidate, color, command);
            assert_eq!(
                projected.contains(&command),
                result.is_ok(),
                "projection disagrees with apply for {color:?} {command:?}"
            );
            if result.is_ok() {
                accepted[index] += 1;
            } else {
                assert_eq!(canonical_encode(&candidate).unwrap(), before);
            }
        }
    }
    accepted
}

#[test]
fn opening_projects_resignation_but_no_premature_draw_offer() {
    let initial = State::initial();
    for color in [Color::White, Color::Black] {
        assert_eq!(actions(&initial, color), vec![Command::Resign]);
    }
    assert_actions_match_reducer(&initial);

    let after_white =
        run_typed::<ChessRules>(&scenario(vec![move_to(0, 12, 28)])).expect("e2-e4 is legal");
    for color in [Color::White, Color::Black] {
        assert_eq!(actions(&after_white, color), vec![Command::Resign]);
    }
    assert_actions_match_reducer(&after_white);
}

#[test]
fn draw_offer_actions_follow_off_turn_seat_and_recipient() {
    let mut state =
        run_typed::<ChessRules>(&scenario(vec![move_to(0, 12, 28), move_to(1, 52, 36)]))
            .expect("opening moves are legal");
    assert_eq!(actions(&state, Color::White), vec![Command::Resign]);
    assert_eq!(
        actions(&state, Color::Black),
        vec![Command::Resign, Command::OfferDraw]
    );
    assert_actions_match_reducer(&state);

    apply_action(&mut state, Color::Black, Command::OfferDraw).unwrap();
    assert_eq!(
        actions(&state, Color::White),
        vec![Command::Resign, Command::AcceptDraw, Command::DeclineDraw]
    );
    assert_eq!(actions(&state, Color::Black), vec![Command::Resign]);
    assert_actions_match_reducer(&state);

    let mut answered_by_move = state.clone();
    apply_action(
        &mut answered_by_move,
        Color::White,
        Command::Move {
            from: 6,
            to: 21,
            promotion: None,
        },
    )
    .unwrap();
    assert_eq!(answered_by_move.draw_offer, None);
    assert_eq!(
        actions(&answered_by_move, Color::White),
        vec![Command::Resign, Command::OfferDraw]
    );
    assert_actions_match_reducer(&answered_by_move);

    let mut agreed = state.clone();
    apply_action(&mut agreed, Color::White, Command::AcceptDraw).unwrap();
    let Status::Ended { outcome } = &agreed.status else {
        panic!("acceptance must end the match");
    };
    assert_eq!(outcome.summary(), "draw agreed");
    assert_actions_match_reducer(&agreed);

    apply_action(&mut state, Color::White, Command::DeclineDraw).unwrap();
    assert_eq!(state.draw_offer, None);
    assert_eq!(state.turn, Color::White);
    assert_actions_match_reducer(&state);

    apply_action(
        &mut state,
        Color::White,
        Command::Move {
            from: 6,
            to: 21,
            promotion: None,
        },
    )
    .unwrap();
    assert_eq!(
        actions(&state, Color::White),
        vec![Command::Resign, Command::OfferDraw]
    );
    assert_eq!(actions(&state, Color::Black), vec![Command::Resign]);
    apply_action(&mut state, Color::White, Command::OfferDraw).unwrap();
    assert_eq!(
        actions(&state, Color::Black),
        vec![Command::Resign, Command::AcceptDraw, Command::DeclineDraw]
    );
    assert_eq!(actions(&state, Color::White), vec![Command::Resign]);
    assert_actions_match_reducer(&state);
}

#[test]
fn readonly_and_terminal_views_have_no_action_or_movement_affordances() {
    let mut state = State::initial();
    let readonly = [
        Viewer::Seat(SeatId(2)),
        Viewer::Seat(SeatId(255)),
        Viewer::Spectator(SpectatorTier::Live),
        Viewer::Spectator(SpectatorTier::Delayed { by: Millis(1_000) }),
        Viewer::Audit,
    ];
    for viewer in readonly {
        let view = ChessRules::project(&state, viewer);
        assert_eq!(view.you, None);
        assert!(view.actions.is_empty());
        assert!(view.legal_moves.is_empty());
    }

    apply_action(&mut state, Color::White, Command::Resign).unwrap();
    assert!(matches!(state.status, Status::Ended { .. }));
    for viewer in readonly
        .into_iter()
        .chain([Viewer::Seat(SeatId(0)), Viewer::Seat(SeatId(1))])
    {
        let view = ChessRules::project(&state, viewer);
        assert!(view.actions.is_empty());
        assert!(view.legal_moves.is_empty());
    }
    assert_actions_match_reducer(&state);
}

#[test]
fn fifty_move_claim_is_projected_only_at_the_threshold_and_to_current_turn() {
    for (turn, color) in [("w", Color::White), ("b", Color::Black)] {
        for halfmoves in [99, 100] {
            let state = State::from_fen(&format!("k7/8/8/8/8/8/1R6/4K3 {turn} - - {halfmoves} 8"))
                .expect("fifty-move fixture is valid");
            assert_eq!(
                actions(&state, color).contains(&Command::ClaimDraw),
                halfmoves == 100
            );
            assert!(!actions(&state, color.other()).contains(&Command::ClaimDraw));
            assert_actions_match_reducer(&state);
        }
    }
}

#[test]
fn repetition_claim_uses_canonical_history_without_reconstructing_it_from_view() {
    let cycle = [
        move_to(0, 6, 21),
        move_to(1, 62, 45),
        move_to(0, 21, 6),
        move_to(1, 45, 62),
    ];
    let twice = run_typed::<ChessRules>(&scenario(cycle.to_vec())).unwrap();
    assert!(!actions(&twice, Color::White).contains(&Command::ClaimDraw));
    assert_actions_match_reducer(&twice);

    let repeated =
        run_typed::<ChessRules>(&scenario(cycle.iter().cloned().cycle().take(8).collect()))
            .unwrap();
    let reset_history =
        State::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 8 5").unwrap();
    let repeated_view = ChessRules::project(&repeated, Viewer::Seat(SeatId(0)));
    let reset_view = ChessRules::project(&reset_history, Viewer::Seat(SeatId(0)));
    assert_eq!(repeated_view.board, reset_view.board);
    assert_eq!(repeated_view.turn, reset_view.turn);
    assert_eq!(repeated_view.halfmove_clock, reset_view.halfmove_clock);
    assert_eq!(repeated_view.fullmove_number, reset_view.fullmove_number);
    assert_eq!(repeated_view.legal_moves, reset_view.legal_moves);
    assert!(repeated_view.actions.contains(&Command::ClaimDraw));
    assert!(!reset_view.actions.contains(&Command::ClaimDraw));
    assert!(!actions(&repeated, Color::Black).contains(&Command::ClaimDraw));
    assert_actions_match_reducer(&repeated);
    assert_actions_match_reducer(&reset_history);

    let black_claim =
        run_typed::<ChessRules>(&scenario(cycle.into_iter().cycle().take(9).collect())).unwrap();
    assert_eq!(black_claim.turn, Color::Black);
    assert!(actions(&black_claim, Color::Black).contains(&Command::ClaimDraw));
    assert!(!actions(&black_claim, Color::White).contains(&Command::ClaimDraw));
    assert_actions_match_reducer(&black_claim);
}

#[test]
fn projected_movement_hints_and_legal_commands_remain_move_only() {
    let state = State::initial();
    let white = ChessRules::project(&state, Viewer::Seat(SeatId(0)));
    let black = ChessRules::project(&state, Viewer::Seat(SeatId(1)));
    let LegalCommands::Enumerated(commands) = ChessRules::legal_commands(&state, SeatId(0)) else {
        panic!("white has opening moves");
    };
    assert_eq!(white.legal_moves, commands);
    assert_eq!(white.legal_moves.len(), 20);
    assert!(white
        .legal_moves
        .iter()
        .all(|command| matches!(command, Command::Move { .. })));
    assert!(black.legal_moves.is_empty());
    assert!(matches!(
        ChessRules::legal_commands(&state, SeatId(1)),
        LegalCommands::None
    ));
    assert!(white
        .actions
        .iter()
        .all(|command| !matches!(command, Command::Move { .. })));
}

#[test]
fn check_is_projected_for_every_viewer_and_preserved_at_checkmate() {
    for fen in [
        "4k3/8/8/8/8/8/4r3/4K3 w - - 0 1",
        "4k3/4R3/8/8/8/8/8/4K3 b - - 0 1",
    ] {
        let state = State::from_fen(fen).unwrap();
        for viewer in VIEWERS.into_iter().chain([Viewer::Audit]) {
            assert!(ChessRules::project(&state, viewer).in_check);
        }
    }
    for viewer in VIEWERS {
        assert!(!ChessRules::project(&State::initial(), viewer).in_check);
    }
    let mate = run_typed::<ChessRules>(&scenario(vec![
        move_to(0, 13, 21),
        move_to(1, 52, 36),
        move_to(0, 14, 30),
        move_to(1, 59, 31),
    ]))
    .expect("Fool's mate is legal");
    assert!(matches!(mate.status, Status::Ended { .. }));
    for viewer in VIEWERS {
        let view = ChessRules::project(&mate, viewer);
        assert!(view.in_check);
        assert!(view.actions.is_empty());
    }
}

#[test]
fn every_non_move_command_is_reached_in_projection_reducer_agreement_checks() {
    let mut opening =
        run_typed::<ChessRules>(&scenario(vec![move_to(0, 12, 28), move_to(1, 52, 36)])).unwrap();
    let offered = {
        apply_action(&mut opening, Color::Black, Command::OfferDraw).unwrap();
        opening
    };
    let claimable = State::from_fen("k7/8/8/8/8/8/1R6/4K3 w - - 100 8").unwrap();
    let mut reached = [0; 5];
    for state in [State::initial(), offered, claimable] {
        for (total, count) in reached.iter_mut().zip(assert_actions_match_reducer(&state)) {
            *total += count;
        }
    }
    for (command, count) in ACTIONS.into_iter().zip(reached) {
        assert!(count > 0, "{command:?} was never exercised as accepted");
    }
}
