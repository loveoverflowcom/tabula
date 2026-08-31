//! Deterministic Chess clock transition evidence.

use smallvec::smallvec;
use tabula_core::{
    canonical_encode, AbortReason, DetRng, InputIndex, LogicalTime, MatchSeed, Millis, Occupant,
    OutcomeKind, SeatChange, SeatEntry, SeatId, SeatRoster, UserId,
};
use tabula_game_api::{AdminInput, Budget, Ctx, Effect, GameModule, GameRules, Input, Outcome};
use tabula_game_chess::{
    ChessModule, ChessRules, ClockConfig, ClockControl, ClockState, Color, Command, Config, Event,
    State, Status,
};

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

fn config(control: ClockControl) -> Config {
    Config {
        clock: Some(ClockConfig {
            initial: Millis(10_000),
            control,
        }),
    }
}

#[allow(clippy::needless_pass_by_value)] // Keeps inline clock configurations concise in tests.
fn create(config: Config, now: u64) -> State {
    let seed = MatchSeed::from_bytes([19; 32]);
    let mut rng = DetRng::for_input(&seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime(now),
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget::default(),
    };
    ChessRules::create(&config, &roster(), &mut ctx)
        .expect("valid clock config creates a match")
        .state
}

fn apply_at(
    state: &mut State,
    input: Input<Command>,
    now: u64,
    index: u64,
) -> Result<Outcome<ChessRules>, tabula_core::RuleError> {
    let seed = MatchSeed::from_bytes([23; 32]);
    let mut rng = DetRng::for_input(&seed, InputIndex(index));
    let mut ctx = Ctx {
        now: LogicalTime(now),
        index: InputIndex(index),
        rng: &mut rng,
        budget: Budget::default(),
    };
    ChessRules::apply(state, input, &mut ctx)
}

fn move_input(seat: u8, from: u8, to: u8) -> Input<Command> {
    Input::Player {
        seat: SeatId(seat),
        command: Command::Move {
            from,
            to,
            promotion: None,
        },
    }
}

fn timer_delay(outcome: &Outcome<ChessRules>) -> Millis {
    let [Effect::SetTimer { id, delay }] = outcome.effects.as_slice() else {
        panic!(
            "expected exactly one SetTimer effect, got {:?}",
            outcome.effects
        );
    };
    assert_eq!(*id, tabula_core::TimerId(1));
    *delay
}

fn assert_terminal_effects(outcome: &Outcome<ChessRules>) {
    assert_eq!(outcome.effects.len(), 2);
    assert!(matches!(
        &outcome.effects[0],
        Effect::CancelTimer {
            id: tabula_core::TimerId(1)
        }
    ));
    assert!(matches!(&outcome.effects[1], Effect::EndMatch { .. }));
}

fn assert_timeout(state: &State, flagged: Color) {
    let Status::Ended { outcome } = &state.status else {
        panic!("timeout must end the match");
    };
    assert_eq!(outcome.summary(), "timeout");
    assert_eq!(outcome.standings()[0].seat, flagged.other().seat());
    assert_eq!(
        state.clock.unwrap().remaining[flagged.seat().0 as usize],
        Millis::ZERO
    );
}

fn clocked_fen(fen: &str) -> State {
    let mut state = State::from_fen(fen).expect("clock fixture FEN is valid");
    state.clock = Some(ClockState {
        remaining: [Millis(10_000); 2],
        last_move_at: LogicalTime::ZERO,
        control: ClockControl::Fischer {
            increment: Millis(1_000),
        },
    });
    state
}

fn timeout_at_deadline(state: &mut State) -> Outcome<ChessRules> {
    apply_at(
        state,
        Input::Timer {
            timer: tabula_core::TimerId(1),
        },
        10_000,
        1,
    )
    .expect("deadline timer is an accepted input")
}

fn assert_timeout_command(state: &mut State, input: Input<Command>, now: u64, index: u64) {
    let outcome = apply_at(state, input, now, index).expect("valid command reaches timeout");
    let Status::Ended { outcome: result } = &state.status else {
        panic!("expired command must end the match");
    };
    assert_eq!(result.summary(), "timeout");
    assert_eq!(
        state.clock.unwrap().remaining[usize::from(state.turn.seat().0)],
        Millis::ZERO
    );
    assert_terminal_effects(&outcome);
}

