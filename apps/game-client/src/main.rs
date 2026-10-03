//! Macroquad platform entry point for local hot-seat gameplay wiring.
//!
//! # One generic loop, multiple games
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
use tabula_game_client::{
    resolve_display_geometry,
    runtime_ui::{parse_local_theme, FeedbackInput, LocalFeedback},
    LocalMatch,
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
    Tiles, // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
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
        window_title: String::from("Tabula — local game"),
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
    let options = parse_options();
    let theme = tabula_design::Theme::by_kind(options.theme);

    // The only place a game is named. Each arm is a one-liner so rustfmt keeps
    // its trailing comment, which is what lets it carry its own I-9
    // suppression marker instead of the whole block sharing one.
    loop {
        match options.game {
            SelectedGame::Chess => run_chess(&mut renderer, &mut audio, &theme).await, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
            SelectedGame::Tiles => run_tiles(&mut renderer, &mut audio, &theme, options).await, // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
        }
        // A completed or stopped session returns only after New local game.
        // activation. Reconstruct the match, local state, clocks and bot RNG.
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
    let mut feedback = fresh_feedback();
    let started_at_ms = presentation_now_ms();

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
        let frame = renderer.begin_frame(
            viewport,
            dpi,
            session_elapsed_ms(started_at_ms, presentation_now_ms()),
            *theme,
        );
        let board_frame = sync_local_frame(&mut local_match, &mut feedback, &frame);

        if feedback.allows_gameplay(local_match.ended().is_some()) {
            match local_match.advance_frame(&board_frame) {
                Ok(cues) => play_cues(audio, &cues),
                Err(error) => feedback.note_timer_error(&error),
            }
        }
        sync_local_frame(&mut local_match, &mut feedback, &frame);

        for event in renderer.drain_input() {
            match feedback.route_input(&mut local_match, &event, &frame) {
                FeedbackInput::NewLocalGame => {
                    renderer.end_frame().expect("an active frame can be ended");
                    mq::next_frame().await;
                    return;
                }
                FeedbackInput::Consumed | FeedbackInput::CancelGameplayPointer => continue,
                FeedbackInput::PassThrough => {}
            }
            // Fatal feedback consumed input above. Terminal matches still allow
            // local inspection; LocalMatch discards any emitted intent.
            let accepted_before = local_match.replay_trace().accepted_inputs().len();
            let was_ended = local_match.ended().is_some();
            let board_frame = sync_local_frame(&mut local_match, &mut feedback, &frame);
            match local_match.handle_presentation_input(&event, &board_frame) {
                Ok(cues) => {
                    play_cues(audio, &cues);
                    if local_match.replay_trace().accepted_inputs().len() > accepted_before {
                        feedback.note_accepted_input();
                    }
                }
                Err(error) => {
                    feedback.note_error(&error);
                    feedback.suppress_opening_input(&event);
                }
            }
            sync_local_frame(&mut local_match, &mut feedback, &frame);
            if !was_ended && local_match.ended().is_some() {
                feedback.suppress_opening_input(&event);
            }
        }

        // Bot seats. Requests the rules raise explicitly are drained first so
        // a game that emits `RequestBotMove` is not second-guessed.
        let requested: Vec<SeatId> = local_match
            .drain_bot_requests()
            .map(|request| request.seat)
            .collect();
        if let Some(bot) = bot
            .as_ref()
            .filter(|_| feedback.allows_gameplay(local_match.ended().is_some()))
        {
            let on_turn = turn_of(local_match.view());
            let due = requested
                .iter()
                .copied()
                .chain(bot_seats.iter().copied().filter(|seat| *seat == on_turn));
            for seat in due {
                if !feedback.allows_gameplay(local_match.ended().is_some()) {
                    break;
                }
                local_match.set_viewer(Viewer::Seat(seat));
                let Some(command) = bot.choose(local_match.view(), seat, &mut bot_rng) else {
                    continue;
                };
                match local_match.submit_bot_move(seat, command, &frame) {
                    Ok(cues) => play_cues(audio, &cues),
                    Err(error) => feedback.note_error(&error),
                }
            }
        }

        // Notices carry an explicit audience. Until a viewer-filtered notice
        // adapter exists, discard them without leaking payloads into logs/UI.
        local_match.drain_notices().for_each(drop);

        // Hot seat: show whoever is on turn. With bots, the human at seat 0
        // keeps their own view.
        let viewer = if bot_seats.is_empty() {
            Viewer::Seat(turn_of(local_match.view()))
        } else {
            Viewer::Seat(SeatId(0))
        };
        local_match.set_viewer(viewer);

        let board_frame = sync_local_frame(&mut local_match, &mut feedback, &frame);
        present_local_scenes(&local_match, &mut feedback, renderer, &frame, &board_frame);
        renderer
            .end_frame()
            .expect("Macroquad end_frame is infallible");
        mq::next_frame().await;
    }
}

