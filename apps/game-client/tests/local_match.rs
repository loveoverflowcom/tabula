//! Integration tests for [`LocalMatch`] driving both Chess and Tiles.

#![allow(clippy::doc_markdown)]

use glam::Vec2;
use tabula_core::{
    InputIndex, LogicalTime, MatchSeed, Millis, Occupant, RuleError, RuleErrorCode, SeatEntry,
    SeatId, SeatRoster, TimerId, UserId, Viewer,
};
use tabula_game_api::GameModule;
use tabula_game_api::Input;
use tabula_game_chess::{
    presentation::{BoardLayout as ChessLayout, ChessPresentation},
    ChessRules, ClockConfig, ClockControl, Color, Config as ChessConfig, PieceKind, Square,
};
use tabula_game_client::{
    runtime_ui::{FeedbackInput, LocalFeedback},
    LocalEffect, LocalMatch, LocalMatchError, RecordedInput,
};
use tabula_game_tiles::{
    presentation::{world_rect as tiles_world_rect, TilesLocal, TilesPresentation},
    rules::legal_placements as tiles_legal_placements,
    Config as TilesConfig, Coord as TilesCoord, Status as TilesStatus, TilesModule, TilesRules,
    TurnPhase as TilesTurnPhase,
};
use tabula_presentation::{
    Dpi, FrameCtx, InputEvent, Key, PointerButton, PointerPhase, PointerPosition, RenderCmd,
    Viewport,
};
use tabula_render_macroquad::MacroquadRenderer;

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
        tiles_press_key(&mut tiles, Key::Space, &frame);
    }
    tiles_tap(&mut tiles, coord, &frame);

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
fn terminal_local_match_offers_restart_without_changing_its_result_or_trace() {
    let frame = frame(10);
    let mut match_ = chess_match_with_clock(10);
    match_.advance_frame(&frame).unwrap();
    let outcome = match_.ended().unwrap().clone();
    let view = match_.view().clone();
    let accepted = match_.replay_trace().accepted_inputs().len();
    let mut feedback = LocalFeedback::default();

    assert_eq!(
        feedback.route_input(&mut match_, &tiles_key(Key::Tab), &frame),
        FeedbackInput::Consumed
    );
    assert!(feedback.present(&frame).unwrap().commands().iter().any(
        |command| matches!(command, RenderCmd::Text { text, .. } if text == "Local game ended")
    ));
    assert_eq!(
        feedback.route_input(&mut match_, &tiles_key(Key::Enter), &frame),
        FeedbackInput::NewLocalGame
    );
    assert_eq!(match_.ended(), Some(&outcome));
    assert_eq!(match_.view().board, view.board);
    assert_eq!(match_.view().status, view.status);
    assert_eq!(match_.view().turn, view.turn);
    assert_eq!(match_.replay_trace().accepted_inputs().len(), accepted);
    assert_eq!(match_.recorded_inputs().len(), 1);
}