#[test]
fn clock_config_is_validated_before_match_creation() {
    let valid = config(ClockControl::Fischer {
        increment: Millis(1_000),
    });
    assert!(ChessModule::validate_config(&valid, &roster()).is_ok());

    let exact_fischer_limit = Config {
        clock: Some(ClockConfig {
            initial: Millis(1),
            control: ClockControl::Fischer {
                increment: Millis(u64::MAX - 1),
            },
        }),
    };
    assert!(ChessModule::validate_config(&exact_fischer_limit, &roster()).is_ok());

    let above_fischer_limit = Config {
        clock: Some(ClockConfig {
            initial: Millis(1),
            control: ClockControl::Fischer {
                increment: Millis(u64::MAX),
            },
        }),
    };
    assert!(ChessModule::validate_config(&above_fischer_limit, &roster()).is_err());

    for invalid in [
        Config {
            clock: Some(ClockConfig {
                initial: Millis::ZERO,
                control: ClockControl::Fischer {
                    increment: Millis(0),
                },
            }),
        },
        Config {
            clock: Some(ClockConfig {
                initial: Millis(u64::MAX),
                control: ClockControl::Fischer {
                    increment: Millis(1),
                },
            }),
        },
        Config {
            clock: Some(ClockConfig {
                initial: Millis(u64::MAX),
                control: ClockControl::Bronstein { delay: Millis(1) },
            }),
        },
    ] {
        assert!(ChessModule::validate_config(&invalid, &roster()).is_err());

        let seed = MatchSeed::from_bytes([29; 32]);
        let mut rng = DetRng::for_input(&seed, InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        assert!(ChessRules::create(&invalid, &roster(), &mut ctx).is_err());
    }
}

#[test]
fn clocked_creation_initializes_and_arms_the_white_clock() {
    let state = create(
        config(ClockControl::Bronstein {
            delay: Millis(2_000),
        }),
        7_000,
    );
    assert_eq!(
        state.clock,
        Some(ClockState {
            remaining: [Millis(10_000); 2],
            last_move_at: LogicalTime(7_000),
            control: ClockControl::Bronstein {
                delay: Millis(2_000),
            },
        })
    );

    let seed = MatchSeed::from_bytes([31; 32]);
    let mut rng = DetRng::for_input(&seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime(7_000),
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget::default(),
    };
    let init = ChessRules::create(
        &config(ClockControl::Bronstein {
            delay: Millis(2_000),
        }),
        &roster(),
        &mut ctx,
    )
    .unwrap();
    assert!(matches!(
        init.effects.as_slice(),
        [Effect::SetTimer {
            id: tabula_core::TimerId(1),
            delay: Millis(12_000)
        }]
    ));
}

#[test]
fn fischer_move_decrements_elapsed_and_adds_increment() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(2_000),
        }),
        0,
    );
    let outcome = apply_at(&mut state, move_input(0, 12, 28), 5_000, 1).unwrap();

    assert_eq!(state.clock.unwrap().remaining[0], Millis(7_000));
    assert_eq!(state.clock.unwrap().last_move_at, LogicalTime(5_000));
    assert_eq!(state.turn, Color::Black);
    assert_eq!(timer_delay(&outcome), Millis(10_000));
    assert!(matches!(outcome.events[0], Event::Moved { .. }));
    assert!(matches!(
        outcome.events[1],
        Event::ClockUpdated {
            seat: SeatId(0),
            remaining: Millis(7_000)
        }
    ));
}

#[test]
fn fischer_increment_is_not_granted_to_flagged_player() {
    let mut state = create(
        Config {
            clock: Some(ClockConfig {
                initial: Millis(5_000),
                control: ClockControl::Fischer {
                    increment: Millis(3_000),
                },
            }),
        },
        0,
    );
    let outcome = apply_at(&mut state, move_input(0, 12, 28), 5_000, 1).unwrap();

    assert_timeout(&state, Color::White);
    assert!(matches!(outcome.events[0], Event::ClockUpdated { .. }));
    assert_terminal_effects(&outcome);
    assert_eq!(
        state.board[12].unwrap().kind,
        tabula_game_chess::PieceKind::Pawn
    );
    assert!(state.board[28].is_none());
}

#[test]
fn fischer_near_limit_repeated_zero_elapsed_moves_never_wrap() {
    let mut state = create(
        Config {
            clock: Some(ClockConfig {
                initial: Millis(1),
                control: ClockControl::Fischer {
                    increment: Millis(u64::MAX - 1),
                },
            }),
        },
        0,
    );

    apply_at(&mut state, move_input(0, 12, 28), 0, 1).unwrap();
    assert_eq!(state.clock.unwrap().remaining[0], Millis(u64::MAX));
    apply_at(&mut state, move_input(1, 52, 36), 0, 2).unwrap();
    assert_eq!(state.clock.unwrap().remaining[1], Millis(u64::MAX));
    apply_at(&mut state, move_input(0, 6, 21), 0, 3).unwrap();
    assert_eq!(state.clock.unwrap().remaining[0], Millis(u64::MAX));
}

