//! Integration tests for [`LocalMatch`] driving both Chess and Tiles.

#![allow(clippy::doc_markdown)]

use glam::Vec2;
use renderer_macroquad::MacroquadRenderer;
use tabula_core::{
    InputIndex, LogicalTime, MatchSeed, Millis, Occupant, SeatEntry, SeatId, SeatRoster, TimerId,
    UserId, Viewer,
};
use tabula_game_api::GameModule;
use tabula_game_api::Input;
use tabula_game_chess::{
    presentation::{BoardLayout as ChessLayout, ChessPresentation},
    ChessRules, ClockConfig, ClockControl, Color, Config as ChessConfig, PieceKind, Square,
};
use tabula_game_client::{LocalEffect, LocalMatch, RecordedInput};
use tabula_game_tiles::{
    presentation::{world_rect as tiles_world_rect, TilesLocal, TilesPresentation},
    rules::legal_placements as tiles_legal_placements,
    Config as TilesConfig, Coord as TilesCoord, Status as TilesStatus, TilesModule, TilesRules,
    TurnPhase as TilesTurnPhase,
};
use tabula_presentation::{
    Dpi, FrameCtx, InputEvent, Key, PointerButton, PointerPhase, PointerPosition, Viewport,
};

type ChessMatch = LocalMatch<ChessRules, ChessPresentation>;

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