#[test]
fn completion_repeats_and_interrupted_restart_require_a_fresh_activation() {
    let frame = frame(10);
    let mut match_ = chess_match_with_clock(10);
    match_.advance_frame(&frame).unwrap();
    let mut feedback = LocalFeedback::default();
    feedback.sync_match(&match_);
    // Simulate the key that produced the terminal effect, as the real loop does.
    feedback.suppress_opening_input(&tiles_key(Key::Enter));
    feedback.route_input(&mut match_, &tiles_key(Key::Tab), &frame);
    for _ in 0..3 {
        feedback.sync_match(&match_);
        assert_eq!(
            feedback.route_input(&mut match_, &tiles_key(Key::Enter), &frame),
            FeedbackInput::Consumed
        );
    }
    assert_eq!(
        feedback.route_input(&mut match_, &tiles_key(Key::Escape), &frame),
        FeedbackInput::PassThrough
    );
    feedback.sync_match(&match_);
    assert_eq!(feedback.message(), Some("Final result shown on board."));
    feedback.note_accepted_input();
    feedback.note_error(&LocalMatchError::MatchEnded);
    assert_eq!(feedback.message(), Some("Final result shown on board."));
    feedback.route_input(
        &mut match_,
        &InputEvent::Key {
            key: Key::Enter,
            pressed: false,
        },
        &frame,
    );
    feedback.route_input(&mut match_, &tiles_key(Key::Tab), &frame);
    feedback.sync_match(&match_);
    assert_eq!(
        feedback.route_input(&mut match_, &tiles_key(Key::Enter), &frame),
        FeedbackInput::NewLocalGame,
        "repeated completion sync must preserve keyboard focus"
    );
    assert_eq!(
        feedback.route_input(&mut match_, &tiles_key(Key::Enter), &frame),
        FeedbackInput::Consumed
    );

    let action_at = feedback
        .present(&frame)
        .unwrap()
        .commands()
        .iter()
        .find_map(|command| match command {
            RenderCmd::Text { text, at, .. } if text == "New local game" => Some(*at),
            _ => None,
        })
        .unwrap();
    for interruption in [
        feedback_pointer(Vec2::ZERO, PointerPhase::Move),
        feedback_pointer(action_at, PointerPhase::Cancel),
        InputEvent::Focus(false),
    ] {
        feedback.route_input(
            &mut match_,
            &feedback_pointer(action_at, PointerPhase::Down),
            &frame,
        );
        feedback.route_input(&mut match_, &interruption, &frame);
        feedback.route_input(&mut match_, &InputEvent::Focus(true), &frame);
        assert_ne!(
            feedback.route_input(
                &mut match_,
                &feedback_pointer(action_at, PointerPhase::Up),
                &frame
            ),
            FeedbackInput::NewLocalGame
        );
    }
    feedback.route_input(
        &mut match_,
        &feedback_pointer(action_at, PointerPhase::Down),
        &frame,
    );
    assert_eq!(
        feedback.route_input(
            &mut match_,
            &feedback_pointer(action_at, PointerPhase::Up),
            &frame
        ),
        FeedbackInput::NewLocalGame
    );
    assert_eq!(match_.recorded_inputs().len(), 1);
    assert_eq!(match_.replay_trace().accepted_inputs().len(), 1);
}

#[test]
fn completion_release_while_hidden_and_resize_during_press_do_not_leave_a_restart_latch() {
    let full = frame(10);
    let resized = FrameCtx::new(
        Viewport::new(Vec2::new(568.0, 320.0)).unwrap(),
        full.dpi(),
        full.now_ms(),
        full.theme(),
    );
    let tiny = FrameCtx::new(
        Viewport::new(Vec2::new(320.0, 320.0)).unwrap(),
        full.dpi(),
        full.now_ms(),
        full.theme(),
    );
    let mut match_ = chess_match_with_clock(10);
    match_.advance_frame(&full).unwrap();
    let mut feedback = LocalFeedback::default();
    assert!(feedback.sync_match(&match_));
    feedback.suppress_opening_input(&tiles_key(Key::Enter));
    feedback.route_input(
        &mut match_,
        &InputEvent::Key {
            key: Key::Enter,
            pressed: false,
        },
        &tiny,
    );
    feedback.route_input(&mut match_, &tiles_key(Key::Tab), &full);
    assert_eq!(
        feedback.route_input(&mut match_, &tiles_key(Key::Enter), &full),
        FeedbackInput::NewLocalGame
    );

    let action_at = |feedback: &LocalFeedback, frame: &FrameCtx| {
        feedback
            .present(frame)
            .unwrap()
            .commands()
            .iter()
            .find_map(|command| match command {
                RenderCmd::Text { text, at, .. } if text == "New local game" => Some(*at),
                _ => None,
            })
            .unwrap()
    };
    let old_at = action_at(&feedback, &full);
    let new_at = action_at(&feedback, &resized);
    feedback.route_input(
        &mut match_,
        &feedback_pointer(old_at, PointerPhase::Down),
        &full,
    );
    assert_ne!(
        feedback.route_input(
            &mut match_,
            &feedback_pointer(new_at, PointerPhase::Up),
            &resized
        ),
        FeedbackInput::NewLocalGame,
        "resizing the action cancels an earlier pointer press"
    );
    feedback.route_input(
        &mut match_,
        &feedback_pointer(new_at, PointerPhase::Down),
        &resized,
    );
    feedback.route_input(&mut match_, &tiles_key(Key::ArrowLeft), &tiny);
    assert_ne!(
        feedback.route_input(
            &mut match_,
            &feedback_pointer(new_at, PointerPhase::Up),
            &resized
        ),
        FeedbackInput::NewLocalGame
    );
    feedback.route_input(
        &mut match_,
        &feedback_pointer(new_at, PointerPhase::Down),
        &resized,
    );
    assert_eq!(
        feedback.route_input(
            &mut match_,
            &feedback_pointer(new_at, PointerPhase::Up),
            &resized
        ),
        FeedbackInput::NewLocalGame
    );
    assert_eq!(match_.recorded_inputs().len(), 1);
    assert_eq!(match_.replay_trace().accepted_inputs().len(), 1);
}