#[test]
fn fischer_exact_zero_flags_before_move() {
    let mut state = create(
        Config {
            clock: Some(ClockConfig {
                initial: Millis(5_000),
                control: ClockControl::Fischer {
                    increment: Millis(3_000),
                },
            }),
        },
        0,
    );
    let outcome = apply_at(&mut state, move_input(0, 12, 28), 5_000, 1).unwrap();

    assert_timeout(&state, Color::White);
    assert_eq!(outcome.events.len(), 2);
    assert!(matches!(outcome.events[0], Event::ClockUpdated { .. }));
    assert!(matches!(outcome.events[1], Event::Ended { .. }));
}

#[test]
fn fischer_one_millisecond_before_deadline_survives() {
    let mut state = create(
        Config {
            clock: Some(ClockConfig {
                initial: Millis(5_000),
                control: ClockControl::Fischer {
                    increment: Millis(0),
                },
            }),
        },
        0,
    );
    apply_at(&mut state, move_input(0, 12, 28), 4_999, 1).unwrap();

    assert!(matches!(state.status, Status::Playing));
    assert_eq!(state.clock.unwrap().remaining[0], Millis(1));
}

#[test]
fn bronstein_move_inside_delay_consumes_no_clock() {
    let mut state = create(
        config(ClockControl::Bronstein {
            delay: Millis(2_000),
        }),
        0,
    );
    let outcome = apply_at(&mut state, move_input(0, 12, 28), 1_000, 1).unwrap();

    assert_eq!(state.clock.unwrap().remaining[0], Millis(10_000));
    assert_eq!(state.turn, Color::Black);
    assert_eq!(timer_delay(&outcome), Millis(12_000));
}

#[test]
fn bronstein_move_past_delay_consumes_only_excess() {
    let mut state = create(
        config(ClockControl::Bronstein {
            delay: Millis(2_000),
        }),
        0,
    );
    let outcome = apply_at(&mut state, move_input(0, 12, 28), 4_000, 1).unwrap();

    assert_eq!(state.clock.unwrap().remaining[0], Millis(8_000));
    assert_eq!(timer_delay(&outcome), Millis(12_000));
}

#[test]
fn bronstein_exact_timeout_boundary_flags() {
    let mut state = create(
        config(ClockControl::Bronstein {
            delay: Millis(2_000),
        }),
        0,
    );
    let outcome = apply_at(&mut state, move_input(0, 12, 28), 12_000, 1).unwrap();

    assert_timeout(&state, Color::White);
    assert_terminal_effects(&outcome);
    assert!(state.board[28].is_none());
}

#[test]
fn bronstein_delay_is_refunded_without_resetting_the_turn() {
    let mut state = create(
        config(ClockControl::Bronstein {
            delay: Millis(2_000),
        }),
        0,
    );
    apply_at(&mut state, move_input(0, 12, 28), 1_000, 1).unwrap();

    assert_eq!(state.turn, Color::Black);
    assert_eq!(state.clock.unwrap().last_move_at, LogicalTime(1_000));
    assert_eq!(state.clock.unwrap().remaining[0], Millis(10_000));
}

#[test]
fn timer_at_exact_deadline_flags_side_to_move() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let outcome = apply_at(
        &mut state,
        Input::Timer {
            timer: tabula_core::TimerId(1),
        },
        10_000,
        1,
    )
    .unwrap();

    assert_timeout(&state, Color::White);
    assert_terminal_effects(&outcome);
}

#[test]
fn timeout_with_mating_material_is_decisive_and_does_not_mutate_board() {
    let mut state = clocked_fen("7k/8/8/8/8/8/6q1/7K w - - 0 1");
    let board_before = state.board;
    let outcome = timeout_at_deadline(&mut state);

    assert_timeout(&state, Color::White);
    let Status::Ended { outcome: result } = &state.status else {
        panic!("timeout must end the match");
    };
    assert_eq!(result.kind(), OutcomeKind::Decisive);
    assert_eq!(state.board, board_before);
    assert_terminal_effects(&outcome);
}

#[test]
fn timeout_is_a_draw_when_the_surviving_side_has_only_a_king() {
    let mut state = clocked_fen("7k/8/8/8/8/8/8/7K w - - 0 1");
    let board_before = state.board;
    let outcome = timeout_at_deadline(&mut state);

    let Status::Ended { outcome: result } = &state.status else {
        panic!("timeout must end the match");
    };
    assert_eq!(result.kind(), OutcomeKind::Draw);
    assert_eq!(result.summary(), "timeout");
    assert_eq!(state.board, board_before);
    assert_eq!(state.clock.unwrap().remaining[0], Millis::ZERO);
    assert_terminal_effects(&outcome);
}

