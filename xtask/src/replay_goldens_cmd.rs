//! Intentional regeneration of the small checked-in Phase 1 replay corpus.
//!
//! Ordinary tests only read the committed bytes. This command is the explicit
//! update path documented in `tests/replays/README.md`; it builds the frames
//! through the ordinary typed rules functions and then passes them to the
//! format writer.

use std::{fs, path::Path};

use tabula_core::{
    canonical_encode, InputIndex, LogicalTime, MatchId, MatchOutcome, MatchSeed, SeatRoster,
};
use tabula_game_api::{Budget, Ctx, Effect, GameModule, GameRules, Input};
use tabula_testkit::{ReplayDraft, ReplayFrame, ReplayHeader, ReplayKind};

pub(crate) fn run() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask has no workspace parent".to_owned())?;
    let replay_dir = root.join("tests/replays");
    fs::create_dir_all(&replay_dir).map_err(|error| error.to_string())?;

    write_tictactoe(&replay_dir.join("tictactoe-golden.tbr"))?;
    write_chess(&replay_dir.join("chess-golden.tbr"))?;
    write_chess_clock(&replay_dir.join("chess-clock-golden.tbr"))?;
    println!("wrote replay goldens under {}", replay_dir.display());
    Ok(())
}

fn write_tictactoe(path: &Path) -> Result<(), String> {
    let inputs = vec![
        Input::Player {
            seat: tabula_core::SeatId(0),
            command: tabula_game_tictactoe::Command::Place { cell: 0 },
        },
        Input::Player {
            seat: tabula_core::SeatId(1),
            command: tabula_game_tictactoe::Command::Place { cell: 3 },
        },
        Input::Player {
            seat: tabula_core::SeatId(0),
            command: tabula_game_tictactoe::Command::Place { cell: 1 },
        },
        Input::Player {
            seat: tabula_core::SeatId(1),
            command: tabula_game_tictactoe::Command::Place { cell: 4 },
        },
        Input::Player {
            seat: tabula_core::SeatId(0),
            command: tabula_game_tictactoe::Command::Place { cell: 2 },
        },
    ];
    write_replay::<tabula_game_tictactoe::TicTacToeModule>(
        path,
        &tabula_game_tictactoe::Config {
            move_timeout_ms: 5_000,
        },
        standard_roster(),
        MatchSeed::from_bytes([0x11; 32]),
        inputs,
        vec![1_000, 2_000, 3_000, 4_000, 5_000],
        MatchId(1),
    )
}

fn write_chess(path: &Path) -> Result<(), String> {
    let inputs = vec![
        chess_move(13, 21), // f2-f3
        chess_move(52, 36), // e7-e5
        chess_move(14, 30), // g2-g4
        chess_move(59, 31), // Qd8-h4, mate
    ];
    write_replay::<tabula_game_chess::ChessModule>(
        path,
        &tabula_game_chess::Config { clock: None },
        standard_roster(),
        MatchSeed::from_bytes([0x22; 32]),
        inputs,
        vec![1_000, 2_000, 3_000, 4_000],
        MatchId(2),
    )
}

fn write_chess_clock(path: &Path) -> Result<(), String> {
    let inputs = vec![
        chess_move(12, 20), // e2-e3
        Input::Timer {
            timer: tabula_core::TimerId(1),
        },
    ];
    write_replay::<tabula_game_chess::ChessModule>(
        path,
        &tabula_game_chess::Config {
            clock: Some(tabula_game_chess::ClockConfig {
                initial: tabula_core::Millis(5_000),
                control: tabula_game_chess::ClockControl::Fischer {
                    increment: tabula_core::Millis(1_000),
                },
            }),
        },
        standard_roster(),
        MatchSeed::from_bytes([0x33; 32]),
        inputs,
        vec![1_000, 6_000],
        MatchId(3),
    )
}

fn chess_move(from: u8, to: u8) -> Input<tabula_game_chess::Command> {
    Input::Player {
        seat: tabula_core::SeatId(u8::from(from >= 32)),
        command: tabula_game_chess::Command::Move {
            from,
            to,
            promotion: None,
        },
    }
}

