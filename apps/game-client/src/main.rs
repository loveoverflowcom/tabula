//! Macroquad platform entry point for local hot-seat gameplay wiring.

use glam::Vec2;
use macroquad::prelude as mq;
use renderer_macroquad::{MacroquadAudioSink, MacroquadRenderer};
use tabula_core::{MatchSeed, Millis, Occupant, SeatEntry, SeatId, SeatRoster, UserId, Viewer};
#[rustfmt::skip]
use tabula_game_chess::{ // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    presentation::ChessPresentation, ChessRules, ClockConfig, ClockControl, Config as ChessConfig,
};
use tabula_game_client::LocalMatch;
#[rustfmt::skip]
use tabula_game_tictactoe::{ // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    presentation::TicTacToePresentation, Config as TicTacToeConfig, TicTacToeRules,
};
use tabula_presentation::{AudioSink, Dpi, Renderer, Viewport};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SelectedGame {
    #[default]
    Chess, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    TicTacToe, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
}

fn window_conf() -> mq::Conf {
    mq::Conf {
        window_title: String::from("Tabula — local hot seat"),
        window_width: 720,
        window_height: 720,
        ..mq::Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut renderer = MacroquadRenderer::new();
    let mut audio = MacroquadAudioSink::new();
    let theme = tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light);
    let selected_game = parse_selected_game();

    match selected_game {
        SelectedGame::Chess => run_chess(&mut renderer, &mut audio, &theme).await, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        SelectedGame::TicTacToe => run_tictactoe(&mut renderer, &mut audio, &theme).await, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    }
}

fn parse_selected_game() -> SelectedGame {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--game" {
            if let Some(name) = args.next() {
                match name.to_ascii_lowercase().as_str() {
                    "chess" => return SelectedGame::Chess, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
                    "tictactoe" | "tic-tac-toe" => return SelectedGame::TicTacToe, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
                    other => {
                        eprintln!("unknown game '{other}', defaulting to chess"); // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
                        return SelectedGame::Chess; // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
                    }
                }
            }
        }
    }
    SelectedGame::Chess // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
}

#[rustfmt::skip]
async fn run_chess( // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
) {
    let mut local_match = LocalMatch::<ChessRules, ChessPresentation>::new(
        &ChessConfig {
            clock: Some(ClockConfig {
                initial: Millis::from_secs(5 * 60),
                control: ClockControl::Fischer {
                    increment: Millis::from_secs(2),
                },
            }),
        },
        &local_roster(),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("the fixed local chess configuration is valid"); // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.

    'game_loop: loop {
        let viewport = Viewport::new(Vec2::new(mq::screen_width(), mq::screen_height()))
            .expect("Macroquad supplies a finite non-empty viewport");
        let dpi =
            Dpi::new(mq::screen_dpi_scale()).expect("Macroquad supplies a positive DPI scale");
        let frame = renderer.begin_frame(viewport, dpi, presentation_now_ms(), *theme);
        local_match.local_mut().set_viewport(frame.viewport());
        match local_match.advance_frame(&frame) {
            Ok(cues) => play_cues(audio, &cues),
            Err(error) => {
                eprintln!("local timer execution failed: {error:?}");
                break 'game_loop;
            }
        }
        for event in renderer.drain_input() {
            match local_match.handle_presentation_input(&event, &frame) {
                Ok(cues) => play_cues(audio, &cues),
                Err(tabula_game_client::LocalMatchError::Rejected(error)) => {
                    eprintln!("local command rejected: {error}");
                }
                Err(error) => {
                    eprintln!("local match stopped: {error:?}");
                    break 'game_loop;
                }
            }
        }

        local_match.set_viewer(Viewer::Seat(local_match.view().turn.seat()));
        let scene = local_match.present(&frame);
        if let Err(error) = renderer.submit(&scene) {
            eprintln!("local board render failed: {error:?}");
            break;
        }
        renderer
            .end_frame()
            .expect("Macroquad end_frame is infallible");
        mq::next_frame().await;
    }
}

#[rustfmt::skip]
async fn run_tictactoe( // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
) {
    let mut local_match = LocalMatch::<TicTacToeRules, TicTacToePresentation>::new(
        &TicTacToeConfig {
            move_timeout_ms: 30_000,
        },
        &local_roster(),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("the fixed local tictactoe configuration is valid"); // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.

    'game_loop: loop {
        let viewport = Viewport::new(Vec2::new(mq::screen_width(), mq::screen_height()))
            .expect("Macroquad supplies a finite non-empty viewport");
        let dpi =
            Dpi::new(mq::screen_dpi_scale()).expect("Macroquad supplies a positive DPI scale");
        let frame = renderer.begin_frame(viewport, dpi, presentation_now_ms(), *theme);
        local_match.local_mut().set_viewport(frame.viewport());
        match local_match.advance_frame(&frame) {
            Ok(cues) => play_cues(audio, &cues),
            Err(error) => {
                eprintln!("local timer execution failed: {error:?}");
                break 'game_loop;
            }
        }
        for event in renderer.drain_input() {
            match local_match.handle_presentation_input(&event, &frame) {
                Ok(cues) => play_cues(audio, &cues),
                Err(tabula_game_client::LocalMatchError::Rejected(error)) => {
                    eprintln!("local command rejected: {error}");
                }
                Err(error) => {
                    eprintln!("local match stopped: {error:?}");
                    break 'game_loop;
                }
            }
        }

        local_match.set_viewer(Viewer::Seat(local_match.view().turn));
        let scene = local_match.present(&frame);
        if let Err(error) = renderer.submit(&scene) {
            eprintln!("local board render failed: {error:?}");
            break;
        }
        renderer
            .end_frame()
            .expect("Macroquad end_frame is infallible");
        mq::next_frame().await;
    }
}

fn local_roster() -> SeatRoster {
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

fn play_cues(audio: &mut MacroquadAudioSink, cues: &tabula_presentation::AudioCues) {
    for cue in cues {
        let _ = audio.play(cue);
    }
}

/// Converts Macroquad's monotonic presentation clock into the renderer frame fact.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn presentation_now_ms() -> u64 {
    (mq::get_time() * 1_000.0) as u64
}