#[test]
fn completion_never_comes_from_a_rejection_and_reserves_the_final_chess_hud() {
    let mut match_ = chess_match_with_clock(10);
    let mut feedback = LocalFeedback::default();
    feedback.note_error(&LocalMatchError::Rejected(RuleError::code(
        RuleErrorCode::MatchOver,
    )));
    feedback.sync_match(&match_);
    assert!(feedback
        .present(&frame(0))
        .unwrap()
        .commands()
        .iter()
        .any(|command| matches!(command, RenderCmd::Text { text, .. } if text == "Move rejected")));
    assert!(feedback.allows_gameplay(false));
    match_.advance_frame(&frame(10)).unwrap();
    feedback.sync_match(&match_);

    for kind in [
        tabula_design::ThemeKind::Light,
        tabula_design::ThemeKind::Dark,
        tabula_design::ThemeKind::HighContrastLight,
        tabula_design::ThemeKind::HighContrastDark,
    ] {
        for size in [
            Vec2::new(320.0, 568.0),
            Vec2::new(568.0, 320.0),
            Vec2::new(900.0, 720.0),
        ] {
            let full = FrameCtx::new(
                Viewport::new(size).unwrap(),
                Dpi::new(2.0).unwrap(),
                10,
                tabula_design::Theme::by_kind(kind),
            );
            let board = feedback.board_frame(&full);
            match_.local_mut().set_viewport(board.viewport());
            assert_eq!(board.dpi(), full.dpi());
            assert_eq!(board.now_ms(), full.now_ms());
            assert_eq!(board.theme(), full.theme());
            let dock = feedback.present(&full).unwrap();
            assert_eq!(MacroquadRenderer::preflight(&dock, &full), Ok(()));
            let panel = dock
                .commands()
                .iter()
                .find_map(|command| match command {
                    RenderCmd::Rect {
                        rect,
                        fill: Some(tabula_presentation::Paint::Solid(color)),
                        ..
                    } if *color == full.theme().color.surface_container_high => Some(*rect),
                    _ => None,
                })
                .unwrap();
            let status = ChessLayout::from_viewport(board.viewport()).status();
            assert!(
                status.origin().y + status.size().y <= panel.origin().y
                    || status.origin().x + status.size().x <= panel.origin().x
            );
            assert!(match_.present(&board).commands().iter().any(
                |command| matches!(command, RenderCmd::Text { text, at, .. } if (text == "Black wins" || text == "Game over / Black wins") && status.contains(*at))
            ));
            assert!(dock.commands().iter().any(|command| matches!(command,
                RenderCmd::Rect { rect, fill: Some(tabula_presentation::Paint::Solid(color)), .. }
                    if *color == full.theme().color.primary && rect.size().min_element() >= 44.0)));
        }
    }
    let tiny = FrameCtx::new(
        Viewport::new(Vec2::new(320.0, 320.0)).unwrap(),
        Dpi::new(1.0).unwrap(),
        10,
        tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light),
    );
    assert_eq!(feedback.board_frame(&tiny), tiny);
    assert!(feedback.present(&tiny).unwrap().commands().is_empty());
    assert!(!feedback.allows_gameplay(true));
}