fn standard_roster() -> SeatRoster {
    use tabula_core::{BotLevel, Occupant, SeatEntry, SeatId};
    SeatRoster::new(smallvec::smallvec![
        SeatEntry {
            seat: SeatId(0),
            occupant: Occupant::Bot {
                level: BotLevel::Trivial,
            },
            team: None,
        },
        SeatEntry {
            seat: SeatId(1),
            occupant: Occupant::Bot {
                level: BotLevel::Trivial,
            },
            team: None,
        },
    ])
    .expect("golden roster uses distinct seats")
}

fn write_replay<M: GameModule>(
    path: &Path,
    config: &<M::Rules as GameRules>::Config,
    roster: SeatRoster,
    seed: MatchSeed,
    inputs: Vec<Input<<M::Rules as GameRules>::Command>>,
    times: Vec<u64>,
    match_id: MatchId,
) -> Result<(), String> {
    if inputs.len() != times.len() {
        return Err("golden inputs and logical times differ in length".to_owned());
    }
    M::validate_config(config, &roster).map_err(|error| error.to_string())?;

    let mut create_rng = tabula_core::DetRng::for_input(&seed, InputIndex(0));
    let mut create_ctx = context(&mut create_rng, InputIndex(0), LogicalTime::ZERO);
    let init = M::Rules::create(config, &roster, &mut create_ctx)
        .map_err(|error| format!("golden create failed: {error:?}"))?;
    let mut state = init.state;
    let mut frames = Vec::with_capacity(inputs.len());
    let mut derived_outcome = terminal_outcome(&init.effects, 0)?;
    for (position, (input, time)) in inputs.into_iter().zip(times).enumerate() {
        if derived_outcome.is_some() {
            return Err(format!(
                "golden input {} follows a terminal EndMatch effect",
                position as u64 + 1
            ));
        }
        let index = InputIndex(position as u64 + 1);
        let logical_time = LogicalTime(time);
        let mut rng = tabula_core::DetRng::for_input(&seed, index);
        let mut ctx = context(&mut rng, index, logical_time);
        let outcome = M::Rules::apply(&mut state, input.clone(), &mut ctx)
            .map_err(|error| format!("golden input {} rejected: {error:?}", index.0))?;
        derived_outcome = terminal_outcome(&outcome.effects, index.0)?;
        frames.push(ReplayFrame {
            input_index: index,
            logical_time,
            input: canonical_encode(&input).map_err(|_| "input encoding failed".to_owned())?,
            checkpoint: Some(M::Rules::state_hash(&state)),
        });
    }

    let draft = ReplayDraft {
        header: ReplayHeader {
            match_id,
            game_id: M::metadata().id().clone(),
            game_version: M::metadata().version().clone(),
            rules_version: M::Rules::RULES_VERSION,
            rules_hash: M::rules_hash(),
            config: canonical_encode(&config).map_err(|_| "config encoding failed".to_owned())?,
            roster,
            seed: Some(seed),
            initial_snapshot: None,
            started_at: 0,
            duration_ms: frames.last().map_or(0, |frame| frame.logical_time.0),
            outcome: derived_outcome,
            kind: ReplayKind::Canonical,
        },
        frames,
        final_state_hash: M::Rules::state_hash(&state),
    };
    let bytes = draft
        .to_bytes()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn terminal_outcome(effects: &[Effect], input_index: u64) -> Result<Option<MatchOutcome>, String> {
    let mut terminal = None;
    for effect in effects {
        if let Effect::EndMatch { outcome } = effect {
            if terminal.is_some() {
                return Err(format!(
                    "golden input {input_index} emitted EndMatch more than once"
                ));
            }
            terminal = Some(outcome.clone());
        }
    }
    Ok(terminal)
}

fn context(rng: &mut tabula_core::DetRng, index: InputIndex, now: LogicalTime) -> Ctx<'_> {
    Ctx {
        now,
        index,
        rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    }
}
