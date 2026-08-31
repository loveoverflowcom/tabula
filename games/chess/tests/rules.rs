//! Chess-specific transition and edge-partition evidence.

use tabula_core::{
    canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, RuleErrorCode, SeatId,
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
fn threefold_repetition_ends_the_match() {
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
    assert!(matches!(state.status, Status::Ended { .. }));
    assert!(final_outcome
        .unwrap()
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::EndMatch { .. })));
}

#[test]
fn fifty_move_rule_and_insufficient_material_are_terminal_draws() {
    let mut fifty = State::from_fen("k7/8/8/8/8/8/1R6/4K3 w - - 99 1").unwrap();
    let outcome = apply(&mut fifty, 1, 0, move_to(9, 17)).unwrap();
    assert!(matches!(fifty.status, Status::Ended { .. }));
    assert!(outcome
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::EndMatch { .. })));

    let mut material = State::from_fen("k7/8/8/8/8/8/8/2B1K3 w - - 0 1").unwrap();
    apply(&mut material, 1, 0, move_to(2, 11)).unwrap();
    assert!(matches!(material.status, Status::Ended { .. }));
}

#[test]
fn repetition_key_omits_an_unavailable_en_passant_target() {
    let with_irrelevant_ep = State::from_fen("4k3/8/8/8/8/8/8/4K3 w - e3 0 1").unwrap();
    let without_ep = State::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    assert_eq!(with_irrelevant_ep.position_key(), without_ep.position_key());
    assert_eq!(with_irrelevant_ep.turn, Color::White);
}

#[test]
fn a_draw_offer_is_answered_by_the_other_seat_without_changing_turn() {
    let mut state = State::initial();
    apply(&mut state, 1, 0, Command::OfferDraw).unwrap();
    assert_eq!(state.draw_offer, Some(Color::White));
    apply(&mut state, 2, 1, Command::AcceptDraw).unwrap();
    assert!(matches!(state.status, Status::Ended { .. }));
}