#[test]
fn timeout_ignores_flagged_side_material_when_survivor_is_bare_king() {
    let mut state = clocked_fen("7k/8/8/8/8/8/6Q1/7K w - - 0 1");
    let board_before = state.board;
    let outcome = timeout_at_deadline(&mut state);

    let Status::Ended { outcome: result } = &state.status else {
        panic!("timeout must end the match");
    };
    assert_eq!(result.kind(), OutcomeKind::Draw);
    assert_eq!(state.board, board_before);
    assert_terminal_effects(&outcome);
}

#[test]
fn timeout_material_edges_distinguish_minor_and_mating_combinations() {
    for (fen, expected_kind) in [
        ("7k/8/8/8/8/8/8/b6K w - - 0 1", OutcomeKind::Draw),
        ("7k/8/8/8/8/8/8/n6K w - - 0 1", OutcomeKind::Draw),
        ("7k/8/2n5/8/8/8/8/b6K w - - 0 1", OutcomeKind::Decisive),
        ("7k/8/8/8/5b2/8/8/2b4K w - - 0 1", OutcomeKind::Draw),
        ("7k/8/8/8/4b3/8/8/2b4K w - - 0 1", OutcomeKind::Decisive),
    ] {
        let mut state = clocked_fen(fen);
        let board_before = state.board;
        let outcome = timeout_at_deadline(&mut state);
        let Status::Ended { outcome: result } = &state.status else {
            panic!("timeout must end the match");
        };
        assert_eq!(result.kind(), expected_kind, "unexpected result for {fen}");
        assert_eq!(state.board, board_before);
        assert_terminal_effects(&outcome);
    }
}

#[test]
fn early_timer_rearms_instead_of_ending_match() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let before = state.clone();
    let outcome = apply_at(
        &mut state,
        Input::Timer {
            timer: tabula_core::TimerId(1),
        },
        3_000,
        1,
    )
    .unwrap();

    assert!(matches!(state.status, Status::Playing));
    assert_eq!(state.clock, before.clock);
    assert_eq!(timer_delay(&outcome), Millis(7_000));
}

#[test]
fn unknown_timer_is_harmless() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let before = canonical_encode(&state).unwrap();
    let outcome = apply_at(
        &mut state,
        Input::Timer {
            timer: tabula_core::TimerId(99),
        },
        10_000,
        1,
    )
    .unwrap();

    assert!(outcome.events.is_empty());
    assert!(outcome.effects.is_empty());
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

#[test]
fn seat_disconnect_does_not_pause_clock() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let before = canonical_encode(&state).unwrap();
    let outcome = apply_at(
        &mut state,
        Input::Seat {
            seat: SeatId(0),
            change: SeatChange::Disconnected,
        },
        5_000,
        1,
    )
    .unwrap();

    assert!(outcome.events.is_empty());
    assert!(outcome.effects.is_empty());
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

#[test]
fn illegal_move_does_not_consume_clock() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let before = canonical_encode(&state).unwrap();
    let error = apply_at(&mut state, move_input(0, 52, 44), 10_000, 1).unwrap_err();

    assert_eq!(error.code, tabula_core::RuleErrorCode::IllegalMove);
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

#[test]
fn out_of_turn_move_does_not_consume_clock() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let before = canonical_encode(&state).unwrap();
    let error = apply_at(&mut state, move_input(1, 52, 36), 10_000, 1).unwrap_err();

    assert_eq!(error.code, tabula_core::RuleErrorCode::NotYourTurn);
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

#[test]
fn rejected_draw_command_does_not_consume_clock() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    apply_at(&mut state, move_input(0, 12, 28), 1_000, 1).unwrap();
    let before = canonical_encode(&state).unwrap();
    let error = apply_at(
        &mut state,
        Input::Player {
            seat: SeatId(1),
            command: Command::OfferDraw,
        },
        10_000,
        2,
    )
    .unwrap_err();

    assert_eq!(error.code, tabula_core::RuleErrorCode::WrongPhase);
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