fn chess_match_with_clock(initial_ms: u64) -> ChessMatch {
    ChessMatch::new(
        &ChessConfig {
            clock: Some(ClockConfig {
                initial: Millis(initial_ms),
                control: ClockControl::Fischer {
                    increment: Millis::ZERO,
                },
            }),
        },
        &roster(),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("standard local chess configuration is valid")
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

#[test]
fn shared_runtime_drives_both_chess_and_tiles_without_game_specific_branches() {
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

    // 2. Tiles drives tile placement through identical generic LocalMatch
    let mut tiles = tiles_match(2);
    tiles.local_mut().set_viewport(frame.viewport());
    tiles.advance_frame(&frame).expect("frame is accepted");

    let kind = tiles.view().drawn.expect("tiles match drawn tile");
    let (coord, rotations) = tiles_legal_placements(&tiles.view().board, kind)
        .first()
        .cloned()
        .expect("legal placement available");
    for _ in 0..4 {
        if rotations.contains(&tiles.local_mut().preview_rotation()) {
            break;
        }
        tiles
            .handle_presentation_input(&tiles_key(Key::Space), &frame)
            .expect("rotating is local only");
    }
    let event = tiles_click(tiles.local_mut(), coord);
    tiles
        .handle_presentation_input(&event, &frame)
        .expect("tiles placement is accepted");

    assert_eq!(tiles.recorded_inputs().len(), 1);
    assert_eq!(tiles.recorded_inputs()[0].index, InputIndex(1));
    assert_eq!(tiles.view().last_placed, Some(coord));
}

#[test]
fn chess_pointer_input_and_presentation_workflow() {
    let frame = frame(0);
    let layout = ChessLayout::from_viewport(frame.viewport());
    let mut match_ = chess_match();
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

    // First move: White plays e2-e4 (square 12 to 28)
    match_
        .handle_presentation_input(&chess_click(layout, 12), &frame)
        .expect("selection is local only");
    match_
        .handle_presentation_input(&chess_click(layout, 28), &frame)
        .expect("move 1 accepted");
    assert_eq!(match_.recorded_inputs().len(), 1);
    assert_eq!(match_.view().board[28].unwrap().kind, PieceKind::Pawn);
    assert_eq!(match_.view().turn, Color::Black);

    // Switch viewer to seat 1 (hot-seat)
    match_.set_viewer(Viewer::Seat(SeatId(1)));
    assert_eq!(match_.viewer(), Viewer::Seat(SeatId(1)));

    // Second move: Black plays e7-e5 (square 52 to 36)
    match_
        .handle_presentation_input(&chess_click(layout, 52), &frame)
        .expect("selection is local only");
    match_
        .handle_presentation_input(&chess_click(layout, 36), &frame)
        .expect("move 2 accepted");
    assert_eq!(match_.recorded_inputs().len(), 2);
    assert_eq!(match_.view().board[36].unwrap().kind, PieceKind::Pawn);
    assert_eq!(match_.view().turn, Color::White);
}

#[test]
fn chess_timer_deadline_terminates_match_canonically() {
    let mut match_ = chess_match_with_clock(5_000);
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
fn chess_presenter_produces_macroquad_supported_render_list() {
    let match_ = chess_match();
    let frame = frame(0);
    assert_eq!(
        MacroquadRenderer::preflight(&match_.present(&frame), &frame),
        Ok(())
    );
}

// ---------------------------------------------------------------------------
// Tiles — the Phase 3 vertical slice, driven end to end through the same
// generic `LocalMatch` the two Phase 2 games use.
// ---------------------------------------------------------------------------

type TilesMatch = LocalMatch<TilesRules, TilesPresentation>;

fn tiles_roster(seats: u8) -> SeatRoster {
    SeatRoster::new(
        (0..seats)
            .map(|index| SeatEntry {
                seat: SeatId(index),
                occupant: Occupant::Human(UserId(u128::from(index) + 1)),
                team: None,
            })
            .collect(),
    )
    .expect("local seats are unique")
}

fn tiles_match(seats: u8) -> TilesMatch {
    TilesMatch::new(
        &TilesConfig {
            turn_deadline_ms: 0,
        },
        &tiles_roster(seats),
        MatchSeed::from_bytes([9; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("the local tiles configuration is valid")
}

/// A click on the centre of a board square, in the coordinates that square
/// currently occupies on screen.
fn tiles_click(local: &TilesLocal, coord: TilesCoord) -> InputEvent {
    let rect = tiles_world_rect(coord);
    let world = rect.origin() + rect.size() * 0.5;
    let screen = (world - local.camera().origin()) * local.camera().zoom();
    InputEvent::Pointer {
        position: PointerPosition::new(screen).expect("test pointer is finite"),
        button: PointerButton::Primary,
        phase: PointerPhase::Up,
    }
}

fn tiles_key(key: Key) -> InputEvent {
    InputEvent::Key { key, pressed: true }
}

/// **The Phase 3 acceptance test.** A whole Tiles match is played from match
/// creation to `EndMatch` using only what a real client has: normalized
/// pointer and keyboard input, the presenter, and the generic runtime.
///
/// Nothing here reaches for canonical state. Every decision is taken from the
/// projection the presenter was handed, which is what makes this evidence that
/// the game is *playable* rather than merely that `apply` accepts commands.
#[test]
fn a_whole_tiles_match_is_playable_through_pointer_and_keyboard_input() {
    let frame = frame(0);
    let mut match_ = tiles_match(3);
    match_.local_mut().set_viewport(frame.viewport());
    match_
        .advance_frame(&frame)
        .expect("frame time is accepted");

    let mut placements = 0usize;
    let mut claims = 0usize;
    let mut passes = 0usize;

    for _ in 0..400 {
        if match_.ended().is_some() {
            break;
        }
        // Hot seat: the runtime shows whoever is on turn, exactly as the shell
        // does.
        let on_turn = match_.view().turn;
        match_.set_viewer(Viewer::Seat(on_turn));

        match match_.view().phase {
            TilesTurnPhase::PlaceTile => {
                let kind = match_.view().drawn.expect("a playing match holds a tile");
                let options = tiles_legal_placements(&match_.view().board, kind);
                let (coord, rotations) = options
                    .first()
                    .cloned()
                    .expect("a playing match always has somewhere to play");

                // Rotate with the keyboard until the preview matches, then
                // click the square — the real interaction, not a shortcut.
                for _ in 0..4 {
                    if rotations.contains(&match_.local_mut().preview_rotation()) {
                        break;
                    }
                    match_
                        .handle_presentation_input(&tiles_key(Key::Space), &frame)
                        .expect("rotating is local only");
                }
                let event = tiles_click(match_.local_mut(), coord);
                match_
                    .handle_presentation_input(&event, &frame)
                    .expect("a legal placement is accepted");
                placements += 1;
                assert_eq!(
                    match_.view().last_placed,
                    Some(coord),
                    "the projection must show the tile the click placed"
                );
            }
            TilesTurnPhase::PlaceMeeple => {
                // Claim two turns out of three and pass on the third, so both
                // paths are exercised. The rules never *offer* the claim step
                // with no slots — a seat out of followers has its turn ended by
                // the placement — so a driver that always claimed would leave
                // the pass path untested.
                let claim = (claims + passes) % 3 != 2 && !match_.view().meeple_slots.is_empty();
                if claim {
                    let last = match_.view().last_placed.expect("a tile was just placed");
                    let event = tiles_click(match_.local_mut(), last);
                    match_
                        .handle_presentation_input(&event, &frame)
                        .expect("clicking a claim slot claims it");
                    claims += 1;
                } else {
                    match_
                        .handle_presentation_input(&tiles_key(Key::Escape), &frame)
                        .expect("passing is always available in the claim step");
                    passes += 1;
                }
            }
        }
    }

    let outcome = match_.ended().expect("the match reached a terminal state");
    assert_eq!(placements, match_.view().board.len() - 1);
    assert!(claims > 0, "no follower was ever claimed through the UI");
    assert!(passes > 0, "the pass path was never exercised");
    assert_eq!(match_.view().bag_remaining, 0);
    assert_eq!(match_.view().status, TilesStatus::Ended);

    // Somebody won on points, and the standings say so.
    assert_eq!(outcome.standings().len(), 3);
    assert!(
        match_.view().scores.values().any(|score| *score > 0),
        "a whole match that scored nothing is not a playable game"
    );
    for standing in outcome.standings() {
        assert_eq!(
            standing.score,
            match_
                .view()
                .scores
                .get(&standing.seat)
                .copied()
                .unwrap_or(0)
        );
    }

    // Replay evidence was collected for every accepted input, and only those.
    let trace = match_.replay_trace();
    assert_eq!(
        trace.accepted_inputs().len(),
        match_.recorded_inputs().len()
    );
    assert!(trace.accepted_inputs().len() > 100);
    assert!(matches!(
        match_.effects().last(),
        Some(LocalEffect::MatchEnded { .. })
    ));
}

/// The camera is presentation-local, driven through the runtime rather than by
/// poking `Local` directly: panning and zooming produce no canonical input at
/// all, so no index is consumed and the state hash does not move.
#[test]
fn panning_and_zooming_through_the_runtime_consume_no_canonical_input() {
    let frame = frame(0);
    let mut match_ = tiles_match(2);
    match_.local_mut().set_viewport(frame.viewport());
    match_.advance_frame(&frame).expect("frame is accepted");
    let before = match_.view().clone();
    let camera_before = match_.local_mut().camera();

    let drag = |point: Vec2, phase: PointerPhase| InputEvent::Pointer {
        position: PointerPosition::new(point).expect("finite"),
        button: PointerButton::Primary,
        phase,
    };
    for event in [
        drag(Vec2::new(400.0, 300.0), PointerPhase::Down),
        drag(Vec2::new(300.0, 380.0), PointerPhase::Move),
        drag(Vec2::new(200.0, 460.0), PointerPhase::Move),
        drag(Vec2::new(200.0, 460.0), PointerPhase::Up),
    ] {
        match_
            .handle_presentation_input(&event, &frame)
            .expect("panning is local only");
    }

    assert!(
        match_.recorded_inputs().is_empty(),
        "a pan must not enter the canonical input stream"
    );
    assert_ne!(
        match_.local_mut().camera().origin(),
        camera_before.origin(),
        "the drag did not actually pan, so this proves nothing"
    );
    assert_eq!(
        &before,
        match_.view(),
        "the projection changed although no canonical input was applied"
    );
}

#[test]
fn the_tiles_presenter_produces_a_macroquad_supported_render_list() {
    let frame = frame(0);
    let mut match_ = tiles_match(4);
    match_.local_mut().set_viewport(frame.viewport());
    assert_eq!(
        MacroquadRenderer::preflight(&match_.present(&frame), &frame),
        Ok(())
    );

    // And after a real turn, when followers, hints, and the claim overlay are
    // all on screen at once.
    match_.advance_frame(&frame).expect("frame is accepted");
    let kind = match_.view().drawn.expect("a tile is in hand");
    let (coord, rotations) = tiles_legal_placements(&match_.view().board, kind)
        .first()
        .cloned()
        .expect("something is playable");
    for _ in 0..4 {
        if rotations.contains(&match_.local_mut().preview_rotation()) {
            break;
        }
        match_
            .handle_presentation_input(&tiles_key(Key::Space), &frame)
            .expect("rotating is local only");
    }
    let event = tiles_click(match_.local_mut(), coord);
    match_
        .handle_presentation_input(&event, &frame)
        .expect("a legal placement is accepted");
    assert_eq!(
        MacroquadRenderer::preflight(&match_.present(&frame), &frame),
        Ok(())
    );
}

/// Bot seats reach `apply` through the ordinary player path, so a solo local
/// match progresses without a human touching the other seats.
#[test]
fn a_local_bot_can_take_the_seats_nobody_is_sitting_at() {
    let frame = frame(0);
    let mut match_ = tiles_match(3);
    match_.local_mut().set_viewport(frame.viewport());
    match_.advance_frame(&frame).expect("frame is accepted");

    let bot = TilesModule::bot(tabula_core::BotLevel::Easy).expect("tiles offers an easy bot");
    let mut rng = tabula_core::DetRng::for_input(&MatchSeed::from_bytes([9; 32]), InputIndex(999));

    for _ in 0..400 {
        if match_.ended().is_some() {
            break;
        }
        let seat = match_.view().turn;
        match_.set_viewer(Viewer::Seat(seat));
        let Some(command) = bot.choose(match_.view(), seat, &mut rng) else {
            break;
        };
        match_
            .submit_bot_move(seat, command, &frame)
            .expect("the bot only proposes commands its own projection allows");
    }

    assert!(
        match_.ended().is_some(),
        "a bot-driven local match must reach a terminal state"
    );
    assert_eq!(match_.view().bag_remaining, 0);
}