#[test]
fn chess_presenter_produces_macroquad_supported_render_list_with_verified_art() {
    use tabula_game_chess::presentation::assets;
    use tabula_render_macroquad::assets::{
        AssetCacheLimits, DecodedRaster, SpriteAssetCache, TextureUploader,
    };
    struct CpuUploader;
    impl TextureUploader for CpuUploader {
        type Texture = (u16, u16);
        fn upload(&mut self, image: &DecodedRaster) -> Result<Self::Texture, String> {
            Ok((image.width(), image.height()))
        }
    }
    let match_ = chess_match();
    let frame = frame(0);
    let scene = match_.present(&frame);
    assert!(matches!(
        MacroquadRenderer::preflight(&scene, &frame),
        Err(tabula_presentation::RenderError::Execution(_))
    ));
    let manifest = tabula_assets::AssetPackManifest::from_toml(assets::MANIFEST).unwrap();
    let mut cache = SpriteAssetCache::new(
        CpuUploader,
        AssetCacheLimits::new(
            320 * 1024,
            1024,
            256 * 1024,
            2 * 1024 * 1024,
            4 * 1024 * 1024,
            4,
        )
        .unwrap(),
    );
    cache
        .bind_pack(
            &manifest,
            &tabula_core::GameId::new("com.tabula.chess").unwrap(),
            &assets::asset_pack(),
        )
        .unwrap();
    for file in manifest.files() {
        let bytes = assets::ALL_IMAGES
            .iter()
            .find(|(name, _)| *name == file.name().as_str())
            .unwrap()
            .1;
        cache
            .insert_verified(
                file.verify_owned_bytes(tabula_assets::UnverifiedAssetBytes::new(bytes.to_vec()))
                    .unwrap(),
            )
            .unwrap();
    }
    assert_eq!(
        MacroquadRenderer::preflight_with_cache(&scene, &frame, &cache),
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

/// A complete physical tap, including the press required by interruption guards.
fn tiles_tap(match_: &mut TilesMatch, coord: TilesCoord, frame: &FrameCtx) {
    // Navigate the growing board through the actual keyboard path so the tap
    // remains visible and cannot land under an enlarged HUD or outside screen.
    for _ in 0..512 {
        let cursor = match_.local_mut().cursor();
        let key = if cursor.x() > coord.x() {
            Some(Key::ArrowLeft)
        } else if cursor.x() < coord.x() {
            Some(Key::ArrowRight)
        } else if cursor.y() > coord.y() {
            Some(Key::ArrowUp)
        } else if cursor.y() < coord.y() {
            Some(Key::ArrowDown)
        } else {
            None
        };
        let Some(key) = key else {
            break;
        };
        tiles_press_key(match_, key, frame);
    }
    assert_eq!(
        match_.local_mut().cursor(),
        coord,
        "the target must be reachable through navigation"
    );
    let InputEvent::Pointer {
        position, button, ..
    } = tiles_click(match_.local_mut(), coord)
    else {
        unreachable!("the helper always constructs a pointer");
    };
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        match_
            .handle_presentation_input(
                &InputEvent::Pointer {
                    position,
                    button,
                    phase,
                },
                frame,
            )
            .expect("the complete tap is accepted");
    }
}

/// Repeated actions need fresh physical presses, rather than repeated keydown.
fn tiles_press_key(match_: &mut TilesMatch, key: Key, frame: &FrameCtx) {
    for pressed in [true, false] {
        match_
            .handle_presentation_input(&InputEvent::Key { key, pressed }, frame)
            .expect("the physical key press is accepted");
    }
}

fn feedback_test_frame() -> FrameCtx {
    FrameCtx::new(
        Viewport::new(Vec2::new(320.0, 568.0)).unwrap(),
        Dpi::new(1.0).unwrap(),
        0,
        tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light),
    )
}

fn rejection_feedback() -> LocalFeedback {
    let mut feedback = LocalFeedback::default();
    feedback.note_error(&LocalMatchError::Rejected(RuleError::code(
        RuleErrorCode::IllegalMove,
    )));
    feedback
}

fn route_tiles_feedback(
    match_: &mut TilesMatch,
    feedback: &mut LocalFeedback,
    event: &InputEvent,
    frame: &FrameCtx,
) -> FeedbackInput {
    let routed = feedback.route_input(match_, event, frame);
    if routed == FeedbackInput::PassThrough {
        let board = feedback.board_frame(frame);
        match_.local_mut().set_viewport(board.viewport());
        match_.handle_presentation_input(event, &board).unwrap();
    }
    routed
}

fn feedback_pointer(at: Vec2, phase: PointerPhase) -> InputEvent {
    InputEvent::Pointer {
        position: PointerPosition::new(at).unwrap(),
        button: PointerButton::Primary,
        phase,
    }
}

