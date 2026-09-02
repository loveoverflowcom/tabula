//! Macroquad platform entry point for the local hot-seat chess wiring.

use glam::Vec2;
use local_game::{presentation::ChessPresentation, ChessRules, ClockConfig, ClockControl, Config};
use macroquad::prelude as mq;
use renderer_macroquad::{MacroquadAudioSink, MacroquadRenderer};
use tabula_core::{MatchSeed, Millis, Occupant, SeatEntry, SeatId, SeatRoster, UserId, Viewer};
use tabula_game_chess as local_game; // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
use tabula_presentation::{AudioSink, Dpi, Renderer, Viewport};

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
    let mut local_match = tabula_game_client::LocalMatch::<ChessRules, ChessPresentation>::new(
        &Config {
            // The generic timer executor is exercised by real chess clock rules.
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
    .expect("the fixed local configuration is valid");

    'game_loop: loop {
        let viewport = Viewport::new(Vec2::new(mq::screen_width(), mq::screen_height()))
            .expect("Macroquad supplies a finite non-empty viewport");
        let dpi =
            Dpi::new(mq::screen_dpi_scale()).expect("Macroquad supplies a positive DPI scale");
        let frame = renderer.begin_frame(viewport, dpi, presentation_now_ms(), theme);
        local_match.local_mut().set_viewport(frame.viewport());
        match local_match.advance_frame(&frame) {
            Ok(cues) => play_cues(&mut audio, &cues),
            Err(error) => {
                eprintln!("local timer execution failed: {error:?}");
                break 'game_loop;
            }
        }
        for event in renderer.drain_input() {
            match local_match.handle_presentation_input(&event, &frame) {
                Ok(cues) => play_cues(&mut audio, &cues),
                // Rejections are returned by the runtime, not erased. Keep the
                // local board running so a player can correct an illegal move.
                Err(tabula_game_client::LocalMatchError::Rejected(error)) => {
                    eprintln!("local command rejected: {error}");
                }
                Err(error) => {
                    eprintln!("local match stopped: {error:?}");
                    break 'game_loop;
                }
            }
        }

        // Hot-seat viewer selection is Chess wiring, not a generic runtime rule.
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
