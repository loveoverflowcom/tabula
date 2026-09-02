//! Integration tests for [`LocalMatch`] driving both Chess and Tic-tac-toe.

#![allow(clippy::doc_markdown)]

use glam::Vec2;
use renderer_macroquad::MacroquadRenderer;
use tabula_core::{
    InputIndex, LogicalTime, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, TimerId, UserId,
    Viewer,
};
use tabula_game_api::Input;
use tabula_game_chess::{
    presentation::{BoardLayout as ChessLayout, ChessPresentation},
    ChessRules, Config as ChessConfig, PieceKind, Square,
};
use tabula_game_client::{LocalEffect, LocalMatch, RecordedInput};
use tabula_game_tictactoe::{
    presentation::{BoardLayout as TttLayout, TicTacToePresentation},
    Config as TttConfig, Mark, TicTacToeRules,
};
use tabula_presentation::{
    Dpi, FrameCtx, InputEvent, PointerButton, PointerPhase, PointerPosition, Viewport,
};

type ChessMatch = LocalMatch<ChessRules, ChessPresentation>;
type TicTacToeMatch = LocalMatch<TicTacToeRules, TicTacToePresentation>;

fn frame(now_ms: u64) -> FrameCtx {
    FrameCtx::new(
        Viewport::new(Vec2::splat(640.0)).expect("test viewport is valid"),
        Dpi::new(1.0).expect("test DPI is valid"),
        now_ms,
        tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light),
    )
}

fn roster() -> SeatRoster {
    SeatRoster::new(
        [
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
        ]
        .into_iter()
        .collect(),
    )
    .expect("local seats are unique")
}

fn chess_match() -> ChessMatch {
    ChessMatch::new(
        &ChessConfig::default(),
        &roster(),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("standard local rules configuration is valid")
}

fn chess_click(layout: ChessLayout, square: u8) -> InputEvent {
    let square = Square::new(square).expect("test square is valid");
    let rect = layout
        .square_rect(square)
        .expect("test square has geometry");
    InputEvent::Pointer {
        position: PointerPosition::new(rect.origin() + rect.size() * 0.5)
            .expect("test pointer is finite"),
        button: PointerButton::Primary,
        phase: PointerPhase::Up,
    }
}

fn ttt_match(timeout_ms: u64) -> TicTacToeMatch {
    TicTacToeMatch::new(
        &TttConfig {
            move_timeout_ms: timeout_ms,
        },
        &roster(),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("standard local tictactoe configuration is valid")
}

fn ttt_click(layout: TttLayout, cell: u8) -> InputEvent {
    let rect = layout.cell_rect(cell).expect("test cell has geometry");
    InputEvent::Pointer {
        position: PointerPosition::new(rect.origin() + rect.size() * 0.5)
            .expect("test pointer is finite"),
        button: PointerButton::Primary,
        phase: PointerPhase::Up,
    }
}

#[test]
fn shared_runtime_drives_both_chess_and_tictactoe_without_game_specific_branches() {
    let frame = frame(0);

    // 1. Chess drives pawn move through generic LocalMatch
    let chess_layout = ChessLayout::from_viewport(frame.viewport());
    let mut chess = chess_match();
    chess.local_mut().set_viewport(frame.viewport());
    chess.advance_frame(&frame).expect("frame time is accepted");
    chess
        .handle_presentation_input(&chess_click(chess_layout, 12), &frame)
        .expect("selection is local only");
    chess
        .handle_presentation_input(&chess_click(chess_layout, 28), &frame)
        .expect("chess move is accepted");

    assert_eq!(chess.recorded_inputs().len(), 1);
    assert_eq!(chess.recorded_inputs()[0].index, InputIndex(1));
    assert_eq!(chess.view().board[28].unwrap().kind, PieceKind::Pawn);

    // 2. TicTacToe drives mark placement through identical generic LocalMatch
    let ttt_layout = TttLayout::from_viewport(frame.viewport());
    let mut ttt = ttt_match(30_000);
    ttt.local_mut().set_viewport(frame.viewport());
    ttt.advance_frame(&frame).expect("frame is accepted");

    ttt.handle_presentation_input(&ttt_click(ttt_layout, 0), &frame)
        .expect("tictactoe placement is accepted");

    assert_eq!(ttt.recorded_inputs().len(), 1);
    assert_eq!(ttt.recorded_inputs()[0].index, InputIndex(1));
    assert_eq!(ttt.view().board[0], Some(Mark::X));
    assert_eq!(ttt.view().turn, SeatId(1));
}

#[test]
fn tictactoe_pointer_input_and_presentation_workflow() {
    let frame = frame(0);
    let layout = TttLayout::from_viewport(frame.viewport());
    let mut match_ = ttt_match(30_000);
    match_.local_mut().set_viewport(frame.viewport());
    match_.advance_frame(&frame).expect("frame is accepted");

    // Outside click consumes no input index
    let outside_event = InputEvent::Pointer {
        position: PointerPosition::new(Vec2::new(10.0, 10.0)).unwrap(),
        button: PointerButton::Primary,
        phase: PointerPhase::Up,
    };
    match_
        .handle_presentation_input(&outside_event, &frame)
        .expect("outside click produces no command");
    assert!(match_.recorded_inputs().is_empty());

    // First move: X plays cell 0
    match_
        .handle_presentation_input(&ttt_click(layout, 0), &frame)
        .expect("move 1 accepted");
    assert_eq!(match_.recorded_inputs().len(), 1);
    assert_eq!(match_.view().board[0], Some(Mark::X));
    assert_eq!(match_.view().turn, SeatId(1));

    // Switch viewer to seat 1 (hot-seat)
    match_.set_viewer(Viewer::Seat(SeatId(1)));
    assert_eq!(match_.viewer(), Viewer::Seat(SeatId(1)));

    // Second move: O plays cell 4
    match_
        .handle_presentation_input(&ttt_click(layout, 4), &frame)
        .expect("move 2 accepted");
    assert_eq!(match_.recorded_inputs().len(), 2);
    assert_eq!(match_.view().board[4], Some(Mark::O));
    assert_eq!(match_.view().turn, SeatId(0));
}

#[test]
fn tictactoe_timer_deadline_terminates_match_canonically() {
    let mut match_ = ttt_match(5_000);
    assert_eq!(match_.recorded_inputs().len(), 0);
    assert!(match_.ended().is_none());

    // Advance to deadline: 5000 ms
    let cues = match_
        .advance_frame(&frame(5_000))
        .expect("due timer executes");

    assert_eq!(match_.recorded_inputs().len(), 1);
    assert!(matches!(
        match_.recorded_inputs()[0],
        RecordedInput {
            input: Input::Timer { timer: TimerId(1) },
            now: LogicalTime(5_000),
            index: InputIndex(1),
        }
    ));
    assert!(match_.ended().is_some());
    assert!(matches!(
        match_.effects().last(),
        Some(LocalEffect::MatchEnded { .. })
    ));
    assert!(!cues.is_empty());
}

#[test]
fn tictactoe_presenter_produces_macroquad_supported_render_list() {
    let match_ = ttt_match(30_000);
    let frame = frame(0);
    assert_eq!(
        MacroquadRenderer::preflight(&match_.present(&frame), &frame),
        Ok(())
    );
}
