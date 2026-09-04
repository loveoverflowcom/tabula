//! Macroquad platform entry point for local hot-seat gameplay wiring.
//!
//! # One loop, three games
//!
//! [`run_local`] is generic over `(GameRules, GamePresentation)` and contains no
//! game-specific behaviour: it resolves the viewport, advances logical time,
//! feeds normalized input through the presenter, optionally drives bot seats,
//! and submits the render list. Everything a game contributes is passed in —
//! its config, its roster, and two closures that name facts only the game can
//! know (which seat is on turn, and whether the match wants a bot to move).
//!
//! The `SelectedGame` match below is the only place a game is named, and that
//! is the Phase-2 local vertical slice's deliberate leaf wiring: Phase 4
//! replaces it with `tabula-registry` (doc 01 §5.1).

use macroquad::prelude as mq;
use renderer_macroquad::{MacroquadAudioSink, MacroquadRenderer};
use tabula_core::{
    BotLevel, DetRng, InputIndex, MatchSeed, Millis, Occupant, SeatEntry, SeatId, SeatRoster,
    UserId, Viewer,
};
use tabula_game_api::{GameBot, GameModule, GameRules};
#[rustfmt::skip]
use tabula_game_chess::{ // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    presentation::ChessPresentation, ChessRules, ClockConfig, ClockControl, Config as ChessConfig,
};
use tabula_game_client::{resolve_display_geometry, LocalMatch, LocalMatchError};
#[rustfmt::skip]
use tabula_game_tictactoe::{ // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    presentation::TicTacToePresentation, Config as TicTacToeConfig, TicTacToeRules,
};
#[rustfmt::skip]
use tabula_game_tiles::{ // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
    presentation::TilesPresentation,
    rules::{MAX_SEATS as MAX_PLACEMENT_SEATS, MIN_SEATS as MIN_PLACEMENT_SEATS},
    Config as TilesConfig, TilesModule, TilesRules,
};
use tabula_presentation::{AudioSink, GamePresentation, Renderer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SelectedGame {
    #[default]
    Chess, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    TicTacToe, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    Tiles,     // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
}

/// How the local shell fills the seats nobody is sitting at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SeatFill {
    /// Every seat is played by whoever is at the keyboard; the viewer follows
    /// the seat on turn.
    #[default]
    HotSeat,
    /// Seat 0 is human, every other seat is driven by the game's own bot.
    Solo,
}

fn window_conf() -> mq::Conf {
    mq::Conf {
        window_title: String::from("Tabula — local hot seat"),
        window_width: 900,
        window_height: 720,
        high_dpi: true,
        ..mq::Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut renderer = MacroquadRenderer::new();
    let mut audio = MacroquadAudioSink::new();
    let theme = tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light);
    let options = parse_options();

    // The only place a game is named. Each arm is a one-liner so rustfmt keeps
    // its trailing comment, which is what lets it carry its own I-9
    // suppression marker instead of the whole block sharing one.
    match options.game {
        SelectedGame::Chess => run_chess(&mut renderer, &mut audio, &theme).await, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        SelectedGame::TicTacToe => run_tictactoe(&mut renderer, &mut audio, &theme).await, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        SelectedGame::Tiles => run_tiles(&mut renderer, &mut audio, &theme, options).await, // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
    }
}

