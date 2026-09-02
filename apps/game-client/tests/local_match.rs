//! Integration tests for [`LocalMatch`] driving both Chess and Tic-tac-toe.

#![allow(clippy::doc_markdown)]

use glam::Vec2;
use renderer_macroquad::MacroquadRenderer;
use tabula_core::{
    DetRng, InputIndex, LogicalTime, MatchSeed, Millis, Occupant, RuleError, SeatEntry, SeatId,
    SeatRoster, StateHash, TimerId, UserId, Viewer,
};
use tabula_game_api::{
    Budget, ChatScopes, CheckpointLabel, Ctx, Effect, GameRules, Input, Notice, VoiceScopes,
};
use tabula_game_chess::{
    presentation::{BoardLayout, ChessPresentation},
    ChessRules, ClockConfig, ClockControl, Config, PieceKind, Square,
};
use tabula_game_client::{LocalEffect, LocalMatch, LocalMatchError, RecordedInput};
use tabula_game_tictactoe::{
    presentation::{BoardLayout as TttLayout, TicTacToePresentation},
    Config as TttConfig, Mark, TicTacToeRules,
};
use tabula_presentation::{
    AudioCue, AudioSink, Dpi, FrameCtx, InputEvent, PointerButton, PointerPhase, PointerPosition,
    Viewport,
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

fn match_for_rules(config: &Config) -> ChessMatch {
    ChessMatch::new(
        config,
        &roster(),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("standard local rules configuration is valid")
}

fn timed_config(initial: u64) -> Config {
    Config {
        clock: Some(ClockConfig {
            initial: Millis(initial),
            control: ClockControl::Fischer {
                increment: Millis::ZERO,
            },
        }),
    }
}

fn reference_timeout_hash(deadline: LogicalTime) -> StateHash {
    let seed = MatchSeed::from_bytes([0; 32]);
    let config = timed_config(10);
    let mut create_rng = DetRng::for_input(&seed, InputIndex(0));
    let mut create_ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut create_rng,
        budget: Budget::default(),
    };
    let mut state = ChessRules::create(&config, &roster(), &mut create_ctx)
        .expect("clocked reference match is valid")
        .state;
    let mut timer_rng = DetRng::for_input(&seed, InputIndex(1));
    let mut timer_ctx = Ctx {
        now: deadline,
        index: InputIndex(1),
        rng: &mut timer_rng,
        budget: Budget::default(),
    };
    ChessRules::apply(
        &mut state,
        Input::Timer { timer: TimerId(1) },
        &mut timer_ctx,
    )
    .expect("the reference timer is accepted");
    ChessRules::state_hash(&state)
}

fn click(layout: BoardLayout, square: u8) -> InputEvent {
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

fn prepare(match_: &mut ChessMatch, frame: &FrameCtx) {
    match_.local_mut().set_viewport(frame.viewport());
    match_.advance_frame(frame).expect("frame time is accepted");
}

#[test]
fn presenter_command_goes_through_game_rules_apply() {
    let frame = frame(0);
    let layout = BoardLayout::from_viewport(frame.viewport());
    let mut match_ = match_for_rules(&Config::default());
    prepare(&mut match_, &frame);

    match_
        .handle_presentation_input(&click(layout, 12), &frame)
        .expect("selection is local only");
    match_
        .handle_presentation_input(&click(layout, 28), &frame)
        .expect("legal move is accepted");

    assert_eq!(match_.view().board[12], None);
    assert_eq!(match_.view().board[28].unwrap().kind, PieceKind::Pawn);
    assert_eq!(match_.recorded_inputs().len(), 1);
    assert_eq!(match_.recorded_inputs()[0].index, InputIndex(1));
}

#[test]
fn ui_only_interaction_consumes_no_canonical_input_index() {
    let frame = frame(0);
    let layout = BoardLayout::from_viewport(frame.viewport());
    let mut match_ = match_for_rules(&Config::default());
    prepare(&mut match_, &frame);

    match_
        .handle_presentation_input(&click(layout, 12), &frame)
        .expect("selection has no command");
    assert!(match_.recorded_inputs().is_empty());
}

#[test]
fn accepted_and_rejected_inputs_each_consume_one_index() {
    let frame = frame(0);
    let layout = BoardLayout::from_viewport(frame.viewport());
    let mut match_ = match_for_rules(&Config::default());
    prepare(&mut match_, &frame);

    match_
        .handle_presentation_input(&click(layout, 12), &frame)
        .expect("selection is local only");
    let rejected = match_
        .handle_presentation_input(&click(layout, 36), &frame)
        .expect_err("illegal move must be propagated");
    assert!(matches!(
        rejected,
        LocalMatchError::Rejected(RuleError {
            code: tabula_core::RuleErrorCode::IllegalMove,
            ..
        })
    ));
    match_
        .handle_presentation_input(&click(layout, 12), &frame)
        .expect("selection remains local only");
    match_
        .handle_presentation_input(&click(layout, 28), &frame)
        .expect("legal move is accepted");

    assert_eq!(
        match_
            .recorded_inputs()
            .iter()
            .map(|entry| entry.index)
            .collect::<Vec<_>>(),
        [InputIndex(1), InputIndex(2)]
    );
}

#[test]
fn exhausted_input_index_never_wraps_or_reuses_rng_domain() {
    let frame = frame(0);
    let layout = BoardLayout::from_viewport(frame.viewport());
    let mut match_ = match_for_rules(&Config::default());
    match_.set_next_input_index_for_test(Some(InputIndex(u64::MAX)));
    prepare(&mut match_, &frame);

    match_
        .handle_presentation_input(&click(layout, 12), &frame)
        .expect("selection is local only");
    match_
        .handle_presentation_input(&click(layout, 28), &frame)
        .expect("maximum input index is usable once");
    match_.set_viewer(Viewer::Seat(SeatId(1)));
    match_.local_mut().set_viewport(frame.viewport());
    match_
        .handle_presentation_input(&click(layout, 52), &frame)
        .expect("selection remains local after exhaustion");
    assert!(matches!(
        match_.handle_presentation_input(&click(layout, 36), &frame),
        Err(LocalMatchError::InputIndexExhausted)
    ));
}

#[test]
fn logical_time_is_monotonic_for_stalled_and_backwards_frames() {
    let mut match_ = match_for_rules(&Config::default());
    match_.advance_frame(&frame(12)).expect("time advances");
    match_
        .advance_frame(&frame(12))
        .expect("stalled time is valid");
    match_
        .advance_frame(&frame(7))
        .expect("backwards time clamps");
    assert_eq!(match_.now(), LogicalTime(12));
}

#[test]
fn clock_timer_reenters_rules_as_a_recorded_timer_input() {
    let mut match_ = match_for_rules(&timed_config(10));
    match_
        .advance_frame(&frame(10))
        .expect("due clock timer fires");

    assert!(matches!(
        match_.recorded_inputs(),
        [RecordedInput {
            input: Input::Timer { .. },
            now: LogicalTime(10),
            ..
        }]
    ));
    assert!(match_.ended().is_some());
    assert!(matches!(
        match_.effects().last(),
        Some(LocalEffect::MatchEnded { .. })
    ));
}

#[test]
fn late_frame_replays_the_timer_at_its_deadline_not_frame_arrival() {
    let mut match_ = match_for_rules(&timed_config(10));
    match_
        .advance_frame(&frame(20))
        .expect("late frame processes the due timer");

    assert!(matches!(
        match_.recorded_inputs(),
        [RecordedInput {
            input: Input::Timer { timer: TimerId(1) },
            now: LogicalTime(10),
            ..
        }]
    ));
    assert_eq!(match_.now(), LogicalTime(20));
    assert_eq!(match_.state_hash(), reference_timeout_hash(LogicalTime(10)));
}

#[test]
fn public_bot_entry_cannot_overtake_a_due_timer() {
    let frame = frame(10);
    let mut match_ = match_for_rules(&timed_config(10));

    assert!(matches!(
        match_.submit_bot_move(
            SeatId(0),
            tabula_game_chess::Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
            &frame,
        ),
        Err(LocalMatchError::MatchEnded)
    ));
    assert!(matches!(
        match_.recorded_inputs(),
        [RecordedInput {
            input: Input::Timer { timer: TimerId(1) },
            now: LogicalTime(10),
            ..
        }]
    ));
}

#[test]
fn public_input_entry_cannot_overtake_a_due_timer() {
    let frame = frame(10);
    let mut match_ = match_for_rules(&timed_config(10));

    assert!(matches!(
        match_.submit_input(
            Input::Player {
                seat: SeatId(0),
                command: tabula_game_chess::Command::Move {
                    from: 12,
                    to: 28,
                    promotion: None,
                },
            },
            &frame,
        ),
        Err(LocalMatchError::MatchEnded)
    ));
    assert_eq!(match_.recorded_inputs().len(), 1);
    assert!(matches!(
        match_.recorded_inputs()[0],
        RecordedInput {
            input: Input::Timer { timer: TimerId(1) },
            now: LogicalTime(10),
            ..
        }
    ));
}

#[test]
fn cancellation_rearm_and_simultaneous_timer_order_are_stable() {
    let first_frame = frame(10);
    let mut match_ = match_for_rules(&Config::default());
    match_
        .interpret_effects(&[
            Effect::SetTimer {
                id: TimerId(2),
                delay: Millis(10),
            },
            Effect::SetTimer {
                id: TimerId(1),
                delay: Millis(10),
            },
            Effect::SetTimer {
                id: TimerId(3),
                delay: Millis(15),
            },
            Effect::CancelTimer { id: TimerId(3) },
            Effect::SetTimer {
                id: TimerId(3),
                delay: Millis(20),
            },
        ])
        .expect("effect sequence is valid");
    match_
        .advance_frame(&first_frame)
        .expect("first timer fires");
    match_
        .advance_frame(&frame(20))
        .expect("re-armed timer fires");

    assert_eq!(
        match_
            .recorded_inputs()
            .iter()
            .filter_map(|entry| match entry.input {
                Input::Timer { timer } => Some((timer, entry.now)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            (TimerId(1), LogicalTime(10)),
            (TimerId(2), LogicalTime(10)),
            (TimerId(3), LogicalTime(20)),
        ]
    );
}

#[test]
fn every_effect_has_an_explicit_local_interpretation_in_emitted_order() {
    let mut match_ = match_for_rules(&Config::default());
    let notice = Notice {
        key: "local.notice".into(),
        #[allow(clippy::default_trait_access)]
        args: Default::default(),
    };
    match_
        .interpret_effects(&[
            Effect::SetTimer {
                id: TimerId(3),
                delay: Millis(1),
            },
            Effect::CancelTimer { id: TimerId(3) },
            Effect::RequestBotMove {
                seat: SeatId(0),
                deadline: Millis(2),
            },
            Effect::Notify {
                audience: tabula_core::Audience::Everyone,
                notice: notice.clone(),
            },
            Effect::SetChatScopes(ChatScopes::default()),
            Effect::SetVoiceScopes(VoiceScopes::default()),
            Effect::Checkpoint {
                label: CheckpointLabel("local".into()),
            },
        ])
        .expect("effect sequence is valid");

    assert!(matches!(
        match_.effects(),
        [
            LocalEffect::TimerSet { .. },
            LocalEffect::TimerCancelled { .. },
            LocalEffect::BotRequested(_),
            LocalEffect::Notified { .. },
            LocalEffect::ChatScopesIgnored,
            LocalEffect::VoiceScopesIgnored,
            LocalEffect::CheckpointIgnored,
        ]
    ));
    assert_eq!(match_.drain_bot_requests().count(), 1);
    let notices = match_.drain_notices().collect::<Vec<_>>();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].notice.key, notice.key);
}

#[test]
fn duplicate_end_match_is_rejected_without_overwriting_the_first_outcome() {
    let mut match_ = match_for_rules(&timed_config(10));
    match_
        .advance_frame(&frame(10))
        .expect("clock timer ends the match");
    let first = match_
        .ended()
        .expect("first terminal outcome exists")
        .clone();

    assert!(matches!(
        match_.interpret_effects(&[Effect::EndMatch {
            outcome: first.clone(),
        }]),
        Err(LocalMatchError::MultipleEndMatch)
    ));
    assert_eq!(
        match_.ended().expect("first outcome remains").summary(),
        first.summary()
    );
}

#[test]
fn locally_executed_bot_move_reuses_the_player_input_path() {
    let frame = frame(0);
    let mut match_ = match_for_rules(&Config::default());
    let command = tabula_game_chess::Command::Move {
        from: 12,
        to: 28,
        promotion: None,
    };
    match_
        .submit_bot_move(SeatId(0), command, &frame)
        .expect("bot command uses ordinary rules path");
    assert!(matches!(
        match_.recorded_inputs()[0].input,
        Input::Player {
            seat: SeatId(0),
            ..
        }
    ));
}

#[test]
fn projection_is_rebuilt_from_state_after_accepted_transition() {
    let frame = frame(0);
    let mut match_ = match_for_rules(&Config::default());
    match_
        .submit_bot_move(
            SeatId(0),
            tabula_game_chess::Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
            &frame,
        )
        .expect("move is legal");
    assert_eq!(match_.view().board[12], None);
    assert_eq!(match_.view().board[28].unwrap().kind, PieceKind::Pawn);
}

#[test]
fn presenter_produces_a_macroquad_supported_render_list() {
    let match_ = match_for_rules(&Config::default());
    let frame = frame(0);
    assert_eq!(
        MacroquadRenderer::preflight(&match_.present(&frame), &frame),
        Ok(())
    );
}

#[test]
fn audio_sink_failure_cannot_undo_an_accepted_move() {
    struct UnavailableSink;
    impl AudioSink for UnavailableSink {
        type Error = ();
        fn play(&mut self, _: &AudioCue) -> Result<(), Self::Error> {
            Err(())
        }
    }

    let frame = frame(0);
    let mut match_ = match_for_rules(&Config::default());
    let cues = match_
        .submit_bot_move(
            SeatId(0),
            tabula_game_chess::Command::Move {
                from: 12,
                to: 28,
                promotion: None,
            },
            &frame,
        )
        .expect("move is accepted before audio playback");
    let mut sink = UnavailableSink;
    for cue in &cues {
        assert_eq!(sink.play(cue), Err(()));
    }
    assert_eq!(match_.view().board[12], None);
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
    let chess_layout = BoardLayout::from_viewport(frame.viewport());
    let mut chess = match_for_rules(&Config::default());
    prepare(&mut chess, &frame);
    chess
        .handle_presentation_input(&click(chess_layout, 12), &frame)
        .expect("selection is local only");
    chess
        .handle_presentation_input(&click(chess_layout, 28), &frame)
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