#[test]
fn feedback_occlusion_cancels_tiles_drag_before_release_and_hover() {
    let frame = feedback_test_frame();
    let mut match_ = tiles_match(2);
    match_.local_mut().set_viewport(frame.viewport());
    let mut feedback = rejection_feedback();
    let view = match_.view().clone();
    let original = match_.local_mut().camera();
    let outside = Vec2::new(160.0, 400.0);
    let inside = Vec2::new(160.0, 284.0);
    assert_eq!(
        route_tiles_feedback(
            &mut match_,
            &mut feedback,
            &feedback_pointer(outside, PointerPhase::Down),
            &frame
        ),
        FeedbackInput::PassThrough
    );
    assert_eq!(
        route_tiles_feedback(
            &mut match_,
            &mut feedback,
            &feedback_pointer(inside, PointerPhase::Move),
            &frame
        ),
        FeedbackInput::CancelGameplayPointer
    );
    assert_eq!(
        route_tiles_feedback(
            &mut match_,
            &mut feedback,
            &feedback_pointer(inside, PointerPhase::Up),
            &frame
        ),
        FeedbackInput::Consumed
    );
    route_tiles_feedback(
        &mut match_,
        &mut feedback,
        &feedback_pointer(Vec2::new(170.0, 400.0), PointerPhase::Move),
        &frame,
    );
    assert_eq!(
        match_.local_mut().camera(),
        original,
        "released hover cannot pan an interrupted drag"
    );
    // A fresh real drag outside the banner remains usable.
    for event in [
        feedback_pointer(outside, PointerPhase::Down),
        feedback_pointer(Vec2::new(200.0, 410.0), PointerPhase::Move),
        feedback_pointer(Vec2::new(200.0, 410.0), PointerPhase::Up),
    ] {
        route_tiles_feedback(&mut match_, &mut feedback, &event, &frame);
    }
    assert_ne!(match_.local_mut().camera(), original);
    assert_eq!(match_.view(), &view);
    assert!(match_.recorded_inputs().is_empty());
    assert!(match_.replay_trace().accepted_inputs().is_empty());
}

#[test]
fn feedback_keyboard_focus_forwards_release_to_tiles_before_dismissal() {
    let frame = feedback_test_frame();
    let mut match_ = tiles_match(2);
    match_.local_mut().set_viewport(frame.viewport());
    let mut feedback = rejection_feedback();
    let view = match_.view().clone();
    let rotation = match_.local_mut().preview_rotation();
    route_tiles_feedback(&mut match_, &mut feedback, &tiles_key(Key::Space), &frame);
    let once = match_.local_mut().preview_rotation();
    assert_ne!(
        once, rotation,
        "the first physical Space rotates the preview"
    );
    route_tiles_feedback(&mut match_, &mut feedback, &tiles_key(Key::Tab), &frame);
    assert_eq!(
        route_tiles_feedback(
            &mut match_,
            &mut feedback,
            &InputEvent::Key {
                key: Key::Space,
                pressed: false
            },
            &frame
        ),
        FeedbackInput::PassThrough
    );
    route_tiles_feedback(&mut match_, &mut feedback, &tiles_key(Key::Escape), &frame);
    assert!(feedback.message().is_none());
    route_tiles_feedback(&mut match_, &mut feedback, &tiles_key(Key::Space), &frame);
    assert_ne!(
        match_.local_mut().preview_rotation(),
        once,
        "fresh Space works after banner dismissal"
    );
    assert_eq!(match_.view(), &view);
    assert!(match_.recorded_inputs().is_empty());
    assert!(match_.replay_trace().accepted_inputs().is_empty());
}

