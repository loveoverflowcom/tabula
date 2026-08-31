//! Reachability and relabeling evidence for the tic-tac-toe reference game.

use std::collections::BTreeSet;

use smallvec::smallvec;
use tabula_core::{
    canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, Occupant, SeatEntry, SeatId,
    SeatRoster, SpectatorTier, UserId, Viewer,
};
use tabula_game_api::{Ctx, GameModule, GameRules, Input};
use tabula_game_tictactoe::{Command, Config, Mark, Status, TicTacToeModule, TicTacToeRules};

fn roster(first: u8, second: u8) -> SeatRoster {
    SeatRoster::new(smallvec![
        SeatEntry {
            seat: SeatId(first),
            occupant: Occupant::Human(UserId(1)),
            team: None
        },
        SeatEntry {
            seat: SeatId(second),
            occupant: Occupant::Human(UserId(2)),
            team: None
        },
    ])
    .unwrap()
}

fn create(roster: &SeatRoster, timeout: u64) -> tabula_game_tictactoe::State {
    let seed = MatchSeed::from_bytes([9; 32]);
    let mut rng = DetRng::for_input(&seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime(0),
        index: InputIndex(0),
        rng: &mut rng,
        budget: tabula_game_api::Budget::default(),
    };
    TicTacToeRules::create(
        &Config {
            move_timeout_ms: timeout,
        },
        roster,
        &mut ctx,
    )
    .unwrap()
    .state
}

fn apply(
    state: &mut tabula_game_tictactoe::State,
    input: Input<Command>,
    index: u64,
) -> Result<tabula_game_api::Outcome<TicTacToeRules>, tabula_core::RuleError> {
    let seed = MatchSeed::from_bytes([9; 32]);
    let mut rng = DetRng::for_input(&seed, InputIndex(index));
    let mut ctx = Ctx {
        now: LogicalTime(index * 1_000),
        index: InputIndex(index),
        rng: &mut rng,
        budget: tabula_game_api::Budget::default(),
    };
    TicTacToeRules::apply(state, input, &mut ctx)
}

#[test]
fn every_reachable_position_preserves_independent_tictactoe_invariants() {
    let seats = [SeatId(7), SeatId(42)];
    let mut visited = BTreeSet::new();
    visit(create(&roster(7, 42), 8_000), seats, 0, &mut visited);
    assert!(
        visited.len() > 5_000,
        "exploration should reach the full game tree"
    );
}

fn visit(
    state: tabula_game_tictactoe::State,
    seats: [SeatId; 2],
    depth: u64,
    visited: &mut BTreeSet<Vec<u8>>,
) {
    let key = canonical_encode(&state).unwrap();
    if !visited.insert(key) {
        return;
    }

    let view = TicTacToeRules::project(&state, Viewer::Spectator(SpectatorTier::Live));
    assert!(
        seats.contains(&view.turn),
        "turn always belongs to this match"
    );
    let occupied = view.board.iter().filter(|cell| cell.is_some()).count();
    let x_count = view
        .board
        .iter()
        .filter(|cell| **cell == Some(Mark::X))
        .count();
    let o_count = view
        .board
        .iter()
        .filter(|cell| **cell == Some(Mark::O))
        .count();
    assert!(o_count <= x_count && x_count <= o_count + 1);

    match view.status {
        Status::Playing => {
            assert!(occupied < 9);
            assert!(!has_line(&view.board, Mark::X) && !has_line(&view.board, Mark::O));
            let expected_turn = if x_count == o_count {
                seats[0]
            } else {
                seats[1]
            };
            assert_eq!(view.turn, expected_turn);

            let before = canonical_encode(&state).unwrap();
            let mut invalid = state.clone();
            assert!(apply(
                &mut invalid,
                Input::Player {
                    seat: SeatId(255),
                    command: Command::Place { cell: 9 }
                },
                depth + 10_000,
            )
            .is_err());
            assert_eq!(canonical_encode(&invalid).unwrap(), before);

            for cell in 0..9 {
                if view.board[cell].is_none() {
                    let mut next = state.clone();
                    apply(
                        &mut next,
                        Input::Player {
                            seat: view.turn,
                            command: Command::Place {
                                cell: u8::try_from(cell).unwrap_or(9),
                            },
                        },
                        depth + 1,
                    )
                    .unwrap();
                    visit(next, seats, depth + 1, visited);
                }
            }
        }
        Status::Won(winner) => {
            assert!(seats.contains(&winner));
            assert!(has_line(
                &view.board,
                if winner == seats[0] { Mark::X } else { Mark::O }
            ));
            rejects_post_terminal(state, depth);
        }
        Status::Drawn => {
            assert_eq!(occupied, 9);
            assert!(!has_line(&view.board, Mark::X) && !has_line(&view.board, Mark::O));
            rejects_post_terminal(state, depth);
        }
        Status::Forfeited(winner) => {
            assert!(seats.contains(&winner));
            rejects_post_terminal(state, depth);
        }
        Status::Aborted => rejects_post_terminal(state, depth),
    }
}

fn rejects_post_terminal(mut state: tabula_game_tictactoe::State, index: u64) {
    let before = canonical_encode(&state).unwrap();
    assert!(apply(
        &mut state,
        Input::Player {
            seat: SeatId(7),
            command: Command::Place { cell: 0 }
        },
        index + 20_000,
    )
    .is_err());
    assert_eq!(canonical_encode(&state).unwrap(), before);
}

fn has_line(board: &[Option<Mark>; 9], mark: Mark) -> bool {
    const LINES: [[usize; 3]; 8] = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ];
    LINES.iter().any(|[a, b, c]| {
        board[*a] == Some(mark) && board[*b] == Some(mark) && board[*c] == Some(mark)
    })
}

#[test]
fn relabeling_seats_changes_only_seat_identity() {
    let script = [(0, 0), (1, 3), (0, 1), (1, 4), (0, 2)];
    let left = play(&roster(7, 42), [SeatId(7), SeatId(42)], &script);
    let right = play(&roster(13, 99), [SeatId(13), SeatId(99)], &script);
    assert_eq!(left.board, right.board);
    assert_eq!(left.status, Status::Won(SeatId(7)));
    assert_eq!(right.status, Status::Won(SeatId(13)));
}

fn play(
    roster: &SeatRoster,
    seats: [SeatId; 2],
    script: &[(usize, u8)],
) -> tabula_game_tictactoe::View {
    let mut state = create(roster, 6_000);
    for (index, (seat, cell)) in script.iter().enumerate() {
        apply(
            &mut state,
            Input::Player {
                seat: seats[*seat],
                command: Command::Place { cell: *cell },
            },
            index as u64 + 1,
        )
        .unwrap();
    }
    TicTacToeRules::project(&state, Viewer::Spectator(SpectatorTier::Live))
}

#[test]
fn config_validation_and_creation_have_identical_acceptance() {
    let roster = roster(7, 42);
    for timeout in 0..=6_000 {
        let config = Config {
            move_timeout_ms: timeout,
        };
        let validated = TicTacToeModule::validate_config(&config, &roster).is_ok();
        let created = {
            let seed = MatchSeed::from_bytes([3; 32]);
            let mut rng = DetRng::for_input(&seed, InputIndex(0));
            let mut ctx = Ctx {
                now: LogicalTime(0),
                index: InputIndex(0),
                rng: &mut rng,
                budget: tabula_game_api::Budget::default(),
            };
            TicTacToeRules::create(&config, &roster, &mut ctx).is_ok()
        };
        assert_eq!(validated, created, "timeout {timeout}");
    }
}
