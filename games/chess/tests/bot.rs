#![cfg(feature = "bots")]

use smallvec::smallvec;
use tabula_core::{BotLevel, DetRng, InputIndex, SeatId, Viewer};
use tabula_game_api::{GameBot, GameRules};
use tabula_game_chess::{bot::ChessBot, ChessModule, ChessRules, Command, Config, State, View};
use tabula_testkit::selfplay::{SelfPlayConfig, SelfPlaySetup};

fn view(fen: &str) -> View {
    let state = State::from_fen(fen).expect("bot fixture FEN must parse");
    tabula_game_chess::ChessRules::project(&state, Viewer::Seat(SeatId(0)))
}

fn choose(fen: &str, level: BotLevel, seed: [u8; 32]) -> Option<Command> {
    let view = view(fen);
    let mut rng = DetRng::for_input(&tabula_core::MatchSeed::from_bytes(seed), InputIndex(7));
    ChessBot::new(level).choose(&view, SeatId(0), &mut rng)
}

fn easy_choice(view: &View, seed: [u8; 32]) -> Option<Command> {
    let mut rng = DetRng::for_input(&tabula_core::MatchSeed::from_bytes(seed), InputIndex(7));
    ChessBot::new(BotLevel::Easy).choose(view, SeatId(0), &mut rng)
}

#[test]
fn easy_chooses_the_materially_superior_capture() {
    let fen = "k7/8/8/3q4/8/8/8/3RK3 w - - 0 1";
    let queen_capture = Command::Move {
        from: 3,
        to: 35,
        promotion: None,
    };

    assert!(view(fen).legal_moves.contains(&queen_capture));
    assert_eq!(choose(fen, BotLevel::Easy, [41; 32]), Some(queen_capture));
}

#[test]
fn easy_searches_the_opponent_reply_before_taking_poisoned_material() {
    // Qxd4 appears to win a rook, but ...Kxd4 wins the queen. A one-ply
    // capture scorer chooses Qxd4; the two-ply policy chooses a safe queen move.
    let fen = "8/8/8/4k3/3r4/2Q5/8/K7 w - - 0 1";
    let poisoned_capture = Command::Move {
        from: 18,
        to: 27,
        promotion: None,
    };
    let projected = view(fen);
    let chosen = easy_choice(&projected, [41; 32]).expect("a legal move should be available");

    assert!(projected.legal_moves.contains(&poisoned_capture));
    assert_ne!(chosen, poisoned_capture);
    assert!(projected.legal_moves.contains(&chosen));
}

#[test]
fn easy_uses_piece_square_positioning_when_material_is_equal() {
    // The bishop's central e4 square is materially equal to its other quiet
    // destinations, but is substantially better under the Easy PST heuristic.
    let fen = "7k/8/8/8/8/8/8/KB6 w - - 0 1";
    let central_bishop_move = Command::Move {
        from: 1,
        to: 28,
        promotion: None,
    };
    let projected = view(fen);

    assert_eq!(easy_choice(&projected, [41; 32]), Some(central_bishop_move));
    assert!(projected.legal_moves.contains(&central_bishop_move));
}

#[test]
fn easy_bot_is_deterministic_for_identical_view_and_rng() {
    let projected = view("7k/8/8/8/8/8/8/KN6 w - - 0 1");

    assert_eq!(
        easy_choice(&projected, [41; 32]),
        easy_choice(&projected, [41; 32])
    );
}

#[test]
fn easy_equal_score_ties_use_rng_but_stay_with_the_best_moves() {
    // From b1, Nc3 and Nd2 are equal centralization choices for this sparse
    // position. Different deterministic streams may select either, never an
    // edge/king move.
    let projected = view("7k/8/8/8/8/8/8/KN6 w - - 0 1");
    let best_moves = [
        Command::Move {
            from: 1,
            to: 18,
            promotion: None,
        },
        Command::Move {
            from: 1,
            to: 11,
            promotion: None,
        },
    ];
    let first = easy_choice(&projected, [0; 32]).expect("a legal move should be available");
    let second = easy_choice(&projected, [2; 32]).expect("a legal move should be available");

    assert!(best_moves.contains(&first));
    assert!(best_moves.contains(&second));
    assert_ne!(
        first, second,
        "the fixed RNG vectors should exercise both ties"
    );
    assert_eq!(first, easy_choice(&projected, [0; 32]).unwrap());
}

#[test]
fn easy_bot_always_returns_a_legal_projected_command() {
    for fen in [
        "k7/8/8/3q4/8/8/8/3RK3 w - - 0 1",
        "8/8/8/4k3/3r4/2Q5/8/K7 w - - 0 1",
        "7k/8/8/8/8/8/8/KB6 w - - 0 1",
        "7k/8/8/8/8/8/8/KN6 w - - 0 1",
    ] {
        let projected = view(fen);
        let chosen = easy_choice(&projected, [19; 32]).expect("fixture should have a move");
        assert!(projected.legal_moves.contains(&chosen), "{fen}: {chosen:?}");
    }
}

#[test]
fn easy_returns_none_for_the_wrong_seat_or_when_no_moves_are_projected() {
    let projected = view("7k/8/8/8/8/8/8/KN6 w - - 0 1");
    let mut rng = DetRng::for_input(&tabula_core::MatchSeed::from_bytes([41; 32]), InputIndex(7));
    assert_eq!(
        ChessBot::new(BotLevel::Easy).choose(&projected, SeatId(1), &mut rng),
        None
    );

    let no_moves = view("7k/8/8/8/8/8/8/K7 b - - 0 1");
    assert!(no_moves.legal_moves.is_empty());
    assert_eq!(easy_choice(&no_moves, [41; 32]), None);
}

#[test]
fn phase_one_bot_pairings_self_play_without_illegal_or_nondeterministic_moves() {
    for [white, black] in [
        [BotLevel::Trivial, BotLevel::Easy],
        [BotLevel::Easy, BotLevel::Trivial],
        [BotLevel::Easy, BotLevel::Easy],
    ] {
        let roster = tabula_core::SeatRoster::new(smallvec![
            tabula_core::SeatEntry {
                seat: SeatId(0),
                occupant: tabula_core::Occupant::Bot { level: white },
                team: None,
            },
            tabula_core::SeatEntry {
                seat: SeatId(1),
                occupant: tabula_core::Occupant::Bot { level: black },
                team: None,
            },
        ])
        .expect("self-play seats are distinct");
        let setup = SelfPlaySetup::<ChessRules> {
            config: Config::default(),
            roster,
        };
        let report = tabula_testkit::selfplay::run::<ChessModule>(
            &setup,
            &SelfPlayConfig {
                matches: 1,
                base_seed: [43; 32],
                hostile_fraction: 0.0,
                max_inputs: 2_000,
                check_projections: true,
                start_match_index: 0,
            },
        )
        .expect("self-play setup must be valid");

        assert!(report.is_success(), "{white:?} vs {black:?}: {report:?}");
        assert_eq!(report.determinism_failures, 0);
        assert_eq!(report.transactional_failures, 0);
        assert_eq!(report.max_input_failures, 0);
    }
}

#[test]
fn trivial_bot_remains_deterministic_and_legal_for_a_fixed_seed() {
    let projected = view("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let first = choose(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        BotLevel::Trivial,
        [41; 32],
    )
    .expect("the initial position has legal moves");
    let second = choose(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        BotLevel::Trivial,
        [41; 32],
    )
    .expect("the initial position has legal moves");

    assert_eq!(first, second);
    assert!(projected.legal_moves.contains(&first));
}