#[test]
fn expired_clock_preempts_valid_non_move_commands() {
    let mut resign = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    assert_timeout_command(
        &mut resign,
        Input::Player {
            seat: SeatId(0),
            command: Command::Resign,
        },
        10_000,
        1,
    );

    let mut claim = clocked_fen("4k3/8/8/8/8/8/8/4K3 w - - 100 1");
    assert_timeout_command(
        &mut claim,
        Input::Player {
            seat: SeatId(0),
            command: Command::ClaimDraw,
        },
        10_000,
        1,
    );

    let mut offer = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    apply_at(&mut offer, move_input(0, 12, 28), 0, 1).unwrap();
    apply_at(&mut offer, move_input(1, 52, 36), 0, 2).unwrap();
    assert_timeout_command(
        &mut offer,
        Input::Player {
            seat: SeatId(1),
            command: Command::OfferDraw,
        },
        11_000,
        3,
    );

    let mut offered = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    apply_at(&mut offered, move_input(0, 12, 28), 0, 1).unwrap();
    apply_at(&mut offered, move_input(1, 52, 36), 0, 2).unwrap();
    apply_at(
        &mut offered,
        Input::Player {
            seat: SeatId(1),
            command: Command::OfferDraw,
        },
        0,
        3,
    )
    .unwrap();

    let mut accept = offered.clone();
    assert_timeout_command(
        &mut accept,
        Input::Player {
            seat: SeatId(0),
            command: Command::AcceptDraw,
        },
        11_000,
        4,
    );

    let mut decline = offered;
    assert_timeout_command(
        &mut decline,
        Input::Player {
            seat: SeatId(0),
            command: Command::DeclineDraw,
        },
        11_000,
        4,
    );
}

#[test]
fn clock_timer_rearms_for_new_side_after_move() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let outcome = apply_at(&mut state, move_input(0, 12, 28), 1_000, 1).unwrap();

    assert_eq!(state.turn, Color::Black);
    assert_eq!(timer_delay(&outcome), Millis(10_000));
}

#[test]
fn terminal_move_cancels_clock_timer() {
    let mut state = State::from_fen("7k/5Q2/6K1/8/8/8/8/8 w - - 0 1").unwrap();
    state.clock = Some(ClockState {
        remaining: [Millis(10_000); 2],
        last_move_at: LogicalTime::ZERO,
        control: ClockControl::Fischer {
            increment: Millis(1_000),
        },
    });
    let outcome = apply_at(&mut state, move_input(0, 53, 54), 1_000, 1).unwrap();

    assert!(matches!(state.status, Status::Ended { .. }));
    assert_terminal_effects(&outcome);
}

#[test]
fn resignation_cancels_clock_timer() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let outcome = apply_at(
        &mut state,
        Input::Player {
            seat: SeatId(0),
            command: Command::Resign,
        },
        1_000,
        1,
    )
    .unwrap();

    assert_terminal_effects(&outcome);
}

#[test]
fn admin_cancel_cancels_clock_timer() {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let outcome = apply_at(
        &mut state,
        Input::Admin(AdminInput::Cancel {
            reason: AbortReason::OperatorCancelled,
        }),
        1_000,
        1,
    )
    .unwrap();

    assert_terminal_effects(&outcome);
}

struct ClockTrace {
    state: Vec<u8>,
    hash: tabula_core::StateHash,
    events: Vec<Vec<u8>>,
    effects: Vec<Vec<u8>>,
}

fn run_clocked_script() -> ClockTrace {
    let mut state = create(
        config(ClockControl::Fischer {
            increment: Millis(1_000),
        }),
        0,
    );
    let mut events = Vec::new();
    let mut effects = Vec::new();
    let inputs = [
        (move_input(0, 12, 28), 1_000),
        (
            Input::Timer {
                timer: tabula_core::TimerId(1),
            },
            1_500,
        ),
        (move_input(1, 52, 36), 2_000),
        (
            Input::Timer {
                timer: tabula_core::TimerId(1),
            },
            2_500,
        ),
    ];
    for (index, (input, now)) in inputs.into_iter().enumerate() {
        let outcome = apply_at(&mut state, input, now, index as u64 + 1).unwrap();
        for event in &outcome.events {
            events.push(canonical_encode(event).unwrap());
        }
        for effect in &outcome.effects {
            effects.push(canonical_encode(effect).unwrap());
        }
    }
    ClockTrace {
        state: canonical_encode(&state).unwrap(),
        hash: ChessRules::state_hash(&state),
        events,
        effects,
    }
}

#[test]
fn clocked_script_is_deterministic_in_state_events_and_effects() {
    let first = run_clocked_script();
    let second = run_clocked_script();

    assert_eq!(first.state, second.state);
    assert_eq!(first.hash, second.hash);
    assert_eq!(first.events, second.events);
    assert_eq!(first.effects, second.effects);
}
