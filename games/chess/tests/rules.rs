//! Chess-specific transition and edge-partition evidence.

use tabula_core::{
    canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, OutcomeKind, RuleErrorCode,
    SeatId,
};
use tabula_game_api::{Budget, Ctx, Effect, GameRules, Input, LegalCommands};
use tabula_game_chess::{ChessRules, Color, Command, PieceKind, State, Status};

fn apply(
    state: &mut State,
    index: u64,
    seat: u8,
    command: Command,
) -> Result<tabula_game_api::Outcome<ChessRules>, tabula_core::RuleError> {
    let seed = MatchSeed::from_bytes([7; 32]);
    let mut rng = DetRng::for_input(&seed, InputIndex(index));
    let mut ctx = Ctx {
        now: LogicalTime(index * 1_000),
        index: InputIndex(index),
        rng: &mut rng,
        budget: Budget::default(),
    };
    ChessRules::apply(
        state,
        Input::Player {
            seat: SeatId(seat),
            command,
        },
        &mut ctx,
    )
}

fn move_to(from: u8, to: u8) -> Command {
    Command::Move {
        from,
        to,
        promotion: None,
    }
}

#[test]
fn illegal_moves_are_byte_identical_noops() {
    for (fen, command) in [
        // Moving an opponent piece.
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            move_to(48, 40),
        ),
        // A pinned rook may not expose its king.
        ("k3r3/8/8/8/8/8/4R3/4K3 w - - 0 1", move_to(12, 11)),
        // Castling through a square attacked by a rook is forbidden.
        ("k4r2/8/8/8/8/8/8/4K2R w K - 0 1", move_to(4, 6)),
        // En-passant cannot expose an attack on the moving side's king.
        ("k3r3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", move_to(36, 43)),
        // Hostile square bytes must be rejected without indexing panic.
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            move_to(255, 255),
        ),
    ] {
        let mut state = State::from_fen(fen).unwrap();
        let before = canonical_encode(&state).unwrap();
        let hash = ChessRules::state_hash(&state);
        let result = apply(&mut state, 1, 0, command);
        assert_eq!(result.unwrap_err().code, RuleErrorCode::IllegalMove);
        assert_eq!(canonical_encode(&state).unwrap(), before);
        assert_eq!(ChessRules::state_hash(&state), hash);
    }
}

#[test]
fn all_promotion_choices_are_legal_and_change_the_piece() {
    for promotion in [
        PieceKind::Queen,
        PieceKind::Rook,
        PieceKind::Bishop,
        PieceKind::Knight,
    ] {
        let mut state = State::from_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        apply(
            &mut state,
            1,
            0,
            Command::Move {
                from: 52,
                to: 60,
                promotion: Some(promotion),
            },
        )
        .unwrap();
        assert_eq!(state.board[60].unwrap().kind, promotion);
    }
}

#[test]
fn legal_commands_are_stably_ordered_and_all_authoritative() {
    let state = State::initial();
    let LegalCommands::Enumerated(first) = ChessRules::legal_commands(&state, SeatId(0)) else {
        panic!("white should have legal moves");
    };
    let LegalCommands::Enumerated(second) = ChessRules::legal_commands(&state, SeatId(0)) else {
        panic!("white should have legal moves");
    };
    assert_eq!(
        canonical_encode(&first).unwrap(),
        canonical_encode(&second).unwrap()
    );
    assert_eq!(first.len(), 20);
    for command in first {
        let mut candidate = state.clone();
        apply(&mut candidate, 1, 0, command).unwrap();
    }
}

#[test]
fn threefold_repetition_is_claimable_but_not_automatic() {
    let mut state = State::initial();
    let script = [
        (0, move_to(6, 21)),
        (1, move_to(62, 45)),
        (0, move_to(21, 6)),
        (1, move_to(45, 62)),
        (0, move_to(6, 21)),
        (1, move_to(62, 45)),
        (0, move_to(21, 6)),
        (1, move_to(45, 62)),
    ];
    let mut final_outcome = None;
    for (index, (seat, command)) in script.into_iter().enumerate() {
        final_outcome = Some(apply(&mut state, index as u64 + 1, seat, command).unwrap());
    }
    assert!(matches!(state.status, Status::Playing));
    assert!(!final_outcome
        .unwrap()
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::EndMatch { .. })));

    let outcome = apply(&mut state, 9, 0, Command::ClaimDraw).unwrap();
    assert!(matches!(state.status, Status::Ended { .. }));
    assert_eq!(outcome.events.len(), 1);
    assert!(outcome
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::EndMatch { .. })));
}