#[test]
fn feedback_keeps_pointer_capture_through_escape_dismissal() {
    let frame = feedback_test_frame();
    let mut match_ = tiles_match(2);
    match_.local_mut().set_viewport(frame.viewport());
    let mut feedback = rejection_feedback();
    let original = match_.local_mut().camera();
    route_tiles_feedback(
        &mut match_,
        &mut feedback,
        &feedback_pointer(Vec2::new(160.0, 284.0), PointerPhase::Down),
        &frame,
    );
    route_tiles_feedback(&mut match_, &mut feedback, &tiles_key(Key::Escape), &frame);
    assert!(feedback.message().is_none());
    for event in [
        feedback_pointer(Vec2::new(160.0, 400.0), PointerPhase::Move),
        feedback_pointer(Vec2::new(160.0, 400.0), PointerPhase::Up),
    ] {
        assert_eq!(
            route_tiles_feedback(&mut match_, &mut feedback, &event, &frame),
            FeedbackInput::Consumed
        );
    }
    assert_eq!(match_.local_mut().camera(), original);
    assert!(match_.recorded_inputs().is_empty());
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
                    tiles_press_key(&mut match_, Key::Space, &frame);
                }
                tiles_tap(&mut match_, coord, &frame);
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
                    tiles_tap(&mut match_, last, &frame);
                    claims += 1;
                } else {
                    tiles_press_key(&mut match_, Key::Escape, &frame);
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

    assert_terminal_tiles_inspection(&mut match_, &frame);
}