/// Draws the board and screen-space feedback, preserving fatal render recovery.
fn present_local_scenes<R, P>(
    local_match: &LocalMatch<R, P>,
    feedback: &mut LocalFeedback,
    renderer: &mut MacroquadRenderer,
    frame: &tabula_presentation::FrameCtx,
    board_frame: &tabula_presentation::FrameCtx,
) where
    R: GameRules,
    P: GamePresentation<Rules = R>,
{
    if feedback.allows_gameplay(false)
        && renderer.submit(&local_match.present(board_frame)).is_err()
    {
        feedback.note_render_error();
    }
    // Supported screen-space primitives remain available even when the board
    // failed preflight. The dock uses the full frame, never the board camera.
    if let Ok(scene) = feedback.present(frame) {
        let _ = renderer.submit(&scene);
    }
}

/// Uses one viewport for board input/rendering and suppresses held completion keys.
fn sync_local_frame<R, P>(
    local_match: &mut LocalMatch<R, P>,
    feedback: &mut LocalFeedback,
    frame: &tabula_presentation::FrameCtx,
) -> tabula_presentation::FrameCtx
where
    R: GameRules,
    P: GamePresentation<Rules = R>,
    P::Local: SetViewport,
{
    if feedback.sync_match(local_match) {
        suppress_held_activation(feedback);
    }
    let board_frame = feedback.board_frame(frame);
    local_match.local_mut().set_viewport(board_frame.viewport());
    board_frame
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
impl SetViewport for tabula_game_tiles::presentation::TilesLocal { // xtask-allow-game-id: direct Phase 3 local vertical slice wiring.
    fn set_viewport(&mut self, viewport: tabula_presentation::Viewport) {
        Self::set_viewport(self, viewport);
    }
}

#[derive(Clone, Copy, Debug)]
struct Options {
    game: SelectedGame,
    seats: u8,
    fill: SeatFill,
    theme: tabula_design::ThemeKind,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            game: SelectedGame::default(),
            seats: 3,
            fill: SeatFill::default(),
            theme: tabula_design::ThemeKind::Light,
        }
    }
}

fn parse_options() -> Options {
    parse_options_from(std::env::args().skip(1))
}

fn parse_options_from(mut args: impl Iterator<Item = String>) -> Options {
    let mut options = Options::default();
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
            "--theme" => {
                if let Some(theme) = args.next().and_then(|name| parse_local_theme(&name)) {
                    options.theme = theme;
                }
            }
            _ => {}
        }
    }
    options
}

#[rustfmt::skip]
fn parse_game(name: &str) -> SelectedGame {
    match name.to_ascii_lowercase().as_str() {
        "chess" => SelectedGame::Chess, // xtask-allow-game-id: direct Phase 2 local vertical slice wiring.
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

/// A recovery key held across creation cannot immediately submit a new move.
fn fresh_feedback() -> LocalFeedback {
    let mut feedback = LocalFeedback::default();
    suppress_held_activation(&mut feedback);
    feedback
}

/// A timer/bot completion must not turn a held board key into a restart action.
fn suppress_held_activation(feedback: &mut LocalFeedback) {
    for (key, native_key) in [
        (tabula_presentation::Key::Enter, mq::KeyCode::Enter),
        (tabula_presentation::Key::Space, mq::KeyCode::Space),
    ] {
        if mq::is_key_down(native_key) {
            feedback.suppress_opening_input(&tabula_presentation::InputEvent::Key {
                key,
                pressed: true,
            });
        }
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

/// Every fresh local match starts at logical time zero, including recovery.
fn session_elapsed_ms(started_at_ms: u64, now_ms: u64) -> u64 {
    now_ms.saturating_sub(started_at_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_theme_options_keep_the_selected_game_and_invalid_values_keep_light() {
        for (name, expected) in [
            ("light", tabula_design::ThemeKind::Light),
            ("dark", tabula_design::ThemeKind::Dark),
            ("hc-light", tabula_design::ThemeKind::HighContrastLight),
            ("hc-dark", tabula_design::ThemeKind::HighContrastDark),
        ] {
            let options = parse_options_from(
                ["--theme", name, "--seats", "4", "--solo"]
                    .into_iter()
                    .map(str::to_owned),
            );
            assert_eq!(options.theme, expected);
            assert_eq!(options.seats, 4);
            assert_eq!(options.fill, SeatFill::Solo);
        }
        for args in [vec!["--theme"], vec!["--theme", "unknown"]] {
            assert_eq!(
                parse_options_from(args.into_iter().map(str::to_owned)).theme,
                tabula_design::ThemeKind::Light
            );
        }
    }

    #[test]
    fn fresh_local_recovery_resets_elapsed_clock_time_and_clamps_clock_regression() {
        assert_eq!(session_elapsed_ms(900_000, 900_000), 0);
        assert_eq!(session_elapsed_ms(900_000, 900_012), 12);
        assert_eq!(session_elapsed_ms(900_000, 899_999), 0);
    }
}