#[test]
fn fivefold_repetition_is_automatic() {
    let mut state = State::initial();
    let cycle = [
        (0, move_to(6, 21)),
        (1, move_to(62, 45)),
        (0, move_to(21, 6)),
        (1, move_to(45, 62)),
    ];
    for (index, (seat, command)) in cycle.into_iter().cycle().take(16).enumerate() {
        let outcome = apply(&mut state, index as u64 + 1, seat, command).unwrap();
        if index < 15 {
            assert!(outcome.effects.is_empty());
        } else {
            assert!(outcome
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::EndMatch { .. })));
        }
    }
    let Status::Ended { outcome } = state.status else {
        panic!("fivefold repetition must end the match");
    };
    assert_eq!(outcome.summary.as_str(), "fivefold repetition");
}

#[test]
fn fifty_move_rule_is_claimable_and_seventy_five_move_rule_is_automatic() {
    let mut fifty = State::from_fen("k7/8/8/8/8/8/1R6/4K3 w - - 99 1").unwrap();
    let outcome = apply(&mut fifty, 1, 0, move_to(9, 17)).unwrap();
    assert!(matches!(fifty.status, Status::Playing));
    assert!(!outcome
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::EndMatch { .. })));
    apply(&mut fifty, 2, 1, Command::ClaimDraw).unwrap();
    assert!(matches!(fifty.status, Status::Ended { .. }));

    let mut seventy_five = State::from_fen("k7/8/8/8/8/8/1R6/4K3 w - - 149 1").unwrap();
    let outcome = apply(&mut seventy_five, 1, 0, move_to(9, 17)).unwrap();
    assert!(matches!(seventy_five.status, Status::Ended { .. }));
    assert!(outcome
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::EndMatch { .. })));
}

#[test]
fn repetition_key_omits_an_unavailable_en_passant_target() {
    let with_irrelevant_ep = State::from_fen("4k3/8/8/4p3/8/8/8/4K3 w - e6 0 1").unwrap();
    let without_ep = State::from_fen("4k3/8/8/4p3/8/8/8/4K3 w - - 0 1").unwrap();
    assert_eq!(with_irrelevant_ep.position_key(), without_ep.position_key());
    assert_eq!(with_irrelevant_ep.turn, Color::White);
}

#[test]
fn a_draw_offer_is_answered_by_the_other_seat_without_changing_turn() {
    let mut state = State::initial();
    apply(&mut state, 1, 0, move_to(12, 28)).unwrap();
    apply(&mut state, 2, 1, move_to(52, 36)).unwrap();
    apply(&mut state, 2, 1, Command::OfferDraw).unwrap();
    assert_eq!(state.draw_offer, Some(Color::Black));
    assert_eq!(state.turn, Color::White);
    let rejected = apply(&mut state, 3, 1, Command::OfferDraw).unwrap_err();
    assert_eq!(rejected.code, RuleErrorCode::WrongPhase);
    apply(&mut state, 4, 0, Command::AcceptDraw).unwrap();
    assert!(matches!(state.status, Status::Ended { .. }));
}

#[test]
fn draw_offer_before_both_players_move_is_rejected() {
    let mut state = State::initial();
    let before = canonical_encode(&state).unwrap();
    let result = apply(&mut state, 1, 0, Command::OfferDraw);
    assert_eq!(result.unwrap_err().code, RuleErrorCode::WrongPhase);
    assert_eq!(canonical_encode(&state).unwrap(), before);

    apply(&mut state, 2, 0, move_to(12, 28)).unwrap();
    let before = canonical_encode(&state).unwrap();
    let result = apply(&mut state, 3, 0, Command::OfferDraw);
    assert_eq!(result.unwrap_err().code, RuleErrorCode::WrongPhase);
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

#[test]
fn resigning_from_a_dead_position_is_a_draw_and_can_happen_off_turn() {
    let mut state = State::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    apply(&mut state, 1, 1, Command::Resign).unwrap();
    let Status::Ended { outcome } = state.status else {
        panic!("resignation must end the match");
    };
    assert_eq!(outcome.kind, OutcomeKind::Draw);
}

#[test]
fn fen_rejects_unrepresentable_positions() {
    for fen in [
        "4k3/8/8/8/8/8/8/4K3 w - - 0 0",
        "4k3/8/8/8/8/8/8/4K3 w K - 0 1",
        "4k3/8/8/8/8/8/8/4K3 w - e3 0 1",
        "4k3/8/8/8/8/8/8/P3K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/3Kk3 w - - 0 1",
    ] {
        assert!(State::from_fen(fen).is_err(), "accepted invalid FEN: {fen}");
    }
}