fn assert_terminal_tiles_inspection(match_: &mut TilesMatch, frame: &FrameCtx) {
    let cache = tiles_sprite_fixture_cache();
    // Ending authority does not disable local inspection. Exercise the real
    // labeled zoom control and a board pan after the final placement/claim.
    let terminal_view = match_.view().clone();
    let attempts = match_.recorded_inputs().len();
    let accepted = match_.replay_trace().accepted_inputs().len();
    let outcome = match_.ended().unwrap().clone();
    let mut feedback = LocalFeedback::default();
    feedback.sync_match(match_);
    let board_frame = feedback.board_frame(frame);
    match_.local_mut().set_viewport(board_frame.viewport());
    let camera_before = match_.local_mut().camera();
    let zoom_at = match_
        .present(&board_frame)
        .commands()
        .iter()
        .find_map(|command| match command {
            RenderCmd::Text { text, at, .. } if text == "Zoom in" => Some(*at),
            _ => None,
        })
        .expect("the terminal HUD retains a zoom control");
    let pointer = |at: Vec2, phase: PointerPhase| InputEvent::Pointer {
        position: PointerPosition::new(at).unwrap(),
        button: PointerButton::Primary,
        phase,
    };
    for event in [
        pointer(zoom_at, PointerPhase::Down),
        pointer(zoom_at, PointerPhase::Up),
    ] {
        assert_eq!(
            route_tiles_feedback(match_, &mut feedback, &event, frame),
            FeedbackInput::PassThrough
        );
    }
    assert_ne!(
        match_.local_mut().camera().zoom().to_bits(),
        camera_before.zoom().to_bits(),
        "zoom must actually change"
    );
    let zoomed = match_.local_mut().camera();
    for event in [
        pointer(Vec2::new(400.0, 300.0), PointerPhase::Down),
        pointer(Vec2::new(300.0, 380.0), PointerPhase::Move),
        pointer(Vec2::new(200.0, 460.0), PointerPhase::Up),
        tiles_key(Key::Enter),
        tiles_key(Key::Space),
    ] {
        assert_eq!(
            route_tiles_feedback(match_, &mut feedback, &event, frame),
            FeedbackInput::PassThrough
        );
    }
    assert_ne!(
        match_.local_mut().camera().origin(),
        zoomed.origin(),
        "pan must actually change"
    );
    assert_eq!(match_.view(), &terminal_view);
    assert_eq!(match_.recorded_inputs().len(), attempts);
    assert_eq!(match_.replay_trace().accepted_inputs().len(), accepted);
    assert_eq!(match_.ended(), Some(&outcome));

    for kind in [
        tabula_design::ThemeKind::Light,
        tabula_design::ThemeKind::Dark,
        tabula_design::ThemeKind::HighContrastLight,
        tabula_design::ThemeKind::HighContrastDark,
    ] {
        for size in [
            Vec2::new(320.0, 568.0),
            Vec2::new(568.0, 320.0),
            Vec2::new(900.0, 720.0),
        ] {
            let full = FrameCtx::new(
                Viewport::new(size).unwrap(),
                Dpi::new(1.0).unwrap(),
                0,
                tabula_design::Theme::by_kind(kind),
            );
            let board = feedback.board_frame(&full);
            match_.local_mut().set_viewport(board.viewport());
            assert!(board.viewport().size().y >= 320.0);
            let scene = match_.present(&board);
            let supported = MacroquadRenderer::preflight_with_cache(&scene, &full, &cache);
            assert_eq!(supported, Ok(()));
            for seat in &match_.view().seats {
                let score = match_.view().scores[seat];
                assert!(scene.commands().iter().any(|command| matches!(command,
                    RenderCmd::Text { text, .. } if text == &format!("S{}: {score}", seat.0) || text.starts_with(&format!("Seat {}: {score}", seat.0)))));
            }
            assert!(scene.commands().iter().any(|command| matches!(command,
                RenderCmd::Text { text, .. } if text.starts_with("Match over"))));
            assert_eq!(
                MacroquadRenderer::preflight(&feedback.present(&full).unwrap(), &full),
                Ok(())
            );
        }
    }
    assert_eq!(match_.view(), &terminal_view);
    assert_eq!(match_.recorded_inputs().len(), attempts);
    assert_eq!(match_.replay_trace().accepted_inputs().len(), accepted);
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

/// Exercises verified fixture bytes and real bounded PNG decoding with a context-free upload
/// adapter. This proves ready-resource acceptance; the runtime harness owns rendered pixel proof.
fn tiles_sprite_fixture_cache(
) -> tabula_render_macroquad::assets::SpriteAssetCache<impl tabula_render_macroquad::assets::TextureUploader>
{
    use tabula_assets::{AssetPackManifest, UnverifiedAssetBytes};
    use tabula_game_tiles::presentation::fixture;
    use tabula_presentation::GamePresentation;
    use tabula_render_macroquad::assets::{
        AssetCacheLimits, DecodedRaster, SpriteAssetCache, TextureUploader,
    };

    #[derive(Debug)]
    struct FixtureUploader;
    impl TextureUploader for FixtureUploader {
        type Texture = (u16, u16);
        fn upload(&mut self, raster: &DecodedRaster) -> Result<Self::Texture, String> {
            Ok((raster.width(), raster.height()))
        }
    }
    let manifest =
        AssetPackManifest::from_toml(fixture::MANIFEST).expect("fixture manifest is valid");
    let mut cache = SpriteAssetCache::new(FixtureUploader, AssetCacheLimits::default());
    cache
        .bind_pack(
            &manifest,
            TilesModule::metadata().id(),
            &TilesPresentation::asset_pack(),
        )
        .expect("fixture matches game and exact pack version");
    for file in manifest.files() {
        let bytes = match file
            .density()
            .expect("fixture variants declare density")
            .get()
        {
            1 => fixture::ATLAS_1X,
            2 => fixture::ATLAS_2X,
            other => panic!("fixture has no bundled density {other}"),
        };
        let verified = file
            .verify_owned_bytes(UnverifiedAssetBytes::new(bytes.to_vec()))
            .expect("fixture size and hash are exact");
        cache
            .insert_verified(verified)
            .expect("fixture decodes inside renderer bounds and atlas regions");
    }
    assert_eq!(cache.stats().uploads, 2);
    cache
}

#[test]
fn the_tiles_presenter_produces_a_macroquad_supported_render_list() {
    let frame = frame(0);
    let cache = tiles_sprite_fixture_cache();
    let initial_cache = cache.stats();
    let mut match_ = tiles_match(4);
    match_.local_mut().set_viewport(frame.viewport());
    for dpi in [1.0, 2.0, 3.0] {
        let frame = FrameCtx::new(
            frame.viewport(),
            Dpi::new(dpi).unwrap(),
            frame.now_ms(),
            frame.theme(),
        );
        let list = match_.present(&frame);
        assert!(
            list.commands()
                .iter()
                .any(|command| matches!(command, RenderCmd::Sprite { .. })),
            "the fixture must exercise Sprite acceptance"
        );
        assert_eq!(
            MacroquadRenderer::preflight_with_cache(&list, &frame, &cache),
            Ok(())
        );
    }

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
        tiles_press_key(&mut match_, Key::Space, &frame);
    }
    tiles_tap(&mut match_, coord, &frame);
    assert_eq!(
        MacroquadRenderer::preflight_with_cache(&match_.present(&frame), &frame, &cache),
        Ok(())
    );
    assert_eq!(
        cache.stats(),
        initial_cache,
        "complete-list preflight cannot decode or upload each frame"
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