#[rustfmt::skip]
async fn run_chess( // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
) {
    let local_match = LocalMatch::<ChessRules, ChessPresentation>::new(
        &ChessConfig {
            clock: Some(ClockConfig {
                initial: Millis::from_secs(5 * 60),
                control: ClockControl::Fischer {
                    increment: Millis::from_secs(2),
                },
            }),
        },
        &human_roster(2),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("the fixed local configuration is valid");
    run_local(
        local_match,
        renderer,
        audio,
        theme,
        |view| view.turn.seat(),
        None,
        &[],
    )
    .await;
}

#[rustfmt::skip]
async fn run_tictactoe( // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
) {
    let local_match = LocalMatch::<TicTacToeRules, TicTacToePresentation>::new(
        &TicTacToeConfig {
            move_timeout_ms: 30_000,
        },
        &human_roster(2),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("the fixed local configuration is valid");
    run_local(
        local_match,
        renderer,
        audio,
        theme,
        |view| view.turn,
        None,
        &[],
    )
    .await;
}

#[rustfmt::skip]
async fn run_tiles( // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
    options: Options,
) {
    let seats = options
        .seats
        .clamp(MIN_PLACEMENT_SEATS, MAX_PLACEMENT_SEATS);
    let local_match = LocalMatch::<TilesRules, TilesPresentation>::new(
        &TilesConfig {
            // Local play has no deadline: the human takes as long as they
            // like, and the async path is the same rules with a nonzero value
            // here (docs/games/tiles.md).
            turn_deadline_ms: 0,
        },
        &human_roster(seats),
        MatchSeed::from_bytes([0; 32]),
        Viewer::Seat(SeatId(0)),
    )
    .expect("the fixed local configuration is valid");
    let bot_seats: Vec<SeatId> = match options.fill {
        SeatFill::HotSeat => Vec::new(),
        SeatFill::Solo => (1..seats).map(SeatId).collect(),
    };
    run_local(
        local_match,
        renderer,
        audio,
        theme,
        |view| view.turn,
        TilesModule::bot(BotLevel::Easy),
        &bot_seats,
    )
    .await;
}

/// The whole local gameplay loop, with no game-specific branch in it.
///
/// `turn_of` is the one fact a generic loop cannot derive: which seat's
/// projection to show in hot seat. `bot`/`bot_seats` let the shell drive the
/// seats nobody is sitting at — a bot is a seat whose commands come from a
/// function of that seat's projection (doc 00 §6.5), so its answer goes in
/// through the ordinary player path and can be rejected like anyone else's.
#[allow(clippy::too_many_arguments)]
async fn run_local<R, P>(
    mut local_match: LocalMatch<R, P>,
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
    turn_of: fn(&R::View) -> SeatId,
    bot: Option<Box<dyn GameBot<R>>>,
    bot_seats: &[SeatId],
) where
    R: GameRules,
    P: GamePresentation<Rules = R>,
    P::Local: SetViewport,
{
    // The bot's randomness is derived from the same deterministic kernel the
    // rules use, so a local session with bots is as reproducible as one
    // without.
    let mut bot_rng = DetRng::for_input(&MatchSeed::from_bytes([0; 32]), InputIndex(u64::MAX));

    'game_loop: loop {
        let Some((viewport, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            // A transient zero or non-finite viewport/DPI (startup,
            // backgrounding, rapid browser resize) is skipped at the shell
            // boundary rather than weakening the validated `Viewport` type.
            mq::next_frame().await;
            continue 'game_loop;
        };
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
                Err(LocalMatchError::Rejected(error)) => {
                    eprintln!("local command rejected: {error}");
                }
                Err(error) => {
                    eprintln!("local match stopped: {error:?}");
                    break 'game_loop;
                }
            }
        }

        // Bot seats. Requests the rules raise explicitly are drained first so
        // a game that emits `RequestBotMove` is not second-guessed.
        let requested: Vec<SeatId> = local_match
            .drain_bot_requests()
            .map(|request| request.seat)
            .collect();
        if let Some(bot) = bot.as_ref() {
            let on_turn = turn_of(local_match.view());
            let due = requested
                .iter()
                .copied()
                .chain(bot_seats.iter().copied().filter(|seat| *seat == on_turn));
            for seat in due {
                if local_match.ended().is_some() {
                    break;
                }
                local_match.set_viewer(Viewer::Seat(seat));
                let Some(command) = bot.choose(local_match.view(), seat, &mut bot_rng) else {
                    continue;
                };
                match local_match.submit_bot_move(seat, command, &frame) {
                    Ok(cues) => play_cues(audio, &cues),
                    Err(LocalMatchError::Rejected(error)) => {
                        eprintln!("local bot command rejected: {error}");
                    }
                    Err(error) => {
                        eprintln!("local match stopped: {error:?}");
                        break 'game_loop;
                    }
                }
            }
        }

        for notice in local_match.drain_notices() {
            eprintln!("notice: {notice:?}");
        }

        // Hot seat: show whoever is on turn. With bots, the human at seat 0
        // keeps their own view.
        let viewer = if bot_seats.is_empty() {
            Viewer::Seat(turn_of(local_match.view()))
        } else {
            Viewer::Seat(SeatId(0))
        };
        local_match.set_viewer(viewer);

        let scene = local_match.present(&frame);
        if let Err(error) = renderer.submit(&scene) {
            eprintln!("local board render failed: {error:?}");
            break 'game_loop;
        }
        renderer
            .end_frame()
            .expect("Macroquad end_frame is infallible");
        mq::next_frame().await;
    }
}

/// The one thing the generic loop needs from a presenter's local state.
///
/// Every game already has this method; naming it as a trait is what lets one
/// loop serve all of them instead of one loop per game.
trait SetViewport {
    fn set_viewport(&mut self, viewport: tabula_presentation::Viewport);
}

#[rustfmt::skip]
impl SetViewport for tabula_game_chess::presentation::ChessLocal { // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    fn set_viewport(&mut self, viewport: tabula_presentation::Viewport) {
        Self::set_viewport(self, viewport);
    }
}

#[rustfmt::skip]
impl SetViewport for tabula_game_tictactoe::presentation::TicTacToeLocal { // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
    fn set_viewport(&mut self, viewport: tabula_presentation::Viewport) {
        Self::set_viewport(self, viewport);
    }
}

#[rustfmt::skip]
impl SetViewport for tabula_game_tiles::presentation::TilesLocal { // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
    fn set_viewport(&mut self, viewport: tabula_presentation::Viewport) {
        Self::set_viewport(self, viewport);
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Options {
    game: SelectedGame,
    seats: u8,
    fill: SeatFill,
}

fn parse_options() -> Options {
    let mut options = Options {
        game: SelectedGame::default(),
        seats: 3,
        fill: SeatFill::default(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game" => {
                if let Some(name) = args.next() {
                    options.game = parse_game(&name);
                }
            }
            "--seats" => {
                if let Some(value) = args.next() {
                    if let Ok(seats) = value.parse() {
                        options.seats = seats;
                    }
                }
            }
            "--solo" => options.fill = SeatFill::Solo,
            other => eprintln!("ignoring unknown argument {other:?}"),
        }
    }
    options
}

#[rustfmt::skip]
fn parse_game(name: &str) -> SelectedGame {
    match name.to_ascii_lowercase().as_str() {
        "chess" => SelectedGame::Chess, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        "tictactoe" | "tic-tac-toe" => SelectedGame::TicTacToe, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
        "tiles" => SelectedGame::Tiles, // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
        other => {
            eprintln!("unknown game '{other}', defaulting to the first entry");
            SelectedGame::default()
        }
    }
}

fn human_roster(seats: u8) -> SeatRoster {
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
